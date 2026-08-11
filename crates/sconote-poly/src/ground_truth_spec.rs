//! Integration: extract ground truth from the real full-band MIDI in
//! `examples/midi/`. The expected figures were computed by an independent
//! Python implementation of the SMF spec (FIFO pairing, tempo map, channel
//! 10 excluded).

use std::fs;
use std::path::Path;

use crate::notes_from_midi;

#[test]
fn extracts_ground_truth_from_real_full_band_midi() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/midi/Queen - Bohemian Rhapsody.mid");
    let bytes = fs::read(path).expect("fixture MIDI present");
    let notes = notes_from_midi(&bytes).expect("valid MIDI");

    assert_eq!(notes.len(), 4811);

    let first = notes[0];
    assert_eq!(first.midi, 65);
    assert!((first.onset_s - 2.692305).abs() < 1e-4);
    assert!((first.offset_s - 3.012818).abs() < 1e-4);

    let last = notes[notes.len() - 1];
    assert_eq!(last.midi, 65);
    assert!((last.onset_s - 322.876800).abs() < 1e-4);
    assert!((last.offset_s - 327.876795).abs() < 1e-4);

    assert!(
        notes
            .windows(2)
            .all(|pair| pair[0].onset_s <= pair[1].onset_s)
    );
    assert!(notes.iter().all(|note| note.offset_s > note.onset_s));
}
