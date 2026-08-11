use super::*;
use crate::ground_truth::notes_from_midi;

fn note(midi: u8, onset_s: f64, offset_s: f64) -> TranscribedNote {
    TranscribedNote {
        midi,
        onset_s,
        offset_s,
    }
}

#[test]
fn notes_round_trip_through_the_ground_truth_parser() {
    let notes = [note(60, 0.5, 1.0), note(64, 0.5, 1.25), note(60, 1.5, 2.0)];
    let bytes = notes_to_midi_bytes(&notes);
    let parsed = notes_from_midi(&bytes).expect("valid MIDI");
    assert_eq!(parsed.len(), notes.len());
    for (original, parsed) in notes.iter().zip(&parsed) {
        assert_eq!(parsed.midi, original.midi);
        // 960 ticks per second → about 1 ms of quantization.
        assert!((parsed.onset_s - original.onset_s).abs() < 0.002);
        assert!((parsed.offset_s - original.offset_s).abs() < 0.002);
    }
}

#[test]
fn zero_length_note_still_produces_a_pair() {
    let bytes = notes_to_midi_bytes(&[note(60, 1.0, 1.0)]);
    let parsed = notes_from_midi(&bytes).expect("valid MIDI");
    assert_eq!(parsed.len(), 1);
    assert!(parsed[0].offset_s > parsed[0].onset_s);
}

#[test]
fn empty_note_list_is_a_valid_empty_midi_file() {
    let bytes = notes_to_midi_bytes(&[]);
    assert_eq!(notes_from_midi(&bytes).expect("valid MIDI"), Vec::new());
}

#[test]
fn out_of_range_pitch_is_clamped_not_panicking() {
    let bytes = notes_to_midi_bytes(&[note(200, 0.0, 1.0)]);
    let parsed = notes_from_midi(&bytes).expect("valid MIDI");
    assert_eq!(parsed[0].midi, 127);
}
