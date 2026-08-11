//! A phrase played with pauses, fed in Web Audio-sized chunks, yields a
//! clean note history through the public API.

use crate::NoteTracker;
use crate::test_signals::{SAMPLE_RATE, WINDOW, sine};

#[test]
fn a_played_phrase_produces_its_note_history() {
    let rest = vec![0.0; WINDOW * 4];
    let mut phrase = sine(440.0, WINDOW * 5); // A4
    phrase.extend_from_slice(&rest);
    phrase.extend(sine(329.628, WINDOW * 5)); // E4
    phrase.extend_from_slice(&rest);
    phrase.extend(sine(523.251, WINDOW * 5)); // C5

    let mut tracker = NoteTracker::new(SAMPLE_RATE, WINDOW);
    let mut history = Vec::new();
    // 128 samples per call, as an AudioWorklet would deliver them.
    for chunk in phrase.chunks(128) {
        if let Some(event) = tracker.process(chunk).note_started {
            history.push(event.note_name);
        }
    }
    assert_eq!(history, ["A4", "E4", "C5"]);
}
