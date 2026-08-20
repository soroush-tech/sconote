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
        onset_ghost_energy_ratio: 0.0,
        overtone_ghost_energy_ratio: 0.0,
        retrigger_octave_veto: 0.0,
        retrigger_dip_ratio: 0.0,
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
    set(&mut frames, 10..16, 40, 0.8); // 6 frames < min_note_len 7
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
fn weak_onset_peak_does_not_split_a_sounding_note() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 40] = 0.9;
    onsets[30 * PITCH_BINS + 40] = 0.6; // probability ripple mid-note
    set(&mut frames, 10..55, 40, 0.8);
    let notes = notes_from_activations(&onsets, &frames, N_FRAMES, &plain_options());
    assert_eq!(
        notes,
        vec![RawNote {
            start_frame: 10,
            end_frame: 55,
            pitch_bin: 40
        }]
    );
}

#[test]
fn energy_dip_admits_a_re_strike_at_the_plain_threshold() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 40] = 0.9;
    onsets[30 * PITCH_BINS + 40] = 0.6; // under the strict bar, over the plain one
    set(&mut frames, 10..55, 40, 0.8);
    // The string decays just before the second strike - still above the
    // frame threshold, so the pitch never stops "sounding".
    set(&mut frames, 26..30, 40, 0.4);
    let notes = notes_from_activations(
        &onsets,
        &frames,
        N_FRAMES,
        &NoteCreationOptions {
            retrigger_dip_ratio: 0.7,
            ..plain_options()
        },
    );
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].start_frame, 30);
    assert_eq!(notes[1].end_frame, 30);
}

#[test]
fn flat_energy_keeps_the_strict_re_articulation_bar() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 40] = 0.9;
    onsets[30 * PITCH_BINS + 40] = 0.6; // ripple, no decay before it
    set(&mut frames, 10..55, 40, 0.8);
    let notes = notes_from_activations(
        &onsets,
        &frames,
        N_FRAMES,
        &NoteCreationOptions {
            retrigger_dip_ratio: 0.7,
            ..plain_options()
        },
    );
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].end_frame, 55);
}

#[test]
fn strong_onset_peak_re_articulates_a_sounding_note() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 40] = 0.9;
    onsets[30 * PITCH_BINS + 40] = 0.8; // the string is struck again
    set(&mut frames, 10..55, 40, 0.8);
    let notes = notes_from_activations(&onsets, &frames, N_FRAMES, &plain_options());
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].start_frame, 30);
    assert_eq!(notes[1].start_frame, 10);
    assert_eq!(notes[1].end_frame, 30);
}

#[test]
fn a_pitch_that_went_quiet_starts_a_note_at_the_plain_threshold() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 40] = 0.9;
    set(&mut frames, 10..25, 40, 0.8);
    // Silence long enough to clear the lookback, then a soft second note
    // whose peak is under the re-articulation bar but over the plain one.
    onsets[35 * PITCH_BINS + 40] = 0.6;
    set(&mut frames, 35..55, 40, 0.8);
    let notes = notes_from_activations(&onsets, &frames, N_FRAMES, &plain_options());
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].start_frame, 35);
    assert_eq!(notes[1].start_frame, 10);
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
    // can start - unless some real onset exists to scale against.
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
fn inferred_ripple_does_not_re_articulate_a_sounding_note() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 40] = 0.9;
    // The pitch keeps sounding but its frame energy steps up mid-note - the
    // kind of ripple a dense texture causes. The inferred-onset rise at
    // frame 25 clears the retrigger bar once rescaled, but the network's own
    // onset there is zero, so the note must stay whole.
    set(&mut frames, 10..25, 40, 0.4);
    set(&mut frames, 25..55, 40, 0.9);
    let notes = notes_from_activations(
        &onsets,
        &frames,
        N_FRAMES,
        &NoteCreationOptions {
            infer_onsets: true,
            ..plain_options()
        },
    );
    assert_eq!(
        notes,
        vec![RawNote {
            start_frame: 10,
            end_frame: 55,
            pitch_bin: 40
        }]
    );
}

#[test]
fn network_onset_re_articulates_with_inference_on() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 40] = 0.9;
    onsets[25 * PITCH_BINS + 40] = 0.8; // the network saw a real re-strike
    set(&mut frames, 10..25, 40, 0.4);
    set(&mut frames, 25..55, 40, 0.9);
    let notes = notes_from_activations(
        &onsets,
        &frames,
        N_FRAMES,
        &NoteCreationOptions {
            infer_onsets: true,
            ..plain_options()
        },
    );
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].start_frame, 25);
    assert_eq!(notes[1].start_frame, 10);
}

#[test]
fn simultaneous_octave_above_strike_does_not_re_articulate() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 40] = 0.9;
    set(&mut frames, 10..55, 40, 0.8);
    // A strong onset ripple mid-note - but the network fired just as hard an
    // octave above at the same moment: that strike explains the ripple.
    onsets[30 * PITCH_BINS + 40] = 0.8;
    onsets[30 * PITCH_BINS + 52] = 0.9;
    let notes = notes_from_activations(
        &onsets,
        &frames,
        N_FRAMES,
        &NoteCreationOptions {
            retrigger_octave_veto: 1.0,
            ..plain_options()
        },
    );
    assert_eq!(
        notes,
        vec![RawNote {
            start_frame: 10,
            end_frame: 55,
            pitch_bin: 40
        }]
    );
}

