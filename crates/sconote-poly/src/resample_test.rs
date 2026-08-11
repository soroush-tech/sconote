use std::f64::consts::TAU;

use super::*;

fn sine(frequency_hz: f64, sample_rate: u32, seconds: f64) -> Vec<f32> {
    let count = (seconds * f64::from(sample_rate)) as usize;
    (0..count)
        .map(|i| (TAU * frequency_hz * i as f64 / f64::from(sample_rate)).sin() as f32)
        .collect()
}

#[test]
fn matching_rate_returns_the_audio_unchanged() {
    let audio = MonoAudio {
        samples: vec![0.1, 0.2, 0.3],
        sample_rate: 22_050,
    };
    assert_eq!(resample(&audio, 22_050), audio);
}

#[test]
fn output_length_follows_the_rate_ratio() {
    let audio = MonoAudio {
        samples: vec![0.0; 48_000],
        sample_rate: 48_000,
    };
    assert_eq!(resample(&audio, 22_050).samples.len(), 22_050);
}

#[test]
fn downsampled_sine_matches_a_natively_generated_one() {
    let audio = MonoAudio {
        samples: sine(440.0, 48_000, 0.5),
        sample_rate: 48_000,
    };
    let resampled = resample(&audio, 22_050);
    let reference = sine(440.0, 22_050, 0.5);
    // Compare away from the edges, where the kernel is fully supported.
    let worst = resampled.samples[200..reference.len() - 200]
        .iter()
        .zip(&reference[200..reference.len() - 200])
        .map(|(&a, &b)| (a - b).abs())
        .fold(0.0_f32, f32::max);
    assert!(worst < 0.01, "worst sample error {worst}");
}

#[test]
fn upsampled_sine_matches_a_natively_generated_one() {
    let audio = MonoAudio {
        samples: sine(440.0, 22_050, 0.5),
        sample_rate: 22_050,
    };
    let resampled = resample(&audio, 44_100);
    let reference = sine(440.0, 44_100, 0.5);
    let worst = resampled.samples[200..reference.len() - 200]
        .iter()
        .zip(&reference[200..reference.len() - 200])
        .map(|(&a, &b)| (a - b).abs())
        .fold(0.0_f32, f32::max);
    assert!(worst < 0.01, "worst sample error {worst}");
}

#[test]
fn dc_level_is_preserved() {
    let audio = MonoAudio {
        samples: vec![0.5; 4800],
        sample_rate: 48_000,
    };
    let resampled = resample(&audio, 22_050);
    let mid = &resampled.samples[500..resampled.samples.len() - 500];
    assert!(mid.iter().all(|&s| (s - 0.5).abs() < 0.005));
}
