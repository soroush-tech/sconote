use super::*;

const N_FRAMES: usize = 60;

fn matrix() -> Vec<f32> {
    vec![0.0; N_FRAMES * PITCH_BINS]
}

fn set(matrix: &mut [f32], frames: std::ops::Range<usize>, bin: usize, value: f32) {
    for frame in frames {
        matrix[frame * PITCH_BINS + bin] = value;
    }
}

/// Options with the heuristics off, so tests exercise one mechanism at a time.
fn plain_options() -> NoteCreationOptions {
    NoteCreationOptions {
        infer_onsets: false,
        melodia_trick: false,
        ..NoteCreationOptions::default()
    }
}

#[test]
fn onset_peak_with_sustained_energy_becomes_a_note() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 40] = 0.9;
    set(&mut frames, 10..40, 40, 0.8);
    let notes = notes_from_activations(&onsets, &frames, N_FRAMES, &plain_options());
    assert_eq!(
        notes,
        vec![RawNote {
            start_frame: 10,
            end_frame: 40,
            pitch_bin: 40
        }]
    );
}

#[test]
fn note_shorter_than_minimum_is_dropped() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 40] = 0.9;
    set(&mut frames, 10..18, 40, 0.8); // 8 frames < min_note_len 11
    let notes = notes_from_activations(&onsets, &frames, N_FRAMES, &plain_options());
    assert_eq!(notes, Vec::new());
}

#[test]
fn onset_below_threshold_starts_nothing() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 40] = 0.4; // below onset_threshold 0.5
    set(&mut frames, 10..40, 40, 0.8);
    let notes = notes_from_activations(&onsets, &frames, N_FRAMES, &plain_options());
    assert_eq!(notes, Vec::new());
}

#[test]
fn short_energy_dip_does_not_split_a_note() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 40] = 0.9;
    set(&mut frames, 10..25, 40, 0.8);
    // 5 quiet frames (< energy tolerance 11), then energy resumes.
    set(&mut frames, 30..45, 40, 0.8);
    let notes = notes_from_activations(&onsets, &frames, N_FRAMES, &plain_options());
    assert_eq!(
        notes,
        vec![RawNote {
            start_frame: 10,
            end_frame: 45,
            pitch_bin: 40
        }]
    );
}

#[test]
fn melodia_trick_finds_energy_without_an_onset() {
    let onsets = matrix();
    let mut frames = matrix();
    set(&mut frames, 20..45, 30, 0.7);
    let without = notes_from_activations(&onsets, &frames, N_FRAMES, &plain_options());
    assert_eq!(without, Vec::new());
    let with = notes_from_activations(
        &onsets,
        &frames,
        N_FRAMES,
        &NoteCreationOptions {
            melodia_trick: true,
            ..plain_options()
        },
    );
    assert_eq!(with.len(), 1);
    assert_eq!(with[0].pitch_bin, 30);
    // The melodia walk stops at the first fully-quiet tolerance run; bounds
    // land within a couple of frames of the true span.
    assert!(with[0].start_frame >= 19 && with[0].start_frame <= 21);
    assert!(with[0].end_frame >= 43 && with[0].end_frame <= 45);
}

#[test]
fn inferred_onsets_start_a_note_from_a_sharp_energy_rise() {
    let onsets = matrix(); // network reports no onset at all
    let mut frames = matrix();
    set(&mut frames, 10..40, 40, 0.8);
    let notes = notes_from_activations(
        &onsets,
        &frames,
        N_FRAMES,
        &NoteCreationOptions {
            infer_onsets: true,
            ..plain_options()
        },
    );
    // With every onset at zero the rescale keeps rises at zero, so nothing
    // can start — unless some real onset exists to scale against.
    assert_eq!(notes, Vec::new());

    let mut onsets = matrix();
    onsets[50 * PITCH_BINS + 10] = 0.9; // unrelated real onset, sets the scale
    let notes = notes_from_activations(
        &onsets,
        &frames,
        N_FRAMES,
        &NoteCreationOptions {
            infer_onsets: true,
            ..plain_options()
        },
    );
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].pitch_bin, 40);
    assert_eq!(notes[0].start_frame, 10);
}

#[test]
fn claimed_energy_is_not_reused_by_the_melodia_trick() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 40] = 0.9;
    set(&mut frames, 10..40, 40, 0.8);
    let options = NoteCreationOptions {
        melodia_trick: true,
        ..plain_options()
    };
    let notes = notes_from_activations(&onsets, &frames, N_FRAMES, &options);
    // One note from the onset pass; the melodia pass must not re-emit it.
    assert_eq!(notes.len(), 1);
}
