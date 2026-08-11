use sconote_core::PitchEngine;

use super::*;

const SAMPLE_RATE: u32 = 48_000;

fn note(midi: u8, onset_s: f64, offset_s: f64) -> GroundTruthNote {
    GroundTruthNote {
        midi,
        onset_s,
        offset_s,
    }
}

#[test]
fn no_notes_render_to_empty_audio() {
    let audio = render_notes(&[], SAMPLE_RATE);
    assert_eq!(audio.samples, Vec::new());
    assert_eq!(audio.sample_rate, SAMPLE_RATE);
}

#[test]
fn rendered_note_is_silent_outside_its_span() {
    let audio = render_notes(&[note(69, 0.5, 1.0)], SAMPLE_RATE);
    let before = &audio.samples[..(0.5 * f64::from(SAMPLE_RATE)) as usize];
    assert!(before.iter().all(|&s| s == 0.0));
    // Buffer ends one release after the offset.
    let expected_len = ((1.0 + RELEASE_S) * f64::from(SAMPLE_RATE)).ceil() as usize;
    assert_eq!(audio.samples.len(), expected_len);
}

#[test]
fn rendered_audio_is_normalized_to_peak_level() {
    let chord = [note(60, 0.0, 1.0), note(64, 0.0, 1.0), note(67, 0.0, 1.0)];
    let audio = render_notes(&chord, SAMPLE_RATE);
    let peak = audio
        .samples
        .iter()
        .fold(0.0_f32, |max, s| max.max(s.abs()));
    assert!((peak - PEAK_LEVEL).abs() < 1e-3, "peak was {peak}");
}

#[test]
fn pitch_engine_recognizes_a_rendered_note() {
    // Cross-check with the monophonic engine: a rendered A4 must be heard
    // as A4, so the synthetic audio is realistic enough to detect.
    let audio = render_notes(&[note(69, 0.0, 1.0)], SAMPLE_RATE);
    let mut engine = PitchEngine::new(SAMPLE_RATE, 2048);
    // Analyze a window from the sustained body, past the attack transient.
    let start = (0.1 * f64::from(SAMPLE_RATE)) as usize;
    let event = engine
        .process(&audio.samples[start..start + 2048])
        .expect("pitch expected");
    assert_eq!(event.note_name, "A4");
}
