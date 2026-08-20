//! Sconote core: monophonic pitch detection from PCM audio frames.
//!
//! Platform-agnostic by design - audio capture is the caller's job (Web Audio
//! on the web, native audio on mobile). Feed [`PitchEngine::process`] chunks of
//! f32 samples in [-1, 1] and it emits a [`NoteEvent`] whenever a full analysis
//! window has accumulated and a clear pitch is present.

mod note;
mod tracker;

#[cfg(test)]
mod test_signals;

pub use note::{NOTE_NAMES, Note, midi_from_frequency};
pub use tracker::{NoteTracker, TrackerUpdate};

use pitch_detection::detector::PitchDetector as _;
use pitch_detection::detector::mcleod::McLeodDetector;

/// Minimum MPM clarity (0..1) for a detection to count as a real pitch.
const CLARITY_THRESHOLD: f32 = 0.7;
/// Minimum signal power for a window to be analyzed at all (rejects silence).
const POWER_THRESHOLD: f32 = 0.15;

/// A pitch detected in one analysis window.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteEvent {
    pub frequency_hz: f32,
    /// Nearest MIDI note number (A4 = 69).
    pub midi: i32,
    /// Scientific pitch name, e.g. "A4".
    pub note_name: String,
    /// Deviation from the named note in cents, in (-50, 50].
    pub cents_offset: f32,
    /// MPM clarity of the detection, 0..1 (higher = more confident).
    pub clarity: f32,
}

impl NoteEvent {
    fn new(frequency_hz: f32, clarity: f32) -> NoteEvent {
        let note = Note::from_frequency(frequency_hz);
        NoteEvent {
            frequency_hz,
            midi: note.midi,
            note_name: note.name(),
            cents_offset: note.cents_offset,
            clarity,
        }
    }
}

/// `McLeodDetector` is `!Send` only because its internal scratch buffers are
/// `Rc<RefCell<...>>`. Those buffers are created by the detector, used inside
/// `get_pitch`, and no clone of them ever escapes it.
struct SendDetector(McLeodDetector<f32>);

// SAFETY: every Rc reference lives inside the detector, so moving the whole
// detector to another thread moves all of them together; no cross-thread
// aliasing is possible. (`&self` access still requires external
// synchronization - SendDetector is deliberately not Sync.)
unsafe impl Send for SendDetector {}

/// Streaming pitch detector (McLeod Pitch Method).
///
/// Accumulates arbitrarily-sized input chunks into windows of `window_size`
/// samples; each full window is analyzed and then discarded.
pub struct PitchEngine {
    sample_rate: usize,
    window_size: usize,
    buffer: Vec<f32>,
    detector: SendDetector,
}

impl PitchEngine {
    /// `window_size` is the analysis window in samples; 2048 at 44.1/48 kHz is
    /// a good default (≈43 ms, reaches down to ~E2 on a guitar).
    pub fn new(sample_rate: u32, window_size: usize) -> PitchEngine {
        PitchEngine {
            sample_rate: sample_rate as usize,
            window_size,
            buffer: Vec::with_capacity(window_size * 2),
            detector: SendDetector(McLeodDetector::new(window_size, window_size / 2)),
        }
    }

    /// Feed a chunk of mono f32 samples in [-1, 1].
    ///
    /// Returns a [`NoteEvent`] when a window completed AND a clear pitch was
    /// found in it; `None` while accumulating, on silence, or on unpitched
    /// input.
    pub fn process(&mut self, chunk: &[f32]) -> Option<NoteEvent> {
        self.buffer.extend_from_slice(chunk);
        if self.buffer.len() < self.window_size {
            return None;
        }
        let frame_start = self.buffer.len() - self.window_size;
        let frame = self.buffer[frame_start..].to_vec();
        self.buffer.clear();
        let pitch = self.detector.0.get_pitch(
            &frame,
            self.sample_rate,
            POWER_THRESHOLD,
            CLARITY_THRESHOLD,
        )?;
        Some(NoteEvent::new(pitch.frequency, pitch.clarity))
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate as u32
    }

    pub fn window_size(&self) -> usize {
        self.window_size
    }

    /// Samples accumulated toward the next analysis window - lets the
    /// tracker mirror the window-completion condition above.
    pub(crate) fn buffered_len(&self) -> usize {
        self.buffer.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_signals::{SAMPLE_RATE, WINDOW, sine};
    use std::f32::consts::TAU;

    /// Harmonic-rich signal (fundamental + 5 harmonics, sawtooth-like).
    fn sawtooth(frequency: f32, samples: usize) -> Vec<f32> {
        (0..samples)
            .map(|i| {
                let t = i as f32 / SAMPLE_RATE as f32;
                (1..=6)
                    .map(|h| (TAU * frequency * h as f32 * t).sin() / h as f32)
                    .sum::<f32>()
            })
            .collect()
    }

    fn detect(signal: &[f32]) -> Option<NoteEvent> {
        let mut engine = PitchEngine::new(SAMPLE_RATE, WINDOW);
        engine.process(signal)
    }

    #[test]
    fn detects_a4_from_sine() {
        let event = detect(&sine(440.0, WINDOW)).expect("pitch expected");
        assert_eq!(event.note_name, "A4");
        assert!((event.frequency_hz - 440.0).abs() < 2.0);
        assert!(event.clarity > 0.9);
    }

    #[test]
    fn detects_fundamental_not_harmonics_in_rich_tone() {
        let event = detect(&sawtooth(220.0, WINDOW)).expect("pitch expected");
        assert_eq!(event.note_name, "A3");
    }

    #[test]
    fn detects_low_guitar_e2() {
        let event = detect(&sine(82.407, WINDOW)).expect("pitch expected");
        assert_eq!(event.note_name, "E2");
    }

    #[test]
    fn reports_configuration() {
        let engine = PitchEngine::new(SAMPLE_RATE, WINDOW);
        assert_eq!(engine.sample_rate(), SAMPLE_RATE);
        assert_eq!(engine.window_size(), WINDOW);
    }

    #[test]
    fn engine_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<PitchEngine>();
    }

    #[test]
    fn silence_yields_nothing() {
        assert_eq!(detect(&vec![0.0; WINDOW]), None);
    }

    #[test]
    fn accumulates_small_chunks_until_window_full() {
        let mut engine = PitchEngine::new(SAMPLE_RATE, WINDOW);
        let signal = sine(440.0, WINDOW);
        // Web Audio-style 128-sample chunks: nothing until the window fills.
        let mut events = Vec::new();
        for chunk in signal.chunks(128) {
            if let Some(event) = engine.process(chunk) {
                events.push(event);
            }
        }
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].note_name, "A4");
    }
}
