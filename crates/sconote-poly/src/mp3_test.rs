use super::*;

/// 0.4 s of a 440 Hz sine at 44.1 kHz, encoded mono at 128 kbps.
const SINE_440: &[u8] = include_bytes!("../testdata/sine_440.mp3");

#[test]
fn decodes_a_sine_mp3_with_rate_and_pitch_intact() {
    let audio = read_mp3_mono(SINE_440).unwrap();
    assert_eq!(audio.sample_rate, 44100);
    // 0.4 s of signal (17 640 samples) plus bounded encoder/decoder padding.
    assert!(
        (17_000..24_000).contains(&audio.samples.len()),
        "unexpected length {}",
        audio.samples.len()
    );
    // Dominant frequency via zero crossings over the steady middle stretch.
    let middle = &audio.samples[4410..13230];
    let crossings = middle
        .windows(2)
        .filter(|pair| (pair[0] >= 0.0) != (pair[1] >= 0.0))
        .count();
    let frequency = crossings as f64 / 2.0 / (middle.len() as f64 / 44100.0);
    assert!(
        (frequency - 440.0).abs() < 5.0,
        "expected ~440 Hz, got {frequency:.1}"
    );
}

#[test]
fn garbage_bytes_are_a_decode_error() {
    assert!(read_mp3_mono(&[0x12; 64]).is_err());
    assert!(read_mp3_mono(&[]).is_err());
}
