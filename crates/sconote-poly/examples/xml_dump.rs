//! Render a transcription MIDI as MusicXML — the same rendering the apps
//! ship — so score output can be inspected and diffed against an engraved
//! reference without a browser.
//!
//! Usage: cargo run --release -p sconote-poly --example xml_dump -- <notes.mid> <out.musicxml>

use sconote_poly::{ScorePart, TranscribedNote, notes_from_midi, parts_to_musicxml};

fn main() {
    let mut args = std::env::args().skip(1);
    let usage = "usage: xml_dump <notes.mid> <out.musicxml>";
    let midi_path = args.next().expect(usage);
    let out_path = args.next().expect(usage);
    let notes: Vec<TranscribedNote> =
        notes_from_midi(&std::fs::read(&midi_path).expect("read midi"))
            .expect("parse midi")
            .iter()
            .map(|n| TranscribedNote {
                midi: n.midi,
                onset_s: n.onset_s,
                offset_s: n.offset_s,
            })
            .collect();
    let xml = parts_to_musicxml(
        &[ScorePart {
            name: "Piano".to_string(),
            notes,
        }],
        None,
    );
    std::fs::write(&out_path, xml).expect("write xml");
    println!("wrote {out_path}");
}
