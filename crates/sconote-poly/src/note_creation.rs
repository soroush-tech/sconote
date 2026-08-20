//! Onset/frame probability matrices → discrete notes.
//!
//! Faithful port of Basic Pitch's `output_to_notes_polyphonic`
//! (basic_pitch/note_creation.py): onset peaks start notes that track
//! forward through the frame matrix; consumed energy is zeroed (including
//! adjacent semitones); the "melodia trick" then mines what energy remains
//! for notes whose onsets the network missed. The thresholds here are the
//! main tuning knobs against real recordings.
//!
//! Six documented deviations from the reference: re-articulating a
//! sounding pitch takes a peak the network itself saw
//! ([`NoteCreationOptions::retrigger_onset_threshold`]) - relaxed to the
//! plain threshold when the pitch's energy dipped just before, the mark of
//! a genuine re-strike ([`NoteCreationOptions::retrigger_dip_ratio`]) -
//! that is not explained by a simultaneous strike an octave or twelfth
//! above ([`NoteCreationOptions::retrigger_octave_veto`]), the melodia pass
//! drops
//! subharmonic shadows of already-claimed notes ([`is_subharmonic_ghost`]),
//! onset-started notes get the same filter with an energy test on top
//! ([`NoteCreationOptions::onset_ghost_energy_ratio`]), and finished notes
//! that are the weak 2nd-6th partial of a note below are dropped
//! ([`NoteCreationOptions::overtone_ghost_energy_ratio`]).
//!
//! All matrices are row-major `[frame][pitch]` with [`PITCH_BINS`] columns.

use crate::model::PITCH_BINS;

/// How far back a pitch must have been sounding without interruption for an
/// onset peak to count as re-articulating it rather than starting it fresh
/// (4 frames ≈ 46 ms).
const RETRIGGER_LOOKBACK_FRAMES: usize = 4;

/// Frames scanned backwards from an onset for the brief energy decay that
/// marks a genuine re-strike (7 frames ≈ 81 ms - within one fast note, so a
/// dip belonging to an earlier event cannot leak in).
const RETRIGGER_DIP_LOOKBACK_FRAMES: usize = 7;

