//! MP3 → mono f32 samples, decoded in Rust so a given file transcribes
//! identically on every platform and browser.

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::wav::MonoAudio;

#[derive(Debug, thiserror::Error)]
pub enum Mp3Error {
    #[error("invalid MP3: {0}")]
    Decode(#[from] SymphoniaError),
    #[error("MP3 contains no audio")]
    NoAudio,
}

/// Decode an MP3 file's bytes (any channel count) into mono f32.
pub fn read_mp3_mono(bytes: &[u8]) -> Result<MonoAudio, Mp3Error> {
    let stream = MediaSourceStream::new(
        Box::new(std::io::Cursor::new(bytes.to_vec())),
        MediaSourceStreamOptions::default(),
    );
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let mut format = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?
        .format;
    let track = format.default_track().ok_or(Mp3Error::NoAudio)?;
    let track_id = track.id;
    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;

    let mut samples = Vec::new();
    let mut sample_rate = 0;
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            // Normal end of stream.
            Err(SymphoniaError::IoError(io))
                if io.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(error) => return Err(error.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            // A corrupt frame is skippable; anything else is fatal.
            Err(SymphoniaError::DecodeError(..)) => continue,
            Err(error) => return Err(error.into()),
        };
        let spec = *decoded.spec();
        sample_rate = spec.rate;
        let channels = spec.channels.count();
        let mut interleaved = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        interleaved.copy_interleaved_ref(decoded);
        samples.extend(
            interleaved
                .samples()
                .chunks(channels)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32),
        );
    }
    if samples.is_empty() {
        return Err(Mp3Error::NoAudio);
    }
    Ok(MonoAudio {
        samples,
        sample_rate,
    })
}

#[cfg(test)]
#[path = "mp3_test.rs"]
mod mp3_test;
