//! Second-order IIR filter, coefficients after the RBJ Audio EQ Cookbook, transposed
//! direct form II (good numerical behaviour in f32, two state variables).

use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BiquadKind {
    LowPass,
    HighPass,
    /// Bell; `gain_db` is used only here.
    Peaking {
        gain_db: f32,
    },
}

/// Normalised coefficients (`a0` = 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coefficients {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl Coefficients {
    pub fn new(kind: BiquadKind, sample_rate: f32, freq_hz: f32, q: f32) -> Self {
        let w0 = 2.0 * PI * (freq_hz / sample_rate).clamp(1e-5, 0.4999);
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q.max(1e-3));
        let (b0, b1, b2, a0, a1, a2) = match kind {
            BiquadKind::LowPass => {
                let b1 = 1.0 - cos;
                (b1 / 2.0, b1, b1 / 2.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha)
            }
            BiquadKind::HighPass => {
                let b1 = -(1.0 + cos);
                (
                    -b1 / 2.0,
                    b1,
                    -b1 / 2.0,
                    1.0 + alpha,
                    -2.0 * cos,
                    1.0 - alpha,
                )
            }
            BiquadKind::Peaking { gain_db } => {
                let a = 10f32.powf(gain_db / 40.0);
                (
                    1.0 + alpha * a,
                    -2.0 * cos,
                    1.0 - alpha * a,
                    1.0 + alpha / a,
                    -2.0 * cos,
                    1.0 - alpha / a,
                )
            }
        };
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Biquad {
    c: Coefficients,
    z1: f32,
    z2: f32,
}

impl Biquad {
    pub fn new(kind: BiquadKind, sample_rate: f32, freq_hz: f32, q: f32) -> Self {
        Self {
            c: Coefficients::new(kind, sample_rate, freq_hz, q),
            z1: 0.0,
            z2: 0.0,
        }
    }

    /// Swap coefficients without resetting state (parameter changes while audio runs).
    pub fn set_coefficients(&mut self, c: Coefficients) {
        self.c = c;
    }

    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    #[inline]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        let c = &self.c;
        let y = c.b0 * x + self.z1;
        self.z1 = c.b1 * x - c.a1 * y + self.z2;
        self.z2 = c.b2 * x - c.a2 * y;
        y
    }

    /// In place, no allocation — safe for the real-time thread.
    pub fn process(&mut self, block: &mut [f32]) {
        for s in block.iter_mut() {
            *s = self.process_sample(*s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{gain_to_db, rms};

    const SR: f32 = 48_000.0;

    fn sine(freq: f32, len: usize) -> Vec<f32> {
        (0..len)
            .map(|i| (2.0 * PI * freq * i as f32 / SR).sin())
            .collect()
    }

    /// Gain in dB of the filter at `freq`, measured on the settled second half.
    fn measured_gain_db(mut f: Biquad, freq: f32) -> f32 {
        let input = sine(freq, 48_000);
        let mut out = input.clone();
        f.process(&mut out);
        gain_to_db(rms(&out[24_000..]) / rms(&input[24_000..]))
    }

    #[test]
    fn low_pass_passes_low_and_cuts_high() {
        let lp = || Biquad::new(BiquadKind::LowPass, SR, 1_000.0, 0.707);
        assert!(measured_gain_db(lp(), 100.0).abs() < 0.1);
        assert!(
            (measured_gain_db(lp(), 1_000.0) + 3.0).abs() < 0.2,
            "-3 dB at cutoff"
        );
        assert!(measured_gain_db(lp(), 10_000.0) < -35.0);
    }

    #[test]
    fn high_pass_80hz_removes_rumble_keeps_voice() {
        let hp = || Biquad::new(BiquadKind::HighPass, SR, 80.0, 0.707);
        assert!(measured_gain_db(hp(), 20.0) < -20.0);
        assert!(measured_gain_db(hp(), 1_000.0).abs() < 0.1);
    }

    #[test]
    fn peaking_hits_its_gain_at_center() {
        let pk = |g| Biquad::new(BiquadKind::Peaking { gain_db: g }, SR, 3_000.0, 1.0);
        assert!((measured_gain_db(pk(6.0), 3_000.0) - 6.0).abs() < 0.1);
        assert!((measured_gain_db(pk(-9.0), 3_000.0) + 9.0).abs() < 0.1);
        assert!(
            measured_gain_db(pk(6.0), 100.0).abs() < 0.2,
            "flat far away"
        );
    }

    #[test]
    fn reset_clears_state() {
        let mut f = Biquad::new(BiquadKind::LowPass, SR, 1_000.0, 0.707);
        f.process(&mut [1.0; 64]);
        f.reset();
        assert_eq!(f.process_sample(0.0), 0.0);
    }
}