/// Tuning knobs, defaults per Basic Pitch's reference implementation except
/// where a field's own docs say otherwise.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteCreationOptions {
    /// Minimum onset-peak probability that starts a note.
    pub onset_threshold: f32,
    /// Minimum onset-peak probability to start a note at a pitch that is
    /// *already sounding*. Not in Basic Pitch, which uses one threshold
    /// everywhere: under a dense chord a held note's probability ripples,
    /// and any resulting weak peak cuts it into two notes, because peaks are
    /// claimed newest-first and the later one takes the tail. Requiring a
    /// struck-string peak to re-articulate suppresses that without raising
    /// the bar for notes starting out of silence.
    ///
    /// The bar applies to the network's *own* onset probability: onsets
    /// inferred from frame-energy rises (see
    /// [`NoteCreationOptions::infer_onsets`]) may start notes out of
    /// silence, but a frame-energy ripple under a dense texture must never
    /// cut a held note in two.
    pub retrigger_onset_threshold: f32,
    /// A struck string decays briefly before sounding again; a held note's
    /// energy under a dense texture only ripples. When the pitch's frame
    /// energy within the last [`RETRIGGER_DIP_LOOKBACK_FRAMES`] frames
    /// dipped to at most this factor of its peak over the same window, a
    /// re-articulation is admitted at the plain
    /// [`NoteCreationOptions::onset_threshold`] instead of the strict
    /// [`NoteCreationOptions::retrigger_onset_threshold`] - the dip is the
    /// evidence the threshold alone was standing in for. `0` disables the
    /// dip test, leaving the strict bar everywhere.
    pub retrigger_dip_ratio: f32,
    /// Minimum frame probability for a pitch to count as still sounding.
    pub frame_threshold: f32,
    /// Veto a re-articulation of a sounding pitch when the network's onset
    /// at the octave or twelfth above (within ±2 frames) is at least this
    /// factor times the candidate's own - the strike above explains the
    /// ripple. `0` disables the veto.
    pub retrigger_octave_veto: f32,
    /// Drop onset-started notes whose span is covered by a note 12 or 19
    /// semitones above, when the candidate's mean frame energy is at most
    /// this factor times the upper note's - subharmonic leakage is much
    /// weaker than its source, while a genuinely doubled bass octave holds
    /// its own. `0` disables the filter.
    pub onset_ghost_energy_ratio: f32,
    /// The mirror image for real recordings: drop notes whose span is
    /// covered by a note an overtone interval *below* (2nd-6th partial:
    /// 12, 19, 24, 28, or 31 semitones), when the candidate's mean frame
    /// energy is at most this factor times the lower note's - a piano
    /// string's partial transcribed as its own note. Runs after all notes
    /// exist, so it also catches mined ones. `0` disables the filter.
    pub overtone_ghost_energy_ratio: f32,
    /// Notes must span strictly more frames than this (7 ≈ 81 ms).
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
            // Swept on a rendered Bach 846 against its own MIDI: 0.6 halves
            // the re-triggers, 0.7 is the F1 peak (0.81 → 0.92, recall
            // unchanged), 0.8+ starts eating genuine repeated notes - but
            // that was without the dip test below. With it, genuine repeats
            // pass on their dip, and re-sweeping on the real-piano MP3 moved
            // the knee to 0.8 (0.9 costs a point of recall for nothing).
            retrigger_onset_threshold: 0.8,
            // Swept jointly with the bar above on the real-piano MP3:
            // 0.6/0.8 is the F1 peak (0.971 → 0.973) - six more matched
            // notes than raising the bar alone, at the same precision.
            // 0.7 already admits chord-ripple as a dip (precision slides),
            // 0.5 admits almost nothing beyond the bar itself.
            retrigger_dip_ratio: 0.6,
            // Basic Pitch's reference default. Tuning on real room
            // recordings showed 0.2 buys ~0.01 F1 there but costs ~0.08 on
            // clean audio - not worth changing; lower it per-recording for
            // noisy rooms instead.
            frame_threshold: 0.3,
            // Swept on the rendered Bach 846: 0.8 is the knee - precision
            // 0.888 → 0.905 for one lost matched note; 1.0 buys a little
            // more F1 but starts calling equal-loudness octave doublings
            // ghosts. 0 disables the filter.
            onset_ghost_energy_ratio: 0.8,
            // Swept on both test recordings: 0.6 is the knee on each - the
            // real-piano MP3 gains 6.7 points of precision (0.883 → 0.950)
            // for 0.7 recall, the clean render gains 2 for 1; 0.8+ eats
            // genuine octave-above voices on both. Re-swept after widening
            // the filter to 4th-6th partials: still the knee (0.4 collapses
            // precision, 0.8 costs 3.5 points of recall).
            overtone_ghost_energy_ratio: 0.6,
            // Swept on the rendered Bach 846: 1.0 is the F1 peak (0.932 →
            // 0.943, splits 89 → 63) at a cost of 4 matched notes; 1.25
            // keeps recall exactly but only reaches 0.915 precision; 0.75
            // and below eat real repeated notes wholesale.
            retrigger_octave_veto: 1.0,
            // Basic Pitch's reference default is 11 (≈128 ms), which eats
            // fast notes: a detected span is shorter than the note's true
            // duration, so on the real-piano MP3 two thirds of all misses
            // were notes ≤150 ms. Swept jointly with the overtone ratio:
            // 7 is the F1 peak (0.959 → 0.970, recall 0.949 → 0.980);
            // 5 gains nothing more, 3 floods precision with short ghosts.
            min_note_len_frames: 7,
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
    let raw_onsets = onsets;
    let onsets = if options.infer_onsets {
        inferred_onsets(onsets, frames, n_frames)
    } else {
        onsets.to_vec()
    };
    let mut remaining = frames.to_vec();
    let mut notes = Vec::new();

    // Onset peaks: local maxima over time, at or above threshold, visited in
    // reverse row-major order (mirrors the reference implementation - the
    // visit order matters because notes consume energy as they are claimed).
    let mut peaks = Vec::new();
    for frame in 1..n_frames.saturating_sub(1) {
        for bin in 0..PITCH_BINS {
            let index = frame * PITCH_BINS + bin;
            let value = onsets[index];
            // A sounding pitch re-articulates only on the network's own
            // onset; a fresh note may also start from an inferred rise.
            let admitted = if still_sounding(frames, frame, bin, options.frame_threshold) {
                let bar = if energy_dipped(frames, frame, bin, options.retrigger_dip_ratio) {
                    options.onset_threshold
                } else {
                    options.retrigger_onset_threshold
                };
                raw_onsets[index] >= bar
                    && !upper_strike_veto(raw_onsets, frame, bin, n_frames, options)
            } else {
                value >= options.onset_threshold
            };
            if admitted
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

    if options.onset_ghost_energy_ratio > 0.0 {
        suppress_onset_ghosts(&mut notes, frames, options.onset_ghost_energy_ratio);
    }
    if options.melodia_trick {
        mine_remaining_energy(&mut remaining, n_frames, options, &mut notes);
    }
    if options.overtone_ghost_energy_ratio > 0.0 {
        suppress_overtone_ghosts(&mut notes, frames, options.overtone_ghost_energy_ratio);
    }
    notes
}

/// Drop notes that are a transcribed partial of a note an
/// [`OVERTONE_INTERVALS`] step below: span covered at least half by the
/// lower note, and mean frame energy at most `max_energy_ratio` times the
/// lower note's over the overlap. Runs on the finished note list
/// (onset-started and mined alike); a genuine octave-above voice keeps its
/// own energy and survives.
fn suppress_overtone_ghosts(notes: &mut Vec<RawNote>, frames: &[f32], max_energy_ratio: f32) {
    let mean_energy = |bin: usize, start: usize, end: usize| {
        if end <= start {
            return 0.0;
        }
        (start..end)
            .map(|f| frames[f * PITCH_BINS + bin])
            .sum::<f32>()
            / (end - start) as f32
    };
    let ghost = |candidate: &RawNote, all: &[RawNote]| {
        all.iter().any(|lower| {
            OVERTONE_INTERVALS
                .iter()
                .any(|&interval| candidate.pitch_bin == lower.pitch_bin + interval)
                && {
                    let overlap_start = lower.start_frame.max(candidate.start_frame);
                    let overlap_end = lower.end_frame.min(candidate.end_frame);
                    2 * overlap_end.saturating_sub(overlap_start)
                        >= candidate.end_frame - candidate.start_frame
                        && mean_energy(
                            candidate.pitch_bin,
                            candidate.start_frame,
                            candidate.end_frame,
                        ) <= max_energy_ratio
                            * mean_energy(lower.pitch_bin, overlap_start, overlap_end)
                }
        })
    };
    let ghosts: Vec<bool> = notes.iter().map(|note| ghost(note, notes)).collect();
    let mut index = 0;
    notes.retain(|_| {
        let drop = ghosts[index];
        index += 1;
        !drop
    });
}

/// Did the network fire at least `retrigger_octave_veto` times as strongly
/// at the octave or twelfth above within ±2 frames of this onset? If so the
/// strike above explains the ripple at this pitch.
fn upper_strike_veto(
    raw_onsets: &[f32],
    frame: usize,
    bin: usize,
    n_frames: usize,
    options: &NoteCreationOptions,
) -> bool {
    if options.retrigger_octave_veto <= 0.0 {
        return false;
    }
    let bar = options.retrigger_octave_veto * raw_onsets[frame * PITCH_BINS + bin];
    let window = frame.saturating_sub(2)..=(frame + 2).min(n_frames - 1);
    GHOST_INTERVALS.iter().any(|&interval| {
        let upper = bin + interval;
        upper < PITCH_BINS
            && window
                .clone()
                .any(|f| raw_onsets[f * PITCH_BINS + upper] >= bar)
    })
}

/// The onset-pass counterpart of [`is_subharmonic_ghost`], with an energy
/// test on top of span coverage so genuine octave doublings - whose bass is
/// comparably loud - survive while leaked subharmonics do not.
fn suppress_onset_ghosts(notes: &mut Vec<RawNote>, frames: &[f32], max_energy_ratio: f32) {
    let mean_energy = |bin: usize, start: usize, end: usize| {
        if end <= start {
            return 0.0;
        }
        (start..end)
            .map(|f| frames[f * PITCH_BINS + bin])
            .sum::<f32>()
            / (end - start) as f32
    };
    let ghost = |candidate: &RawNote, all: &[RawNote]| {
        all.iter().any(|upper| {
            GHOST_INTERVALS
                .iter()
                .any(|&interval| upper.pitch_bin == candidate.pitch_bin + interval)
                && {
                    let overlap_start = upper.start_frame.max(candidate.start_frame);
                    let overlap_end = upper.end_frame.min(candidate.end_frame);
                    2 * overlap_end.saturating_sub(overlap_start)
                        >= candidate.end_frame - candidate.start_frame
                        && mean_energy(
                            candidate.pitch_bin,
                            candidate.start_frame,
                            candidate.end_frame,
                        ) <= max_energy_ratio
                            * mean_energy(upper.pitch_bin, overlap_start, overlap_end)
                }
        })
    };
    let ghosts: Vec<bool> = notes.iter().map(|note| ghost(note, notes)).collect();
    let mut index = 0;
    notes.retain(|_| {
        let drop = ghosts[index];
        index += 1;
        !drop
    });
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
        if is_subharmonic_ghost(notes, start_frame, end_frame, bin) {
            continue;
        }
        notes.push(RawNote {
            start_frame,
            end_frame,
            pitch_bin: bin,
        });
    }
}

