use super::*;

const TOLERANCE_S: f64 = 0.05;

fn reference(midi: u8, onset_s: f64) -> GroundTruthNote {
    GroundTruthNote {
        midi,
        onset_s,
        offset_s: onset_s + 0.5,
    }
}

fn predicted(midi: u8, onset_s: f64) -> TranscribedNote {
    TranscribedNote {
        midi,
        onset_s,
        offset_s: onset_s + 0.5,
    }
}

#[test]
fn perfect_transcription_scores_one() {
    let report = score_notes(
        &[reference(60, 1.0), reference(64, 2.0)],
        &[predicted(60, 1.0), predicted(64, 2.0)],
        TOLERANCE_S,
    );
    assert_eq!(report.matched, 2);
    assert_eq!(report.precision(), 1.0);
    assert_eq!(report.recall(), 1.0);
    assert_eq!(report.f1(), 1.0);
}

#[test]
fn missed_note_lowers_recall_not_precision() {
    let report = score_notes(
        &[reference(60, 1.0), reference(64, 2.0)],
        &[predicted(60, 1.0)],
        TOLERANCE_S,
    );
    assert_eq!(report.missed, vec![reference(64, 2.0)]);
    assert_eq!(report.precision(), 1.0);
    assert_eq!(report.recall(), 0.5);
}

#[test]
fn spurious_note_lowers_precision_not_recall() {
    let report = score_notes(
        &[reference(60, 1.0)],
        &[predicted(60, 1.0), predicted(64, 2.0)],
        TOLERANCE_S,
    );
    assert_eq!(report.spurious, vec![predicted(64, 2.0)]);
    assert_eq!(report.precision(), 0.5);
    assert_eq!(report.recall(), 1.0);
}

#[test]
fn pitch_mismatch_never_matches() {
    let report = score_notes(&[reference(60, 1.0)], &[predicted(61, 1.0)], TOLERANCE_S);
    assert_eq!(report.matched, 0);
    assert_eq!(report.missed.len(), 1);
    assert_eq!(report.spurious.len(), 1);
}

#[test]
fn onset_exactly_at_tolerance_matches() {
    // 0.0625 is exactly representable in binary, so the distance equals the
    // tolerance with no rounding.
    let report = score_notes(&[reference(60, 1.0)], &[predicted(60, 1.0625)], 0.0625);
    assert_eq!(report.matched, 1);
}

#[test]
fn onset_beyond_tolerance_does_not_match() {
    let report = score_notes(&[reference(60, 1.0)], &[predicted(60, 1.051)], TOLERANCE_S);
    assert_eq!(report.matched, 0);
}

#[test]
fn nearest_onset_wins_and_each_note_matches_once() {
    let report = score_notes(
        &[reference(60, 1.0)],
        &[predicted(60, 1.04), predicted(60, 1.01)],
        TOLERANCE_S,
    );
    assert_eq!(report.matched, 1);
    assert_eq!(report.spurious, vec![predicted(60, 1.04)]);
}

#[test]
fn unsorted_reference_is_matched_in_onset_order() {
    // The single prediction sits within tolerance of both references; onset
    // order means the 0.98 reference claims it, not the listed-first 1.02.
    let report = score_notes(
        &[reference(60, 1.02), reference(60, 0.98)],
        &[predicted(60, 0.99)],
        TOLERANCE_S,
    );
    assert_eq!(report.missed, vec![reference(60, 1.02)]);
}

#[test]
fn empty_inputs_score_zero() {
    let report = score_notes(&[], &[], TOLERANCE_S);
    assert_eq!(report.matched, 0);
    assert_eq!(report.precision(), 0.0);
    assert_eq!(report.recall(), 0.0);
    assert_eq!(report.f1(), 0.0);
}

#[test]
fn f1_is_harmonic_mean_of_precision_and_recall() {
    // 1 matched, 1 missed → precision 1, recall 0.5, F1 = 2/3.
    let report = score_notes(
        &[reference(60, 1.0), reference(64, 2.0)],
        &[predicted(60, 1.0)],
        TOLERANCE_S,
    );
    assert!((report.f1() - 2.0 / 3.0).abs() < 1e-12);
}
