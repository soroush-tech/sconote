//! WAV → mono f32 samples in [-1, 1], the format the engines consume.

use std::io::Read;

/// Decoded audio: interleaving resolved to mono by averaging channels.
#[derive(Debug, Clone, PartialEq)]
pub struct MonoAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum WavError {
    #[error("invalid WAV: {0}")]
    Decode(#[from] hound::Error),
}

/// Read a WAV stream (any channel count; int or float samples) into mono f32.
pub fn read_wav_mono<R: Read>(reader: R) -> Result<MonoAudio, WavError> {
    let mut wav = hound::WavReader::new(reader)?;
    let spec = wav.spec();
    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => wav.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = (1_i64 << (spec.bits_per_sample - 1)) as f32;
            wav.samples::<i32>()
                .map(|sample| sample.map(|s| s as f32 / scale))
                .collect::<Result<_, _>>()?
        }
    };
    let channels = usize::from(spec.channels);
    let samples = interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect();
    Ok(MonoAudio {
        samples,
        sample_rate: spec.sample_rate,
    })
}

#[cfg(test)]
#[path = "wav_test.rs"]
mod wav_test;
