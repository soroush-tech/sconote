//! One entry point for decoding an uploaded audio file's bytes: sniff the
//! container and hand off to the matching decoder. Formats beyond WAV and
//! MP3 are the platform's job (e.g. the browser's `decodeAudioData`).

use crate::mp3::{Mp3Error, read_mp3_mono};
use crate::wav::{MonoAudio, WavError, read_wav_mono};

#[derive(Debug, thiserror::Error)]
pub enum AudioDecodeError {
    #[error(transparent)]
    Wav(#[from] WavError),
    #[error(transparent)]
    Mp3(#[from] Mp3Error),
}

/// Decode a WAV or MP3 file's bytes into mono f32 samples.
pub fn read_audio_mono(bytes: &[u8]) -> Result<MonoAudio, AudioDecodeError> {
    if bytes.starts_with(b"RIFF") {
        Ok(read_wav_mono(std::io::Cursor::new(bytes))?)
    } else {
        Ok(read_mp3_mono(bytes)?)
    }
}

#[cfg(test)]
#[path = "decode_test.rs"]
mod decode_test;
