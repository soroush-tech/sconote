//! wasm-bindgen bindings for the web. Thin wrapper over `sconote-core`
//! (live tuner/tracker) and `sconote-poly` (offline polyphonic
//! transcription).

use sconote_poly::{MonoAudio, NoteCreationOptions, WindowedTranscription};
use wasm_bindgen::prelude::*;

/// A pitch detected in one analysis window.
#[wasm_bindgen]
pub struct NoteEvent {
    inner: sconote_core::NoteEvent,
}

#[wasm_bindgen]
impl NoteEvent {
    #[wasm_bindgen(getter, js_name = frequencyHz)]
    pub fn frequency_hz(&self) -> f32 {
        self.inner.frequency_hz
    }

    #[wasm_bindgen(getter)]
    pub fn midi(&self) -> i32 {
        self.inner.midi
    }

    #[wasm_bindgen(getter, js_name = noteName)]
    pub fn note_name(&self) -> String {
        self.inner.note_name.clone()
    }

    #[wasm_bindgen(getter, js_name = centsOffset)]
    pub fn cents_offset(&self) -> f32 {
        self.inner.cents_offset
    }

    #[wasm_bindgen(getter)]
    pub fn clarity(&self) -> f32 {
        self.inner.clarity
    }
}

/// Streaming pitch detector. Feed it Float32Array chunks from an
/// AudioWorklet/AnalyserNode; it emits a NoteEvent per full analysis window.
#[wasm_bindgen]
pub struct PitchDetector {
    engine: sconote_core::PitchEngine,
}

#[wasm_bindgen]
impl PitchDetector {
    #[wasm_bindgen(constructor)]
    pub fn new(sample_rate: u32, window_size: usize) -> PitchDetector {
        PitchDetector {
            engine: sconote_core::PitchEngine::new(sample_rate, window_size),
        }
    }

    pub fn process(&mut self, samples: &[f32]) -> Option<NoteEvent> {
        self.engine
            .process(samples)
            .map(|inner| NoteEvent { inner })
    }
}

/// Result of feeding one chunk to `NoteTracker.process`.
#[wasm_bindgen]
pub struct TrackerUpdate {
    inner: sconote_core::TrackerUpdate,
}

#[wasm_bindgen]
impl TrackerUpdate {
    /// Raw per-window detection - drive a live display (tuner) with this.
    #[wasm_bindgen(getter)]
    pub fn live(&self) -> Option<NoteEvent> {
        self.inner.live.clone().map(|inner| NoteEvent { inner })
    }

    /// Present at most once per held note - append to a note history.
    #[wasm_bindgen(getter, js_name = noteStarted)]
    pub fn note_started(&self) -> Option<NoteEvent> {
        self.inner
            .note_started
            .clone()
            .map(|inner| NoteEvent { inner })
    }
}

/// Streaming note tracker: pitch detection plus debouncing that turns noisy
/// per-window detections into a stream of confirmed instrument notes.
#[wasm_bindgen]
pub struct NoteTracker {
    inner: sconote_core::NoteTracker,
}

#[wasm_bindgen]
impl NoteTracker {
    #[wasm_bindgen(constructor)]
    pub fn new(sample_rate: u32, window_size: usize) -> NoteTracker {
        NoteTracker {
            inner: sconote_core::NoteTracker::new(sample_rate, window_size),
        }
    }

    pub fn process(&mut self, samples: &[f32]) -> TrackerUpdate {
        TrackerUpdate {
            inner: self.inner.process(samples),
        }
    }
}

/// An uploaded file decoded to mono samples by the Rust decoders, so a
/// given file produces identical samples in every browser (the platform
/// `decodeAudioData` resamples to the device rate and differs per browser).
#[wasm_bindgen]
pub struct DecodedAudio {
    samples: Vec<f32>,
    sample_rate: u32,
}

#[wasm_bindgen]
impl DecodedAudio {
    #[wasm_bindgen(getter, js_name = sampleRate)]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Mono samples in [-1, 1].
    pub fn samples(&self) -> Vec<f32> {
        self.samples.clone()
    }
}

/// Decode a WAV or MP3 file's bytes. Errors on any other format - fall
/// back to the browser's decoder for those.
#[wasm_bindgen(js_name = decodeAudio)]
pub fn decode_audio(bytes: &[u8]) -> Result<DecodedAudio, JsError> {
    let audio =
        sconote_poly::read_audio_mono(bytes).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(DecodedAudio {
        samples: audio.samples,
        sample_rate: audio.sample_rate,
    })
}

/// Offline polyphonic transcriber (the Basic Pitch CNN, embedded). Create
/// once, reuse across recordings.
#[wasm_bindgen]
pub struct Transcriber {
    model: sconote_poly::BasicPitch,
}

#[wasm_bindgen]
impl Transcriber {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<Transcriber, JsError> {
        let model = sconote_poly::BasicPitch::new().map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Transcriber { model })
    }

    /// Start a transcription job over a whole recording (any sample rate).
    pub fn begin(&self, samples: &[f32], sample_rate: u32) -> TranscriptionJob {
        let audio = MonoAudio {
            samples: samples.to_vec(),
            sample_rate,
        };
        TranscriptionJob {
            inner: Some(WindowedTranscription::new(&audio)),
        }
    }
}