#[test]
fn octave_re_strike_with_a_dominant_own_onset_survives() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 40] = 0.9;
    set(&mut frames, 10..55, 40, 0.8);
    // The re-strike's own onset outweighs the one above - a real repeat.
    onsets[30 * PITCH_BINS + 40] = 0.95;
    onsets[30 * PITCH_BINS + 52] = 0.9;
    let notes = notes_from_activations(
        &onsets,
        &frames,
        N_FRAMES,
        &NoteCreationOptions {
            retrigger_octave_veto: 1.0,
            ..plain_options()
        },
    );
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].start_frame, 30);
    assert_eq!(notes[1].start_frame, 10);
}

#[test]
fn weak_sub_octave_onset_note_under_a_covering_note_is_dropped() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 52] = 0.9;
    set(&mut frames, 10..40, 52, 0.8);
    // The network also fires an onset at the sub-octave, but its frame
    // energy is a fraction of the note above - leaked salience.
    onsets[10 * PITCH_BINS + 40] = 0.9;
    set(&mut frames, 10..40, 40, 0.35);
    let notes = notes_from_activations(
        &onsets,
        &frames,
        N_FRAMES,
        &NoteCreationOptions {
            onset_ghost_energy_ratio: 0.8,
            ..plain_options()
        },
    );
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].pitch_bin, 52);
}

#[test]
fn a_loud_octave_doubling_survives_the_ghost_filter() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 52] = 0.9;
    set(&mut frames, 10..40, 52, 0.8);
    // A real doubled bass octave holds its own energy.
    onsets[10 * PITCH_BINS + 40] = 0.9;
    set(&mut frames, 10..40, 40, 0.8);
    let notes = notes_from_activations(
        &onsets,
        &frames,
        N_FRAMES,
        &NoteCreationOptions {
            onset_ghost_energy_ratio: 0.8,
            ..plain_options()
        },
    );
    assert_eq!(notes.len(), 2);
    assert!(notes.iter().any(|note| note.pitch_bin == 40));
}

#[test]
fn weak_octave_above_harmonic_is_dropped() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 40] = 0.9;
    set(&mut frames, 10..40, 40, 0.8);
    // The string's 2nd harmonic shows up an octave up: own onset, mirrored
    // span, a fraction of the energy.
    onsets[10 * PITCH_BINS + 52] = 0.9;
    set(&mut frames, 10..40, 52, 0.4);
    let notes = notes_from_activations(
        &onsets,
        &frames,
        N_FRAMES,
        &NoteCreationOptions {
            overtone_ghost_energy_ratio: 0.6,
            ..plain_options()
        },
    );
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].pitch_bin, 40);
}

#[test]
fn a_real_octave_above_voice_survives_the_overtone_filter() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 40] = 0.9;
    set(&mut frames, 10..40, 40, 0.8);
    // A genuinely played upper octave carries its own energy.
    onsets[10 * PITCH_BINS + 52] = 0.9;
    set(&mut frames, 10..40, 52, 0.7);
    let notes = notes_from_activations(
        &onsets,
        &frames,
        N_FRAMES,
        &NoteCreationOptions {
            overtone_ghost_energy_ratio: 0.6,
            ..plain_options()
        },
    );
    assert_eq!(notes.len(), 2);
    assert!(notes.iter().any(|note| note.pitch_bin == 52));
}

#[test]
fn weak_fifth_partial_two_octaves_and_a_third_up_is_dropped() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 40] = 0.9;
    set(&mut frames, 10..40, 40, 0.8);
    // The 5th partial lands 28 semitones up: own onset, mirrored span, a
    // fraction of the energy.
    onsets[10 * PITCH_BINS + 68] = 0.9;
    set(&mut frames, 10..40, 68, 0.4);
    let notes = notes_from_activations(
        &onsets,
        &frames,
        N_FRAMES,
        &NoteCreationOptions {
            overtone_ghost_energy_ratio: 0.6,
            ..plain_options()
        },
    );
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].pitch_bin, 40);
}

#[test]
fn a_real_voice_two_octaves_and_a_third_up_survives_the_overtone_filter() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 40] = 0.9;
    set(&mut frames, 10..40, 40, 0.8);
    // A genuinely played upper voice carries its own energy.
    onsets[10 * PITCH_BINS + 68] = 0.9;
    set(&mut frames, 10..40, 68, 0.7);
    let notes = notes_from_activations(
        &onsets,
        &frames,
        N_FRAMES,
        &NoteCreationOptions {
            overtone_ghost_energy_ratio: 0.6,
            ..plain_options()
        },
    );
    assert_eq!(notes.len(), 2);
    assert!(notes.iter().any(|note| note.pitch_bin == 68));
}

#[test]
fn melodia_skips_the_sub_octave_shadow_of_a_claimed_note() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 52] = 0.9;
    set(&mut frames, 10..40, 52, 0.8);
    // Leaked salience an octave below, mirroring the claimed note's span.
    set(&mut frames, 12..38, 40, 0.6);
    let notes = notes_from_activations(
        &onsets,
        &frames,
        N_FRAMES,
        &NoteCreationOptions {
            melodia_trick: true,
            ..plain_options()
        },
    );
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].pitch_bin, 52);
}

#[test]
fn melodia_keeps_a_bass_note_with_its_own_extent() {
    let mut onsets = matrix();
    let mut frames = matrix();
    onsets[10 * PITCH_BINS + 52] = 0.9;
    set(&mut frames, 10..25, 52, 0.8);
    // An octave below, but sounding far beyond the upper note - a real bass
    // note, not its shadow.
    set(&mut frames, 5..58, 40, 0.6);
    let notes = notes_from_activations(
        &onsets,
        &frames,
        N_FRAMES,
        &NoteCreationOptions {
            melodia_trick: true,
            ..plain_options()
        },
    );
    assert_eq!(notes.len(), 2);
    assert!(notes.iter().any(|note| note.pitch_bin == 40));
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
