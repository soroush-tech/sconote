//! Onset/frame probability matrices → discrete notes.
//!
//! Faithful port of Basic Pitch's `output_to_notes_polyphonic`
//! (basic_pitch/note_creation.py): onset peaks start notes that track
//! forward through the frame matrix; consumed energy is zeroed (including
//! adjacent semitones); the "melodia trick" then mines what energy remains
//! for notes whose onsets the network missed. The thresholds here are the
//! main tuning knobs against real recordings.
//!
//! All matrices are row-major `[frame][pitch]` with [`PITCH_BINS`] columns.

use crate::model::PITCH_BINS;

/// Tuning knobs, defaults per Basic Pitch's reference implementation.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteCreationOptions {
    /// Minimum onset-peak probability that starts a note.
    pub onset_threshold: f32,
    /// Minimum frame probability for a pitch to count as still sounding.
    pub frame_threshold: f32,
    /// Notes must span strictly more frames than this (11 ≈ 128 ms).
    pub min_note_len_frames: usize,
    /// Consecutive sub-threshold frames tolerated inside a note.
    pub energy_tolerance_frames: usize,
    /// Derive extra onsets from sharp rises in frame energy.
    pub infer_onsets: bool,
    /// Mine leftover frame energy for notes without detected onsets.
    pub melodia_trick: bool,
}

impl Default for NoteCreationOptions {
    fn default() -> NoteCreationOptions {
        NoteCreationOptions {
            onset_threshold: 0.5,
            // Basic Pitch's reference default. Tuning on real room
            // recordings showed 0.2 buys ~0.01 F1 there but costs ~0.08 on
            // clean audio — not worth changing; lower it per-recording for
            // noisy rooms instead.
            frame_threshold: 0.3,
            min_note_len_frames: 11,
            energy_tolerance_frames: 11,
            infer_onsets: true,
            melodia_trick: true,
        }
    }
}

/// A note in frame/bin coordinates; `end_frame` is exclusive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawNote {
    pub start_frame: usize,
    pub end_frame: usize,
    pub pitch_bin: usize,
}

/// Extract notes from stitched onset and frame matrices of `n_frames` rows.
pub fn notes_from_activations(
    onsets: &[f32],
    frames: &[f32],
    n_frames: usize,
    options: &NoteCreationOptions,
) -> Vec<RawNote> {
    let onsets = if options.infer_onsets {
        inferred_onsets(onsets, frames, n_frames)
    } else {
        onsets.to_vec()
    };
    let mut remaining = frames.to_vec();
    let mut notes = Vec::new();

    // Onset peaks: local maxima over time, at or above threshold, visited in
    // reverse row-major order (mirrors the reference implementation — the
    // visit order matters because notes consume energy as they are claimed).
    let mut peaks = Vec::new();
    for frame in 1..n_frames.saturating_sub(1) {
        for bin in 0..PITCH_BINS {
            let value = onsets[frame * PITCH_BINS + bin];
            if value >= options.onset_threshold
                && value > onsets[(frame - 1) * PITCH_BINS + bin]
                && value > onsets[(frame + 1) * PITCH_BINS + bin]
            {
                peaks.push((frame, bin));
            }
        }
    }
    for &(start_frame, bin) in peaks.iter().rev() {
        if start_frame >= n_frames - 1 {
            continue;
        }
        // Track forward until the pitch stays quiet for the whole tolerance.
        let mut i = start_frame + 1;
        let mut quiet = 0;
        while i < n_frames - 1 && quiet < options.energy_tolerance_frames {
            if remaining[i * PITCH_BINS + bin] < options.frame_threshold {
                quiet += 1;
            } else {
                quiet = 0;
            }
            i += 1;
        }
        i -= quiet; // back to the last frame above threshold
        if i - start_frame <= options.min_note_len_frames {
            continue;
        }
        for frame in start_frame..i {
            zero_with_neighbors(&mut remaining, frame, bin);
        }
        notes.push(RawNote {
            start_frame,
            end_frame: i,
            pitch_bin: bin,
        });
    }

    if options.melodia_trick {
        mine_remaining_energy(&mut remaining, n_frames, options, &mut notes);
    }
    notes
}

