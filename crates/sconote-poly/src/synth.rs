//! Render ground-truth notes to audio, for synthetic end-to-end tests.
//!
//! Each note becomes a harmonic-rich tone (fundamental + rolled-off
//! overtones, sawtooth-like) with a short attack, an exponential decay over
//! the note, and a short release - close enough to an instrument that a
//! transcription engine faces realistic material, while staying fully
//! deterministic.

use std::f64::consts::TAU;

use crate::ground_truth::GroundTruthNote;
use crate::wav::MonoAudio;

const ATTACK_S: f64 = 0.01;
const RELEASE_S: f64 = 0.02;
const HARMONICS: u32 = 6;
/// Exponential amplitude decay over the note body (piano-like).
const DECAY_RATE_PER_S: f64 = 2.0;
/// Peak level after normalization, leaving headroom below full scale.
const PEAK_LEVEL: f32 = 0.9;

/// Mix every note into one mono buffer at `sample_rate`, normalized to
/// [`PEAK_LEVEL`]. The buffer ends one release past the last offset.
pub fn render_notes(notes: &[GroundTruthNote], sample_rate: u32) -> MonoAudio {
    let rate = f64::from(sample_rate);
    let end_s = notes
        .iter()
        .map(|note| note.offset_s + RELEASE_S)
        .fold(0.0, f64::max);
    let mut samples = vec![0.0_f32; (end_s * rate).ceil() as usize];
    for note in notes {
        let frequency = 440.0 * f64::powf(2.0, (f64::from(note.midi) - 69.0) / 12.0);
        let start = (note.onset_s * rate) as usize;
        let stop = ((note.offset_s + RELEASE_S) * rate).min(samples.len() as f64) as usize;
        for (index, sample) in samples[start..stop].iter_mut().enumerate() {
            let t = index as f64 / rate;
            // 1/h² rolloff: bright enough to be realistic, dark enough that
            // the model doesn't hear upper harmonics as extra notes.
            let tone: f64 = (1..=HARMONICS)
                .map(|h| (TAU * frequency * f64::from(h) * t).sin() / f64::from(h * h))
                .sum();
            *sample += (tone * envelope(t, note.offset_s - note.onset_s)) as f32;
        }
    }
    let peak = samples.iter().fold(0.0_f32, |max, s| max.max(s.abs()));
    if peak > 0.0 {
        let gain = PEAK_LEVEL / peak;
        for sample in &mut samples {
            *sample *= gain;
        }
    }
    MonoAudio {
        samples,
        sample_rate,
    }
}

/// Amplitude at `t` seconds into a note lasting `duration_s`.
fn envelope(t: f64, duration_s: f64) -> f64 {
    let body = f64::exp(-DECAY_RATE_PER_S * t);
    let attack = (t / ATTACK_S).min(1.0);
    let release = ((duration_s + RELEASE_S - t) / RELEASE_S).clamp(0.0, 1.0);
    body * attack * release
}

#[cfg(test)]
#[path = "synth_test.rs"]
mod synth_test;
