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
fn fast_tempo_at_the_top_of_the_range_is_detected() {
    // 200 BPM (0.3 s per event) sits at the top of the search range.
    let onsets: Vec<f64> = (0..24).map(|i| f64::from(i) * 0.3).collect();
    let bpm = estimate_bpm(&notes_at(&onsets));
    assert!((bpm - 200.0).abs() <= 1.0, "got {bpm}");
}

#[test]
fn slow_tempo_at_the_bottom_of_the_range_is_detected() {
    // 50 BPM (1.2 s per beat) - below the old 70 BPM floor, which would have
    // folded this onto its double.
    let onsets: Vec<f64> = (0..24).map(|i| f64::from(i) * 1.2).collect();
    let bpm = estimate_bpm(&notes_at(&onsets));
    assert!((bpm - 50.0).abs() <= 1.0, "got {bpm}");
}

#[test]
fn track_beats_is_uniform_for_a_steady_performance() {
    let onsets: Vec<f64> = (0..16).map(|i| f64::from(i) * 0.5).collect();
    let beats = track_beats(&notes_at(&onsets));
    for pair in beats.windows(2) {
        assert!((pair[1] - pair[0] - 0.5).abs() < 0.01, "beats {beats:?}");
    }
    assert!(*beats.last().unwrap() >= 7.5 + 0.1, "must cover the last note");
}

#[test]
fn track_beats_follows_a_gradual_ritardando() {
    // Steady quarters, then the spacing grows 4% per beat - every onset
    // must still have a beat on it, and the grid must end slower.
    let mut onsets = Vec::new();
    let mut t = 0.0;
    let mut spacing = 0.5;
    for i in 0..32 {
        onsets.push(t);
        if i >= 12 {
            spacing *= 1.04;
        }
        t += spacing;
    }
    let beats = track_beats(&notes_at(&onsets));
    for &onset in &onsets {
        let nearest = beats
            .iter()
            .map(|beat| (beat - onset).abs())
            .fold(f64::INFINITY, f64::min);
        assert!(nearest < 0.05, "onset {onset:.2} has no beat (nearest {nearest:.3})");
    }
    let first = beats[1] - beats[0];
    let last = beats[beats.len() - 1] - beats[beats.len() - 2];
    assert!(last > first * 1.4, "grid must slow down: {first:.3} → {last:.3}");
}

#[test]
fn track_beats_snaps_to_beats_not_subdivisions() {
    // Straight eighths: onsets every 0.25 s, but the beat (folded into the
    // supported range) is 0.5 s - the grid must step over the off-beats.
    let onsets: Vec<f64> = (0..32).map(|i| f64::from(i) * 0.25).collect();
    let beats = track_beats(&notes_at(&onsets));
    for pair in beats.windows(2) {
        assert!((pair[1] - pair[0] - 0.5).abs() < 0.01, "beats {beats:?}");
    }
}

#[test]
fn track_beats_coasts_over_a_rest() {
    // A one-beat hole in otherwise steady quarters: the grid keeps a beat
    // there anyway.
    let onsets: Vec<f64> = (0..16)
        .map(|i| f64::from(i) * 0.5)
        .filter(|&t| (t - 4.0).abs() > 0.01)
        .collect();
    let beats = track_beats(&notes_at(&onsets));
    let nearest = beats.iter().map(|beat| (beat - 4.0).abs()).fold(f64::INFINITY, f64::min);
    assert!(nearest < 0.05, "no beat at the rest: nearest {nearest:.3}");
}

#[test]
fn track_beats_falls_back_to_a_uniform_default_grid_when_sparse() {
    let beats = track_beats(&notes_at(&[0.0, 1.0]));
    assert!(beats.len() >= 2);
    for pair in beats.windows(2) {
        assert!((pair[1] - pair[0] - 0.5).abs() < 1e-9, "default 120 BPM grid");
    }
    assert!(*beats.last().unwrap() >= 1.1, "must cover the last offset");

    let empty = track_beats(&[]);
    assert!(empty.len() >= 2);
}

#[test]
fn out_of_range_tempo_folds_into_a_supported_multiple() {
    // 240 BPM (0.25 s per event) is still outside [40, 200]; its half,
    // 120 BPM, explains every interval as two beats.
    let onsets: Vec<f64> = (0..24).map(|i| f64::from(i) * 0.25).collect();
    let bpm = estimate_bpm(&notes_at(&onsets));
    assert!((bpm - 120.0).abs() <= 1.0, "got {bpm}");
}
