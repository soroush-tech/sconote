use std::io::Cursor;

use hound::{SampleFormat, WavSpec, WavWriter};

use super::*;

fn wav_bytes(
    spec: WavSpec,
    write_samples: impl FnOnce(&mut WavWriter<&mut Cursor<Vec<u8>>>),
) -> Cursor<Vec<u8>> {
    let mut cursor = Cursor::new(Vec::new());
    let mut writer = WavWriter::new(&mut cursor, spec).expect("in-memory writer");
    write_samples(&mut writer);
    writer.finalize().expect("finalize");
    cursor.set_position(0);
    cursor
}

#[test]
fn float_stereo_averages_to_mono() {
    let spec = WavSpec {
        channels: 2,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };
    let cursor = wav_bytes(spec, |writer| {
        for sample in [0.5_f32, -0.5, 1.0, 0.0] {
            writer.write_sample(sample).expect("write");
        }
    });
    let audio = read_wav_mono(cursor).expect("decode");
    assert_eq!(audio.sample_rate, 48_000);
    assert_eq!(audio.samples, vec![0.0, 0.5]);
}

#[test]
fn int16_mono_normalizes_to_unit_range() {
    let spec = WavSpec {
        channels: 1,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let cursor = wav_bytes(spec, |writer| {
        for sample in [i16::MIN, 0, 16_384] {
            writer.write_sample(sample).expect("write");
        }
    });
    let audio = read_wav_mono(cursor).expect("decode");
    assert_eq!(audio.samples, vec![-1.0, 0.0, 0.5]);
}

#[test]
fn garbage_bytes_are_a_decode_error() {
    let error = read_wav_mono(Cursor::new(vec![0_u8; 8])).expect_err("garbage must fail");
    assert!(matches!(error, WavError::Decode(_)));
    assert!(error.to_string().starts_with("invalid WAV"));
}
