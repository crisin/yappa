//! Click generator, click detector and the latency bookkeeping — pure, no I/O.
//!
//! A click is a short 2 kHz burst starting at sample 0 of a 10 ms block. The sender notes
//! when it handed that block on, the receiver notes when the onset came out the other end;
//! both use the same clock (one process, or one machine).

pub const SAMPLE_RATE: u32 = 48_000;
/// Samples in one 10 ms mono block.
pub const BLOCK: usize = 480;

const BURST_HZ: f32 = 2_000.0;
const BURST_SAMPLES: usize = 240; // 5 ms
const AMPLITUDE: f32 = 0.5;

pub struct ClickGen {
    period_blocks: u32,
    block: u32,
    noise: f32,
    rng: u32,
}

impl ClickGen {
    pub fn new(period_ms: u32) -> Self {
        Self {
            period_blocks: (period_ms / 10).max(2),
            block: 0,
            noise: 0.0,
            rng: 0x2545_F491,
        }
    }

    /// Adds white noise with this peak (share of full scale) under the clicks, so the
    /// encoder has something to spend its bitrate on. Keep it below the detector threshold.
    pub fn with_noise(mut self, peak: f32) -> Self {
        self.noise = peak.clamp(0.0, 0.4);
        self
    }

    /// Fills one 10 ms block. Returns true if this block starts with a click.
    pub fn fill(&mut self, out: &mut [i16]) -> bool {
        let click = self.block == 0;
        self.block = (self.block + 1) % self.period_blocks;
        out.fill(0);
        if self.noise > 0.0 {
            for s in out.iter_mut() {
                // xorshift32: cheap, deterministic, good enough for a noise floor.
                self.rng ^= self.rng << 13;
                self.rng ^= self.rng >> 17;
                self.rng ^= self.rng << 5;
                let unit = (self.rng >> 8) as f32 / (1u32 << 23) as f32 - 1.0;
                *s = (unit * self.noise * f32::from(i16::MAX)) as i16;
            }
        }
        if click {
            for (n, s) in out.iter_mut().take(BURST_SAMPLES).enumerate() {
                let phase = std::f32::consts::TAU * BURST_HZ * n as f32 / SAMPLE_RATE as f32;
                *s = s.saturating_add((phase.sin() * AMPLITUDE * f32::from(i16::MAX)) as i16);
            }
        }
        click
    }
}

/// Finds click onsets: the first sample above the threshold after a quiet stretch.
pub struct ClickDetector {
    threshold: i16,
    refractory: usize,
    since_onset: usize,
}

impl ClickDetector {
    /// `threshold` is a share of full scale (0..1); onsets closer than `refractory_ms`
    /// to the previous one are ignored (the burst itself, codec ringing, room echo).
    pub fn new(threshold: f32, refractory_ms: u32) -> Self {
        let refractory = (SAMPLE_RATE / 1000 * refractory_ms) as usize;
        Self {
            threshold: (threshold.clamp(0.0, 1.0) * f32::from(i16::MAX)) as i16,
            refractory,
            since_onset: refractory,
        }
    }

    /// Returns the index of an onset in this block, if any.
    pub fn scan(&mut self, block: &[i16]) -> Option<usize> {
        let mut found = None;
        for (i, s) in block.iter().enumerate() {
            if self.since_onset >= self.refractory && s.saturating_abs() > self.threshold {
                self.since_onset = 0;
                found.get_or_insert(i);
            } else {
                self.since_onset = self.since_onset.saturating_add(1);
            }
        }
        found
    }
}

/// Pairs every sent click with the first detection inside `window` seconds after it.
/// Returns the latencies in seconds and the number of clicks that never arrived.
pub fn match_clicks(sent: &[f64], detected: &[f64], window: f64) -> (Vec<f64>, usize) {
    let mut latencies = Vec::new();
    let mut missed = 0;
    for &t0 in sent {
        match detected.iter().find(|&&t1| t1 >= t0 && t1 - t0 < window) {
            Some(t1) => latencies.push(t1 - t0),
            None => missed += 1,
        }
    }
    (latencies, missed)
}

