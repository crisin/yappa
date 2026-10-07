//! Cost per 10 ms block (480 samples at 48 kHz) — the unit of the latency budget
//! (planning doc: DSP under 5 ms per block). `cargo xtask bench`.

use criterion::{criterion_group, criterion_main, Criterion};
use dsp::{Biquad, BiquadKind};
use std::hint::black_box;

const SR: f32 = 48_000.0;
const BLOCK: usize = 480;

fn eq_chain(c: &mut Criterion) {
    let mut chain = [
        Biquad::new(BiquadKind::HighPass, SR, 80.0, 0.707),
        Biquad::new(BiquadKind::Peaking { gain_db: -3.0 }, SR, 250.0, 1.0),
        Biquad::new(BiquadKind::Peaking { gain_db: 2.0 }, SR, 1_500.0, 1.0),
        Biquad::new(BiquadKind::Peaking { gain_db: 3.0 }, SR, 4_000.0, 1.2),
        Biquad::new(BiquadKind::LowPass, SR, 16_000.0, 0.707),
    ];
    let mut block: Vec<f32> = (0..BLOCK)
        .map(|i| ((i as f32) * 0.05).sin() * 0.5)
        .collect();
    c.bench_function("5-band EQ, one 10 ms block", |b| {
        b.iter(|| {
            for f in chain.iter_mut() {
                f.process(black_box(&mut block));
            }
        })
    });
}

criterion_group!(benches, eq_chain);
criterion_main!(benches);