/// One in-flight transcription. Call `processNextWindow` until it returns
/// false (yielding to the event loop in between keeps the page responsive),
/// then `finish` to get the notes.
#[wasm_bindgen]
pub struct TranscriptionJob {
    /// Taken by `finish`; `None` afterwards.
    inner: Option<WindowedTranscription>,
}

#[wasm_bindgen]
impl TranscriptionJob {
    #[wasm_bindgen(getter, js_name = totalWindows)]
    pub fn total_windows(&self) -> usize {
        self.inner
            .as_ref()
            .map_or(0, WindowedTranscription::total_windows)
    }

    #[wasm_bindgen(getter, js_name = windowsDone)]
    pub fn windows_done(&self) -> usize {
        self.inner
            .as_ref()
            .map_or(0, WindowedTranscription::windows_done)
    }

    /// Run the network over the next window; false once all are done.
    #[wasm_bindgen(js_name = processNextWindow)]
    pub fn process_next_window(&mut self, transcriber: &Transcriber) -> Result<bool, JsError> {
        let Some(job) = self.inner.as_mut() else {
            return Ok(false);
        };
        job.process_next_window(&transcriber.model)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Extract the notes under the given thresholds (0.5 / 0.3 / 0.7 / 0.8
    /// / 1.0 / 0.6 are the defaults). The third is the bar an onset must
    /// clear to re-articulate a pitch that is already sounding - lower it
    /// for material with fast repeated notes. The fourth drops notes that
    /// are the subharmonic shadow of a louder note an octave or twelfth
    /// above (a note this much quieter, or less, is a ghost) - 0 disables
    /// it, raise it toward 1 for a stricter cleanup. The fifth vetoes a
    /// re-articulation whose onset is explained by a simultaneous strike an
    /// octave or twelfth above at least this factor as strong - 0 disables
    /// it, raise it above 1 to keep more repeated notes. The sixth drops
    /// notes that are the weak 2nd/3rd harmonic of a note an octave or
    /// twelfth below - the dominant spurious-note source on real
    /// recordings; 0 disables it. Consumes the job.
    pub fn finish(
        &mut self,
        onset_threshold: f32,
        frame_threshold: f32,
        retrigger_onset_threshold: f32,
        onset_ghost_energy_ratio: f32,
        retrigger_octave_veto: f32,
        overtone_ghost_energy_ratio: f32,
    ) -> Result<TranscribedNotes, JsError> {
        let job = self
            .inner
            .take()
            .ok_or_else(|| JsError::new("transcription already finished"))?;
        let options = NoteCreationOptions {
            onset_threshold,
            frame_threshold,
            retrigger_onset_threshold,
            onset_ghost_energy_ratio,
            retrigger_octave_veto,
            overtone_ghost_energy_ratio,
            ..NoteCreationOptions::default()
        };
        let notes = job.finish().to_notes(&options);
        Ok(TranscribedNotes {
            midis: notes.iter().map(|note| note.midi).collect(),
            onsets: notes.iter().map(|note| note.onset_s).collect(),
            offsets: notes.iter().map(|note| note.offset_s).collect(),
        })
    }
}

/// Transcribed notes as parallel arrays (same index = same note) - cheap to
/// move across the JS boundary.
#[wasm_bindgen]
pub struct TranscribedNotes {
    midis: Vec<u8>,
    onsets: Vec<f64>,
    offsets: Vec<f64>,
}

#[wasm_bindgen]
impl TranscribedNotes {
    #[wasm_bindgen(getter)]
    pub fn count(&self) -> usize {
        self.midis.len()
    }

    /// MIDI note numbers (A4 = 69).
    pub fn midis(&self) -> Vec<u8> {
        self.midis.clone()
    }

    /// Note starts, seconds from the beginning of the recording.
    pub fn onsets(&self) -> Vec<f64> {
        self.onsets.clone()
    }

    /// Note ends, seconds from the beginning of the recording.
    pub fn offsets(&self) -> Vec<f64> {
        self.offsets.clone()
    }

    /// Encode as a Standard MIDI File (120 BPM grid) - save as `.mid`.
    #[wasm_bindgen(js_name = toMidi)]
    pub fn to_midi(&self) -> Vec<u8> {
        sconote_poly::notes_to_midi_bytes(&self.notes())
    }

    /// Tempo estimated from the note onsets (BPM).
    #[wasm_bindgen(js_name = estimatedBpm)]
    pub fn estimated_bpm(&self) -> f64 {
        sconote_poly::estimate_bpm(&self.notes())
    }

    /// Render as a MusicXML score (single grand-staff part) for a sheet
    /// music renderer. `bpm` omitted → the beat is tracked through the
    /// performance, so rubato and ritardandi notate on the right beats;
    /// given → a fixed grid at that tempo.
    #[wasm_bindgen(js_name = toMusicXml)]
    pub fn to_music_xml(&self, bpm: Option<f64>) -> String {
        sconote_poly::parts_to_musicxml(
            &[sconote_poly::ScorePart {
                name: "Piano".to_string(),
                notes: self.notes(),
            }],
            bpm,
        )
    }

    fn notes(&self) -> Vec<sconote_poly::TranscribedNote> {
        (0..self.midis.len())
            .map(|i| sconote_poly::TranscribedNote {
                midi: self.midis[i],
                onset_s: self.onsets[i],
                offset_s: self.offsets[i],
            })
            .collect()
    }
}
