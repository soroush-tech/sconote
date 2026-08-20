//! Noise-filtered note tracking on top of [`PitchEngine`].
//!
//! A tuner wants every raw detection; a note history wants only the notes the
//! player actually meant. [`NoteTracker`] provides both from one stream of
//! PCM chunks: the raw per-window event passes through untouched, while a
//! clarity gate plus a consecutive-window debounce turns attack transients,
//! background noise, and settling vibrato into a clean "note started" stream.

use crate::{NoteEvent, PitchEngine};

/// Minimum clarity for a window to count toward a tracked note - stricter
/// than the engine's detection threshold, so borderline windows still show
/// on a tuner but never enter the note history.
const MIN_TRACK_CLARITY: f32 = 0.8;

/// Consecutive agreeing windows before a note is confirmed, and consecutive
/// quiet windows before the current note counts as released. Three 2048-
/// sample windows at 48 kHz ≈ 130 ms.
const HOLD_WINDOWS: u32 = 3;

/// Result of feeding one chunk to [`NoteTracker::process`].
#[derive(Debug, Clone, PartialEq)]
pub struct TrackerUpdate {
    /// Raw per-window detection, exactly what [`PitchEngine::process`] would
    /// have returned for this chunk - drive a live display with this.
    pub live: Option<NoteEvent>,
    /// Present at most once per held note: the pitch was stable for
    /// [`HOLD_WINDOWS`] windows - append this to a note history.
    pub note_started: Option<NoteEvent>,
}

/// Streaming note tracker: a [`PitchEngine`] plus debouncing that turns noisy
/// per-window detections into a stream of confirmed instrument notes.
///
/// A note is confirmed when the same MIDI pitch holds for [`HOLD_WINDOWS`]
/// consecutive windows at ≥ [`MIN_TRACK_CLARITY`]; it is released after the
/// same number of quiet windows, after which the identical pitch can be
/// confirmed again as a new note. Short dropouts neither release the note
/// nor duplicate it.
pub struct NoteTracker {
    engine: PitchEngine,
    /// Pitch currently accumulating agreeing windows, and how many so far.
    candidate_midi: Option<i32>,
    candidate_windows: u32,
    /// Windows since the last clear detection.
    quiet_windows: u32,
    /// Last confirmed, not-yet-released note.
    current_midi: Option<i32>,
}

impl NoteTracker {
    /// Same contract as [`PitchEngine::new`].
    pub fn new(sample_rate: u32, window_size: usize) -> NoteTracker {
        NoteTracker {
            engine: PitchEngine::new(sample_rate, window_size),
            candidate_midi: None,
            candidate_windows: 0,
            quiet_windows: 0,
            current_midi: None,
        }
    }

    /// Feed a chunk of mono f32 samples in [-1, 1].
    pub fn process(&mut self, chunk: &[f32]) -> TrackerUpdate {
        // The debounce counts analysis windows, not calls; most calls only
        // accumulate samples. Mirrors the engine's own completion condition.
        let completes_window =
            self.engine.buffered_len() + chunk.len() >= self.engine.window_size();
        let live = self.engine.process(chunk);
        let note_started = if completes_window {
            self.observe_window(live.as_ref())
        } else {
            None
        };
        TrackerUpdate { live, note_started }
    }

    fn observe_window(&mut self, event: Option<&NoteEvent>) -> Option<NoteEvent> {
        let Some(event) = event.filter(|event| event.clarity >= MIN_TRACK_CLARITY) else {
            self.candidate_midi = None;
            self.candidate_windows = 0;
            self.quiet_windows += 1;
            if self.quiet_windows >= HOLD_WINDOWS {
                self.current_midi = None;
            }
            return None;
        };
        self.quiet_windows = 0;
        if self.candidate_midi == Some(event.midi) {
            self.candidate_windows += 1;
        } else {
            self.candidate_midi = Some(event.midi);
            self.candidate_windows = 1;
        }
        if self.candidate_windows >= HOLD_WINDOWS && self.current_midi != Some(event.midi) {
            self.current_midi = Some(event.midi);
            return Some(event.clone());
        }
        None
    }
}

#[cfg(test)]
#[path = "tracker_test.rs"]
mod tracker_test;

#[cfg(test)]
#[path = "tracker_spec.rs"]
mod tracker_spec;
