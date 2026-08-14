use super::*;

fn wav_bytes(sample_rate: u32) -> Vec<u8> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = std::io::Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut cursor, spec).unwrap();
    for i in 0..1000_i16 {
        writer.write_sample(i.wrapping_mul(300)).unwrap();
    }
    writer.finalize().unwrap();
    cursor.into_inner()
}

#[test]
fn riff_bytes_take_the_wav_path() {
    let audio = read_audio_mono(&wav_bytes(48_000)).unwrap();
    assert_eq!(audio.sample_rate, 48_000);
    assert_eq!(audio.samples.len(), 1000);
}

#[test]
fn mp3_bytes_take_the_mp3_path() {
    let audio = read_audio_mono(include_bytes!("../testdata/sine_440.mp3")).unwrap();
    assert_eq!(audio.sample_rate, 44_100);
}

#[test]
fn unrecognized_bytes_are_an_error() {
    assert!(read_audio_mono(b"OggS not actually supported").is_err());
}
