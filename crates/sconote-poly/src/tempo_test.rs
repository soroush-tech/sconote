use super::*;

fn notes_at(onsets: &[f64]) -> Vec<TranscribedNote> {
    onsets
        .iter()
        .map(|&onset_s| TranscribedNote {
            midi: 60,
            onset_s,
            offset_s: onset_s + 0.1,
        })
        .collect()
}

#[test]
fn steady_quarter_notes_at_100_bpm_are_detected() {
    // 100 BPM → 0.6 s per beat, slight jitter.
    let onsets: Vec<f64> = (0..24).map(|i| f64::from(i) * 0.6 + 0.003).collect();
    let bpm = estimate_bpm(&notes_at(&onsets));
    assert!((bpm - 100.0).abs() <= 1.0, "got {bpm}");
}

#[test]
fn mixed_eighths_and_quarters_agree_on_the_beat() {
    // 120 BPM: beat 0.5 s, some subdivided in half.
    let onsets = [
        0.0, 0.25, 0.5, 1.0, 1.25, 1.5, 2.0, 2.5, 2.75, 3.0, 3.5, 4.0, 4.25, 4.5, 5.0,
    ];
    let bpm = estimate_bpm(&notes_at(&onsets));
    assert!((bpm - 120.0).abs() <= 1.0, "got {bpm}");
}

#[test]
fn chords_count_as_one_rhythmic_event() {
    // Three-note chords every 0.5 s; intra-chord spread below the cluster
    // threshold must not masquerade as fast intervals.
    let mut onsets = Vec::new();
    for i in 0..16 {
        let t = f64::from(i) * 0.5;
        onsets.extend([t, t + 0.01, t + 0.02]);
    }
    let bpm = estimate_bpm(&notes_at(&onsets));
    assert!((bpm - 120.0).abs() <= 1.0, "got {bpm}");
}

#[test]
fn too_few_notes_fall_back_to_default() {
    assert_eq!(estimate_bpm(&notes_at(&[0.0, 1.0, 2.0])), DEFAULT_BPM);
    assert_eq!(estimate_bpm(&[]), DEFAULT_BPM);
}

#[test]
fn out_of_range_tempo_folds_into_a_supported_multiple() {
    // 200 BPM (0.3 s per event) is outside [70, 160]; its half, 100 BPM,
    // explains every interval as two beats.
    let onsets: Vec<f64> = (0..24).map(|i| f64::from(i) * 0.3).collect();
    let bpm = estimate_bpm(&notes_at(&onsets));
    assert!((bpm - 100.0).abs() <= 1.0, "got {bpm}");
}
