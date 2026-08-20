//! Synthetic end-to-end: render the opening of the real full-band MIDI,
//! transcribe it with the network, and score against ground truth.
//!
//! This is clean audio (no room, no mic), so it measures the ceiling of the
//! pipeline itself. Run with `--nocapture` to see the score break-down.

use std::fs;
use std::path::Path;

use crate::{
    BasicPitch, MODEL_SAMPLE_RATE, NoteCreationOptions, notes_from_midi, render_notes, score_notes,
    transcribe,
};

const CLIP_SECONDS: f64 = 30.0;
const ONSET_TOLERANCE_S: f64 = 0.1;

#[test]
fn transcribes_rendered_full_band_opening_with_usable_accuracy() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/midi/Queen - Bohemian Rhapsody.mid");
    let bytes = fs::read(path).expect("fixture MIDI present");
    let mut clip: Vec<_> = notes_from_midi(&bytes)
        .expect("valid MIDI")
        .into_iter()
        .filter(|note| note.onset_s < CLIP_SECONDS)
        .collect();
    for note in &mut clip {
        note.offset_s = note.offset_s.min(CLIP_SECONDS);
    }
    assert!(clip.len() > 100, "clip unexpectedly sparse: {}", clip.len());

    let audio = render_notes(&clip, MODEL_SAMPLE_RATE);
    let model = BasicPitch::new().expect("model loads");
    let notes = transcribe(&audio, &model, &NoteCreationOptions::default()).expect("transcribes");

    let report = score_notes(&clip, &notes, ONSET_TOLERANCE_S);
    println!(
        "reference={} predicted={} matched={} precision={:.3} recall={:.3} f1={:.3}",
        clip.len(),
        notes.len(),
        report.matched,
        report.precision(),
        report.recall(),
        report.f1(),
    );
    // Currently scores ≈0.84 (precision 0.90, recall 0.80); the margin
    // below guards against regressions without being flaky.
    assert!(report.f1() > 0.75, "f1 regressed: {:.3}", report.f1());
}
