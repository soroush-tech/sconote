//! Full-recording transcription: window the audio, run the network over
//! each window, stitch the overlapping outputs, and extract notes.
//!
//! Windowing and stitching mirror Basic Pitch's reference inference:
//! windows overlap by 30 frames, the audio is front-padded by half an
//! overlap, and each window's first/last 15 output frames are discarded.

use crate::model::{
    BasicPitch, FFT_HOP, LOWEST_MIDI, MODEL_SAMPLE_RATE, ModelError, PITCH_BINS, WINDOW_FRAMES,
    WINDOW_SAMPLES,
};
use crate::note_creation::{NoteCreationOptions, notes_from_activations};
use crate::resample::resample;
use crate::score::TranscribedNote;
use crate::wav::MonoAudio;

const OVERLAP_FRAMES: usize = 30;
const OVERLAP_SAMPLES: usize = OVERLAP_FRAMES * FFT_HOP;
const HOP_SAMPLES: usize = WINDOW_SAMPLES - OVERLAP_SAMPLES;
const TRIM_FRAMES: usize = OVERLAP_FRAMES / 2;
/// Output frames per second of audio - integer, as in the reference
/// (22050 // 256), used only for final output-length trimming.
const ANNOTATIONS_FPS: usize = MODEL_SAMPLE_RATE as usize / FFT_HOP;

/// Stitched network output for a whole recording - compute once, then
/// extract notes under different thresholds cheaply via [`Activations::to_notes`].
pub struct Activations {
    /// Row-major `[frame][pitch]`, [`PITCH_BINS`] columns.
    pub onsets: Vec<f32>,
    pub frames: Vec<f32>,
    pub n_frames: usize,
}

/// Incremental transcription: the same computation as
/// [`compute_activations`], one window per call, so an interactive caller
/// (e.g. a browser main thread) can update a progress display in between.
pub struct WindowedTranscription {
    padded: Vec<f32>,
    /// Length of the resampled audio, without the front padding.
    audio_len: usize,
    onsets: Vec<f32>,
    frames: Vec<f32>,
    next_start: usize,
}

impl WindowedTranscription {
    pub fn new(audio: &MonoAudio) -> WindowedTranscription {
        let audio = resample(audio, MODEL_SAMPLE_RATE);
        let mut padded = vec![0.0_f32; OVERLAP_SAMPLES / 2];
        padded.extend_from_slice(&audio.samples);
        WindowedTranscription {
            audio_len: audio.samples.len(),
            padded,
            onsets: Vec::new(),
            frames: Vec::new(),
            next_start: 0,
        }
    }

    pub fn total_windows(&self) -> usize {
        self.padded.len().div_ceil(HOP_SAMPLES)
    }

    pub fn windows_done(&self) -> usize {
        (self.next_start / HOP_SAMPLES).min(self.total_windows())
    }

    /// Run the network over the next window. Returns whether more windows
    /// remain, so `while job.process_next_window(&model)? {}` runs them all.
    pub fn process_next_window(&mut self, model: &BasicPitch) -> Result<bool, ModelError> {
        if self.next_start >= self.padded.len() {
            return Ok(false);
        }
        let start = self.next_start;
        let mut window = vec![0.0_f32; WINDOW_SAMPLES];
        let available = (self.padded.len() - start).min(WINDOW_SAMPLES);
        window[..available].copy_from_slice(&self.padded[start..start + available]);
        let prediction = model.predict(&window)?;
        let kept = TRIM_FRAMES * PITCH_BINS..(WINDOW_FRAMES - TRIM_FRAMES) * PITCH_BINS;
        self.onsets
            .extend_from_slice(&prediction.onsets[kept.clone()]);
        self.frames.extend_from_slice(&prediction.notes[kept]);
        self.next_start += HOP_SAMPLES;
        Ok(self.next_start < self.padded.len())
    }

    /// Stitched activations; call once every window has been processed.
    pub fn finish(self) -> Activations {
        let expected_frames = self.audio_len * ANNOTATIONS_FPS / MODEL_SAMPLE_RATE as usize;
        let mut onsets = self.onsets;
        let mut frames = self.frames;
        let n_frames = (onsets.len() / PITCH_BINS).min(expected_frames);
        onsets.truncate(n_frames * PITCH_BINS);
        frames.truncate(n_frames * PITCH_BINS);
        Activations {
            onsets,
            frames,
            n_frames,
        }
    }
}

/// Run the network over a whole recording (any sample rate - it is
/// resampled to [`MODEL_SAMPLE_RATE`] first).
pub fn compute_activations(
    audio: &MonoAudio,
    model: &BasicPitch,
) -> Result<Activations, ModelError> {
    let mut job = WindowedTranscription::new(audio);
    while job.process_next_window(model)? {}
    Ok(job.finish())
}

impl Activations {
    /// Extract notes under the given thresholds, sorted by onset.
    pub fn to_notes(&self, options: &NoteCreationOptions) -> Vec<TranscribedNote> {
        let mut notes: Vec<TranscribedNote> =
            notes_from_activations(&self.onsets, &self.frames, self.n_frames, options)
                .into_iter()
                .map(|raw| TranscribedNote {
                    midi: raw.pitch_bin as u8 + LOWEST_MIDI,
                    onset_s: frame_to_seconds(raw.start_frame),
                    offset_s: frame_to_seconds(raw.end_frame),
                })
                .collect();
        notes.sort_by(|a, b| a.onset_s.total_cmp(&b.onset_s).then(a.midi.cmp(&b.midi)));
        notes
    }
}

/// Transcribe a whole recording into notes, sorted by onset.
pub fn transcribe(
    audio: &MonoAudio,
    model: &BasicPitch,
    options: &NoteCreationOptions,
) -> Result<Vec<TranscribedNote>, ModelError> {
    Ok(compute_activations(audio, model)?.to_notes(options))
}

/// Basic Pitch's frame→time mapping, including its empirical per-window
/// drift correction (constants verbatim from `model_frames_to_time`).
fn frame_to_seconds(frame: usize) -> f64 {
    let hop_s = FFT_HOP as f64 / f64::from(MODEL_SAMPLE_RATE);
    let window_number = (frame / WINDOW_FRAMES) as f64;
    let window_offset =
        hop_s * (WINDOW_FRAMES as f64 - WINDOW_SAMPLES as f64 / FFT_HOP as f64) + 0.0018;
    frame as f64 * hop_s - window_offset * window_number
}

#[cfg(test)]
#[path = "transcribe_test.rs"]
mod transcribe_test;

#[cfg(test)]
#[path = "transcribe_spec.rs"]
mod transcribe_spec;
