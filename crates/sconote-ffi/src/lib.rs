//! UniFFI bindings for iOS/Android (Swift/Kotlin). Thin wrapper over
//! `sconote-core` (live tuner/tracker) and `sconote-poly` (offline
//! polyphonic transcription).

use std::sync::Mutex;

uniffi::setup_scaffolding!();

/// A pitch detected in one analysis window.
#[derive(uniffi::Record)]
pub struct NoteEvent {
    pub frequency_hz: f32,
    /// Nearest MIDI note number (A4 = 69).
    pub midi: i32,
    /// Scientific pitch name, e.g. "A4".
    pub note_name: String,
    /// Deviation from the named note in cents, in (-50, 50].
    pub cents_offset: f32,
    /// Detection confidence, 0..1.
    pub clarity: f32,
}

impl From<sconote_core::NoteEvent> for NoteEvent {
    fn from(event: sconote_core::NoteEvent) -> NoteEvent {
        NoteEvent {
            frequency_hz: event.frequency_hz,
            midi: event.midi,
            note_name: event.note_name,
            cents_offset: event.cents_offset,
            clarity: event.clarity,
        }
    }
}

/// Streaming pitch detector. Feed it PCM chunks from the platform's audio
/// capture; it emits a NoteEvent per full analysis window.
#[derive(uniffi::Object)]
pub struct PitchDetector {
    engine: Mutex<sconote_core::PitchEngine>,
}

#[uniffi::export]
impl PitchDetector {
    #[uniffi::constructor]
    pub fn new(sample_rate: u32, window_size: u32) -> PitchDetector {
        PitchDetector {
            engine: Mutex::new(sconote_core::PitchEngine::new(
                sample_rate,
                window_size as usize,
            )),
        }
    }

    pub fn process(&self, samples: Vec<f32>) -> Option<NoteEvent> {
        self.engine
            .lock()
            .expect("engine lock poisoned")
            .process(&samples)
            .map(NoteEvent::from)
    }
}

/// Result of feeding one chunk to [`NoteTracker::process`].
#[derive(uniffi::Record)]
pub struct TrackerUpdate {
    /// Raw per-window detection — drive a live display (tuner) with this.
    pub live: Option<NoteEvent>,
    /// Present at most once per held note — append to a note history.
    pub note_started: Option<NoteEvent>,
}

/// Streaming note tracker: pitch detection plus debouncing that turns noisy
/// per-window detections into a stream of confirmed instrument notes.
#[derive(uniffi::Object)]
pub struct NoteTracker {
    tracker: Mutex<sconote_core::NoteTracker>,
}

#[uniffi::export]
impl NoteTracker {
    #[uniffi::constructor]
    pub fn new(sample_rate: u32, window_size: u32) -> NoteTracker {
        NoteTracker {
            tracker: Mutex::new(sconote_core::NoteTracker::new(
                sample_rate,
                window_size as usize,
            )),
        }
    }

    pub fn process(&self, samples: Vec<f32>) -> TrackerUpdate {
        let update = self
            .tracker
            .lock()
            .expect("tracker lock poisoned")
            .process(&samples);
        TrackerUpdate {
            live: update.live.map(NoteEvent::from),
            note_started: update.note_started.map(NoteEvent::from),
        }
    }
}

/// One note from an offline polyphonic transcription.
#[derive(uniffi::Record)]
pub struct PolyphonicNote {
    /// MIDI note number (A4 = 69).
    pub midi: u8,
    /// Note start, seconds from the beginning of the recording.
    pub onset_s: f64,
    /// Note end, seconds from the beginning of the recording.
    pub offset_s: f64,
}

/// Offline polyphonic transcriber (the Basic Pitch CNN, embedded). Create
/// once, reuse across recordings. Transcription is CPU-bound — call it from
/// a background thread/queue, not the UI thread.
#[derive(uniffi::Object)]
pub struct Transcriber {
    model: sconote_poly::BasicPitch,
}

impl Default for Transcriber {
    fn default() -> Transcriber {
        // The model is embedded in the binary; failing to load it is a
        // build defect, not a runtime condition callers can handle.
        Transcriber {
            model: sconote_poly::BasicPitch::new().expect("embedded model must load"),
        }
    }
}

#[uniffi::export]
impl Transcriber {
    #[uniffi::constructor]
    pub fn new() -> Transcriber {
        Transcriber::default()
    }

    /// Transcribe a whole mono recording (any sample rate) into notes,
    /// sorted by onset. Thresholds 0.5 / 0.3 are the reference defaults.
    pub fn transcribe(
        &self,
        samples: Vec<f32>,
        sample_rate: u32,
        onset_threshold: f32,
        frame_threshold: f32,
    ) -> Vec<PolyphonicNote> {
        let audio = sconote_poly::MonoAudio {
            samples,
            sample_rate,
        };
        let options = sconote_poly::NoteCreationOptions {
            onset_threshold,
            frame_threshold,
            ..sconote_poly::NoteCreationOptions::default()
        };
        sconote_poly::transcribe(&audio, &self.model, &options)
            // Inference on an embedded model cannot fail on valid input.
            .expect("transcription failed")
            .into_iter()
            .map(|note| PolyphonicNote {
                midi: note.midi,
                onset_s: note.onset_s,
                offset_s: note.offset_s,
            })
            .collect()
    }
}

/// Encode transcribed notes as a Standard MIDI File (120 BPM grid) — save
/// as `.mid`.
#[uniffi::export]
pub fn notes_to_midi(notes: Vec<PolyphonicNote>) -> Vec<u8> {
    sconote_poly::notes_to_midi_bytes(&core_notes(&notes))
}

/// Tempo estimated from the note onsets (BPM).
#[uniffi::export]
pub fn estimate_bpm(notes: Vec<PolyphonicNote>) -> f64 {
    sconote_poly::estimate_bpm(&core_notes(&notes))
}

/// Render transcribed notes as a MusicXML score (single grand-staff part)
/// for a sheet music renderer. `bpm` `None` → estimated from the notes.
#[uniffi::export]
pub fn notes_to_musicxml(
    notes: Vec<PolyphonicNote>,
    part_name: String,
    bpm: Option<f64>,
) -> String {
    let notes = core_notes(&notes);
    let bpm = bpm.unwrap_or_else(|| sconote_poly::estimate_bpm(&notes));
    sconote_poly::parts_to_musicxml(
        &[sconote_poly::ScorePart {
            name: part_name,
            notes,
        }],
        bpm,
    )
}

fn core_notes(notes: &[PolyphonicNote]) -> Vec<sconote_poly::TranscribedNote> {
    notes
        .iter()
        .map(|note| sconote_poly::TranscribedNote {
            midi: note.midi,
            onset_s: note.onset_s,
            offset_s: note.offset_s,
        })
        .collect()
}
