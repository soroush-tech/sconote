//! Shared audio fixtures for tests.

use std::f32::consts::TAU;

pub(crate) const SAMPLE_RATE: u32 = 48_000;
pub(crate) const WINDOW: usize = 2048;

pub(crate) fn sine(frequency_hz: f32, samples: usize) -> Vec<f32> {
    (0..samples)
        .map(|i| (TAU * frequency_hz * i as f32 / SAMPLE_RATE as f32).sin())
        .collect()
}

/// Deterministic full-scale white noise (tiny LCG — keeps tests reproducible
/// without a rand dependency).
pub(crate) fn noise(samples: usize) -> Vec<f32> {
    let mut state: u32 = 0x02F6_E2B1;
    (0..samples)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state as f32 / u32::MAX as f32) * 2.0 - 1.0
        })
        .collect()
}
