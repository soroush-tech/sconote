use super::*;
use crate::ground_truth::GroundTruthNote;
use crate::synth::render_notes;

#[test]
fn silence_yields_probability_matrices_of_expected_shape() {
    let model = BasicPitch::new().expect("model loads");
    let prediction = model
        .predict(&vec![0.0; WINDOW_SAMPLES])
        .expect("inference");
    assert_eq!(prediction.onsets.len(), WINDOW_FRAMES * PITCH_BINS);
    assert_eq!(prediction.notes.len(), WINDOW_FRAMES * PITCH_BINS);
    let all = prediction.onsets.iter().chain(&prediction.notes);
    assert!(all.clone().all(|p| (0.0..=1.0).contains(p)));
}

#[test]
fn wrong_window_length_is_rejected() {
    let model = BasicPitch::new().expect("model loads");
    let error = model.predict(&[0.0; 100]).expect_err("must reject");
    assert!(matches!(error, ModelError::BadWindowLength(100)));
}

#[test]
fn rendered_a4_lights_up_the_a4_note_bin() {
    let note = GroundTruthNote {
        midi: 69,
        onset_s: 0.2,
        offset_s: 1.5,
    };
    let mut window = render_notes(&[note], MODEL_SAMPLE_RATE).samples;
    window.resize(WINDOW_SAMPLES, 0.0);

    let model = BasicPitch::new().expect("model loads");
    let prediction = model.predict(&window).expect("inference");

    // Mean note probability over frames inside the note body (0.35 s–1.16 s
    // ≈ frames 30..100 at 86 fps).
    let bin_mean = |pitch_bin: usize| {
        (30..100)
            .map(|frame| prediction.notes[frame * PITCH_BINS + pitch_bin])
            .sum::<f32>()
            / 70.0
    };
    let a4 = bin_mean(usize::from(69 - LOWEST_MIDI));
    let unrelated_low_d = bin_mean(usize::from(38 - LOWEST_MIDI));
    assert!(a4 > 0.5, "A4 bin should be active, got {a4}");
    assert!(
        a4 > 5.0 * unrelated_low_d,
        "A4 {a4} vs D2 {unrelated_low_d}"
    );
}