/// Subharmonic intervals, in semitone bins, at which the network leaks frame
/// salience below a loud note: the sub-octave (f/2) and sub-twelfth (f/3).
const GHOST_INTERVALS: [usize; 2] = [12, 19];

/// Overtone intervals, in semitone bins, at which a string's partials get
/// transcribed as notes of their own above the real one: octave (2nd
/// partial), twelfth (3rd), double octave (4th), double octave plus major
/// third (5th), double octave plus fifth (6th). Wider than
/// [`GHOST_INTERVALS`] because upward leakage reaches higher partials -
/// on the Bach 846 recording the 5th partial alone accounted for a third
/// of all spurious notes.
const OVERTONE_INTERVALS: [usize; 5] = [12, 19, 24, 28, 31];

/// Is a candidate mined note the subharmonic shadow of a note already
/// claimed? True when a note 12 or 19 semitones above covers at least half
/// of the candidate's span - leaked salience mirrors the upper note's span,
/// while a real bass line under a high voice keeps its own extent.
fn is_subharmonic_ghost(
    notes: &[RawNote],
    start_frame: usize,
    end_frame: usize,
    bin: usize,
) -> bool {
    notes.iter().any(|note| {
        GHOST_INTERVALS
            .iter()
            .any(|&interval| note.pitch_bin == bin + interval)
            && {
                let overlap = note
                    .end_frame
                    .min(end_frame)
                    .saturating_sub(note.start_frame.max(start_frame));
                2 * overlap >= end_frame - start_frame
            }
    })
}

/// Did the pitch's frame energy dip within the lookback window - the decay
/// of a string about to be struck again? Compares the window's minimum to
/// its maximum so the test tracks the note's own level rather than an
/// absolute bar.
fn energy_dipped(frames: &[f32], frame: usize, bin: usize, dip_ratio: f32) -> bool {
    if dip_ratio <= 0.0 {
        return false;
    }
    let start = frame.saturating_sub(RETRIGGER_DIP_LOOKBACK_FRAMES);
    let (mut min, mut max) = (f32::INFINITY, 0.0_f32);
    for f in start..=frame {
        let value = frames[f * PITCH_BINS + bin];
        min = min.min(value);
        max = max.max(value);
    }
    min <= dip_ratio * max
}

/// Was this pitch already ringing in the frames leading up to `frame`? Uses
/// the untouched frame matrix, not the energy left after earlier notes
/// claimed theirs.
fn still_sounding(frames: &[f32], frame: usize, bin: usize, frame_threshold: f32) -> bool {
    frame >= RETRIGGER_LOOKBACK_FRAMES
        && (frame - RETRIGGER_LOOKBACK_FRAMES..frame)
            .all(|f| frames[f * PITCH_BINS + bin] >= frame_threshold)
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
