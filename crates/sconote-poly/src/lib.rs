//! Sconote poly: polyphonic note transcription (audio → note events).
//!
//! Detection runs the Basic Pitch CNN ([`BasicPitch`]) over windows of raw
//! audio. Around it sits the evaluation harness the engine is tuned with:
//! MIDI ground-truth extraction ([`notes_from_midi`]), WAV loading
//! ([`read_wav_mono`]), synthetic rendering ([`render_notes`]), and
//! note-level scoring against a reference ([`score_notes`]).

mod ground_truth;
mod midi_export;
mod model;
mod musicxml;
mod note_creation;
mod resample;
mod score;
mod synth;
mod tempo;
mod transcribe;
mod wav;

pub use ground_truth::{GroundTruthError, GroundTruthNote, notes_from_midi};
pub use midi_export::notes_to_midi_bytes;
pub use model::{
    BasicPitch, FFT_HOP, LOWEST_MIDI, MODEL_SAMPLE_RATE, ModelError, PITCH_BINS, WINDOW_FRAMES,
    WINDOW_SAMPLES, WindowPrediction,
};
pub use musicxml::{ScorePart, parts_to_musicxml};
pub use note_creation::{NoteCreationOptions, RawNote, notes_from_activations};
pub use resample::resample;
pub use score::{ScoreReport, TranscribedNote, score_notes};
pub use synth::render_notes;
pub use tempo::{DEFAULT_BPM, estimate_bpm};
pub use transcribe::{Activations, WindowedTranscription, compute_activations, transcribe};
pub use wav::{MonoAudio, WavError, read_wav_mono};