#[derive(Debug, PartialEq)]
pub struct Summary {
    pub n: usize,
    pub min: f64,
    pub median: f64,
    pub p95: f64,
    pub max: f64,
}

/// Nearest-rank percentiles; `None` for an empty series.
pub fn summarize(values: &[f64]) -> Option<Summary> {
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let rank = |p: f64| v[((p * v.len() as f64).ceil() as usize).clamp(1, v.len()) - 1];
    (!v.is_empty()).then(|| Summary {
        n: v.len(),
        min: v[0],
        median: rank(0.5),
        p95: rank(0.95),
        max: v[v.len() - 1],
    })
}

/// The longest stretch without a detection, as (start, length) in seconds.
pub fn longest_gap(detected: &[f64]) -> Option<(f64, f64)> {
    detected
        .windows(2)
        .map(|w| (w[0], w[1] - w[0]))
        .max_by(|a, b| a.1.total_cmp(&b.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generator_clicks_once_per_period_at_sample_zero() {
        let mut gen = ClickGen::new(500);
        let mut block = [0i16; BLOCK];
        let clicks: Vec<bool> = (0..100).map(|_| gen.fill(&mut block)).collect();
        assert_eq!(clicks.iter().filter(|c| **c).count(), 2);
        assert!(clicks[0] && clicks[50]);
    }

    #[test]
    fn detector_finds_the_generated_onset_within_a_sample_or_two() {
        let mut gen = ClickGen::new(500);
        let mut det = ClickDetector::new(0.1, 200);
        let mut block = [0i16; BLOCK];
        let mut onsets = Vec::new();
        for n in 0..100 {
            gen.fill(&mut block);
            if let Some(i) = det.scan(&block) {
                onsets.push((n, i));
            }
        }
        assert_eq!(
            onsets.len(),
            2,
            "one onset per click, the burst is not re-triggered"
        );
        assert_eq!(onsets[0].0, 0);
        assert_eq!(onsets[1].0, 50);
        assert!(onsets.iter().all(|(_, i)| *i <= 2));
    }

    #[test]
    fn noise_floor_stays_under_its_peak_and_does_not_trigger() {
        let mut gen = ClickGen::new(500).with_noise(0.03);
        let mut det = ClickDetector::new(0.1, 200);
        let mut block = [0i16; BLOCK];
        let mut onsets = 0;
        for n in 0..100 {
            gen.fill(&mut block);
            onsets += usize::from(det.scan(&block).is_some());
            if n % 50 != 0 {
                let peak = block.iter().map(|s| s.unsigned_abs()).max().unwrap();
                assert!(peak > 0 && f32::from(peak) <= 0.03 * f32::from(i16::MAX) + 1.0);
            }
        }
        assert_eq!(onsets, 2);
    }

    #[test]
    fn detector_ignores_noise_below_the_threshold() {
        let mut det = ClickDetector::new(0.1, 200);
        let block = [1000i16; BLOCK]; // -30 dBFS
        assert_eq!(det.scan(&block), None);
    }

    #[test]
    fn matching_counts_missed_clicks() {
        let sent = [0.0, 0.5, 1.0, 1.5];
        let detected = [0.08, 1.09, 1.61];
        let (lat, missed) = match_clicks(&sent, &detected, 0.4);
        assert_eq!(missed, 1);
        assert_eq!(lat.len(), 3);
        assert!((lat[0] - 0.08).abs() < 1e-9 && (lat[2] - 0.11).abs() < 1e-9);
    }

    #[test]
    fn summary_uses_nearest_rank() {
        let values: Vec<f64> = (1..=100).map(f64::from).collect();
        let s = summarize(&values).unwrap();
        assert_eq!(
            (s.n, s.min, s.median, s.p95, s.max),
            (100, 1.0, 50.0, 95.0, 100.0)
        );
        assert_eq!(summarize(&[]), None);
    }

    #[test]
    fn longest_gap_points_at_the_outage() {
        let detected = [0.1, 0.6, 1.1, 9.6, 10.1];
        assert_eq!(longest_gap(&detected), Some((1.1, 8.5)));
        assert_eq!(longest_gap(&[0.1]), None);
    }
}