/// The "melodia trick": repeatedly take the loudest leftover cell and grow a
/// note around it in both directions, until nothing exceeds the threshold.
fn mine_remaining_energy(
    remaining: &mut [f32],
    n_frames: usize,
    options: &NoteCreationOptions,
    notes: &mut Vec<RawNote>,
) {
    let energy_tolerance = options.energy_tolerance_frames;
    while let Some((peak_index, _)) = remaining
        .iter()
        .enumerate()
        .filter(|&(_, &value)| value > options.frame_threshold)
        .max_by(|a, b| a.1.total_cmp(b.1).then(b.0.cmp(&a.0)))
    {
        let (mid_frame, bin) = (peak_index / PITCH_BINS, peak_index % PITCH_BINS);
        remaining[peak_index] = 0.0;

        // Forward pass, zeroing consumed energy as it goes.
        let mut i = mid_frame + 1;
        let mut quiet = 0;
        while i < n_frames - 1 && quiet < energy_tolerance {
            if remaining[i * PITCH_BINS + bin] < options.frame_threshold {
                quiet += 1;
            } else {
                quiet = 0;
            }
            zero_with_neighbors(remaining, i, bin);
            i += 1;
        }
        let end_frame = i - 1 - quiet;

        // Backward pass (isize: the walk may step past frame 0).
        let mut i = mid_frame as isize - 1;
        let mut quiet = 0;
        while i > 0 && quiet < energy_tolerance {
            if remaining[i as usize * PITCH_BINS + bin] < options.frame_threshold {
                quiet += 1;
            } else {
                quiet = 0;
            }
            zero_with_neighbors(remaining, i as usize, bin);
            i -= 1;
        }
        let start_frame = (i + 1 + quiet as isize) as usize;

        if end_frame.saturating_sub(start_frame) <= options.min_note_len_frames {
            continue;
        }
        notes.push(RawNote {
            start_frame,
            end_frame,
            pitch_bin: bin,
        });
    }
}

fn zero_with_neighbors(remaining: &mut [f32], frame: usize, bin: usize) {
    remaining[frame * PITCH_BINS + bin] = 0.0;
    if bin + 1 < PITCH_BINS {
        remaining[frame * PITCH_BINS + bin + 1] = 0.0;
    }
    if bin > 0 {
        remaining[frame * PITCH_BINS + bin - 1] = 0.0;
    }
}

/// Merge the network's onsets with onsets inferred from sharp rises in frame
/// energy (elementwise max after rescaling the rises to onset range).
fn inferred_onsets(onsets: &[f32], frames: &[f32], n_frames: usize) -> Vec<f32> {
    const N_DIFF: usize = 2;
    let mut frame_diff = vec![0.0_f32; n_frames * PITCH_BINS];
    for frame in N_DIFF..n_frames {
        for bin in 0..PITCH_BINS {
            let current = frames[frame * PITCH_BINS + bin];
            let rise = (1..=N_DIFF)
                .map(|n| current - frames[(frame - n) * PITCH_BINS + bin])
                .fold(f32::INFINITY, f32::min);
            frame_diff[frame * PITCH_BINS + bin] = rise.max(0.0);
        }
    }
    let max_onset = onsets.iter().copied().fold(0.0, f32::max);
    let max_diff = frame_diff.iter().copied().fold(0.0, f32::max);
    if max_diff > 0.0 {
        let scale = max_onset / max_diff;
        for value in &mut frame_diff {
            *value *= scale;
        }
    }
    onsets
        .iter()
        .zip(&frame_diff)
        .map(|(&onset, &rise)| onset.max(rise))
        .collect()
}

#[cfg(test)]
#[path = "note_creation_test.rs"]
mod note_creation_test;
