//! Basic Pitch CNN inference via tract (pure Rust — WASM/mobile friendly).
//!
//! The vendored model (`models/nmp.onnx`, 230 KB) is Spotify's Basic Pitch
//! "nmp" network, Apache-2.0, from
//! <https://github.com/spotify/basic-pitch/blob/main/basic_pitch/saved_models/icassp_2022/nmp.onnx>.
//! Its graph *includes* the CQT + harmonic-stacking frontend as Conv ops, so
//! the input is raw audio: one window of [`WINDOW_SAMPLES`] mono f32 samples
//! at [`MODEL_SAMPLE_RATE`]. Outputs are per-frame probabilities over the 88
//! piano keys.

use std::io::Cursor;

use tract_onnx::prelude::*;

/// The model hears 22.05 kHz audio; callers must resample to this.
pub const MODEL_SAMPLE_RATE: u32 = 22_050;
/// Samples per analysis frame (the model's internal hop).
pub const FFT_HOP: usize = 256;
/// One input window: 2 s minus one hop.
pub const WINDOW_SAMPLES: usize = 2 * MODEL_SAMPLE_RATE as usize - FFT_HOP;
/// Output frames per window.
pub const WINDOW_FRAMES: usize = 172;
/// The 88 piano keys, A0..C8.
pub const PITCH_BINS: usize = 88;
/// MIDI note of pitch bin 0 (A0).
pub const LOWEST_MIDI: u8 = 21;

const MODEL_BYTES: &[u8] = include_bytes!("../models/nmp.onnx");

/// Per-frame pitch probabilities for one input window, both matrices
/// row-major `[frame][pitch]` with [`WINDOW_FRAMES`] × [`PITCH_BINS`]
/// entries. (The model's third output, pitch-bend contours, is unused for
/// note extraction and not exposed.)
#[derive(Debug, Clone, PartialEq)]
pub struct WindowPrediction {
    /// Probability that a note *starts* at this frame/pitch.
    pub onsets: Vec<f32>,
    /// Probability that this pitch is *sounding* at this frame.
    pub notes: Vec<f32>,
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("Basic Pitch inference failed: {0}")]
    Tract(#[from] TractError),
    #[error("audio window must be exactly {WINDOW_SAMPLES} samples, got {0}")]
    BadWindowLength(usize),
}

type Plan = TypedRunnableModel<TypedModel>;

/// The Basic Pitch network, loaded once and reusable across windows.
pub struct BasicPitch {
    plan: Plan,
}

impl BasicPitch {
    pub fn new() -> Result<BasicPitch, ModelError> {
        let plan = tract_onnx::onnx()
            .model_for_read(&mut Cursor::new(MODEL_BYTES))?
            .with_input_fact(
                0,
                InferenceFact::dt_shape(f32::datum_type(), [1, WINDOW_SAMPLES, 1]),
            )?
            .into_typed()?
            .into_decluttered()?
            // Not `into_optimized()`: tract 0.21's kernel-packing pass panics
            // on this tf2onnx graph (opaque-fact assertion in fact.rs). The
            // decluttered plan runs a 2 s window in ~240 ms — ample.
            .into_runnable()?;
        Ok(BasicPitch { plan })
    }

    /// Run one window of [`WINDOW_SAMPLES`] mono samples at
    /// [`MODEL_SAMPLE_RATE`] through the network.
    pub fn predict(&self, window: &[f32]) -> Result<WindowPrediction, ModelError> {
        if window.len() != WINDOW_SAMPLES {
            return Err(ModelError::BadWindowLength(window.len()));
        }
        let input = Tensor::from_shape(&[1, WINDOW_SAMPLES, 1], window)?;
        // Graph output order is [:2, :1, :0] = [onset, note, contour]
        // (mapping per basic_pitch/inference.py).
        let outputs = self.plan.run(tvec!(input.into()))?;
        Ok(WindowPrediction {
            onsets: outputs[0].as_slice::<f32>()?.to_vec(),
            notes: outputs[1].as_slice::<f32>()?.to_vec(),
        })
    }
}

#[cfg(test)]
#[path = "model_test.rs"]
mod model_test;
