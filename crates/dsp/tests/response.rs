//! Frequency-response snapshots and property tests for the biquad.
//! Snapshot changed on purpose? `cargo insta review`.

use dsp::{gain_to_db, rms, Biquad, BiquadKind};
use proptest::prelude::*;
use std::f32::consts::PI;

const SR: f32 = 48_000.0;
const PROBES: [f32; 9] = [
    20.0, 50.0, 80.0, 200.0, 1_000.0, 3_000.0, 6_000.0, 12_000.0, 20_000.0,
];

fn gain_db_at(kind: BiquadKind, f0: f32, q: f32, freq: f32) -> f32 {
    let mut filter = Biquad::new(kind, SR, f0, q);
    let input: Vec<f32> = (0..48_000)
        .map(|i| (2.0 * PI * freq * i as f32 / SR).sin())
        .collect();
    let mut out = input.clone();
    filter.process(&mut out);
    gain_to_db(rms(&out[24_000..]) / rms(&input[24_000..]))
}

/// One line per probe frequency, rounded to 0.1 dB so the snapshot is stable across
/// platforms (Windows/macOS float differences stay far below that).
fn response(kind: BiquadKind, f0: f32, q: f32) -> String {
    PROBES
        .iter()
        .map(|&f| {
            let db = (gain_db_at(kind, f0, q, f) * 10.0).round() / 10.0;
            // -0.0 vs +0.0 would depend on the last float bit; normalise.
            let db = if db == 0.0 { 0.0 } else { db };
            format!("{f:>7} Hz  {db:+6.1} dB")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn high_pass_80hz_response() {
    insta::assert_snapshot!(response(BiquadKind::HighPass, 80.0, 0.707));
}

#[test]
fn presence_boost_response() {
    insta::assert_snapshot!(response(BiquadKind::Peaking { gain_db: 4.0 }, 3_000.0, 1.0));
}

proptest! {
    /// Any sane parameter set stays stable: bounded input never produces NaN/inf or blows up.
    #[test]
    fn filters_stay_stable(
        f0 in 20.0f32..20_000.0,
        q in 0.3f32..10.0,
        gain in -24.0f32..24.0,
        kind in 0u8..3,
        seed in any::<u32>(),
    ) {
        let kind = match kind {
            0 => BiquadKind::LowPass,
            1 => BiquadKind::HighPass,
            _ => BiquadKind::Peaking { gain_db: gain },
        };
        let mut f = Biquad::new(kind, SR, f0, q);
        // Deterministic pseudo-noise in [-1, 1].
        let mut x = seed.max(1);
        let mut block: Vec<f32> = (0..4_800).map(|_| {
            x ^= x << 13; x ^= x >> 17; x ^= x << 5;
            (x as f32 / u32::MAX as f32) * 2.0 - 1.0
        }).collect();
        f.process(&mut block);
        // +24 dB boost on full-scale noise stays well under 100x.
        prop_assert!(block.iter().all(|s| s.is_finite() && s.abs() < 100.0));
    }
}
