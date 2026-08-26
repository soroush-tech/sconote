use super::*;
use crate::ground_truth::GroundTruthNote;
use crate::synth::render_notes;

#[test]
fn single_rendered_note_transcribes_to_itself() {
    let note = GroundTruthNote {
        midi: 69,
        onset_s: 0.5,
        offset_s: 1.5,
    };
    let audio = render_notes(&[note], MODEL_SAMPLE_RATE);
    let model = BasicPitch::new().expect("model loads");
    let notes = transcribe(&audio, &model, &NoteCreationOptions::default()).expect("transcribes");
    assert_eq!(notes.len(), 1, "got {notes:?}");
    assert_eq!(notes[0].midi, 69);
    assert!(
        (notes[0].onset_s - 0.5).abs() < 0.1,
        "onset {}",
        notes[0].onset_s
    );
    assert!(notes[0].offset_s > notes[0].onset_s);
}

#[test]
fn windowed_transcription_reports_progress_and_terminates() {
    let note = GroundTruthNote {
        midi: 60,
        onset_s: 0.5,
        offset_s: 3.0,
    };
    let audio = render_notes(&[note], MODEL_SAMPLE_RATE);
    let model = BasicPitch::new().expect("model loads");
    let mut job = WindowedTranscription::new(&audio);
    let total = job.total_windows();
    assert!(total >= 2, "3 s should span multiple windows, got {total}");
    assert_eq!(job.windows_done(), 0);
    let mut steps = 0;
    while job.process_next_window(&model).expect("inference") {
        steps += 1;
        assert_eq!(job.windows_done(), steps);
    }
    assert_eq!(job.windows_done(), total);
    let notes = job.finish().to_notes(&NoteCreationOptions::default());
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].midi, 60);
}

#[test]
fn device_rate_audio_is_resampled_and_transcribed() {
    // 48 kHz in, as a browser recording would be.
    let note = GroundTruthNote {
        midi: 64,
        onset_s: 0.5,
        offset_s: 1.5,
    };
    let audio = render_notes(&[note], 48_000);
    let model = BasicPitch::new().expect("model loads");
    let notes = transcribe(&audio, &model, &NoteCreationOptions::default()).expect("transcribes");
    assert_eq!(notes.len(), 1, "got {notes:?}");
    assert_eq!(notes[0].midi, 64);
    assert!(
        (notes[0].onset_s - 0.5).abs() < 0.1,
        "onset {}",
        notes[0].onset_s
    );
}

#[test]
fn the_last_window_is_zero_padded() {
    let note = GroundTruthNote {
        midi: 60,
        onset_s: 0.5,
        offset_s: 3.0,
    };
    let audio = render_notes(&[note], MODEL_SAMPLE_RATE);
    let job = WindowedTranscription::new(&audio);
    let last = job.window_samples(job.total_windows() - 1);
    assert_eq!(last.len(), WINDOW_SAMPLES);
    assert_eq!(
        last[WINDOW_SAMPLES - 1],
        0.0,
        "tail beyond the audio is silence"
    );
    assert!(
        last.iter().any(|&s| s != 0.0),
        "the window still holds audio"
    );
}

#[test]
fn windows_computed_out_of_order_stitch_to_the_sequential_result() {
    let note = GroundTruthNote {
        midi: 60,
        onset_s: 0.5,
        offset_s: 3.0,
    };
    let audio = render_notes(&[note], MODEL_SAMPLE_RATE);
    let model = BasicPitch::new().expect("model loads");
    let sequential = compute_activations(&audio, &model).expect("inference");

    let mut job = WindowedTranscription::new(&audio);
    for index in (0..job.total_windows()).rev() {
        let output = WindowedTranscription::predict_window(&model, &job.window_samples(index))
            .expect("inference");
        job.set_window(index, output);
    }
    assert_eq!(job.windows_done(), job.total_windows());
    let stitched = job.finish();
    assert_eq!(stitched.n_frames, sequential.n_frames);
    assert_eq!(stitched.onsets, sequential.onsets);
    assert_eq!(stitched.frames, sequential.frames);
}

#[cfg(feature = "parallel")]
#[test]
fn parallel_activations_match_the_sequential_loop() {
    let note = GroundTruthNote {
        midi: 60,
        onset_s: 0.5,
        offset_s: 3.0,
    };
    let audio = render_notes(&[note], MODEL_SAMPLE_RATE);
    let model = BasicPitch::new().expect("model loads");
    let parallel = compute_activations(&audio, &model).expect("inference");
    let mut job = WindowedTranscription::new(&audio);
    while job.process_next_window(&model).expect("inference") {}
    let sequential = job.finish();
    assert_eq!(parallel.onsets, sequential.onsets);
    assert_eq!(parallel.frames, sequential.frames);
}
