//! One-off: transcribe an audio file and write the notes as a MIDI file.
//!
//! Usage: cargo run --release -p sconote-poly --example to_midi -- <in.wav|mp3> <out.mid>

use sconote_poly::{
    notes_to_midi_bytes, read_audio_mono, transcribe, BasicPitch, NoteCreationOptions,
};

fn main() {
    let mut args = std::env::args().skip(1);
    let usage = "usage: to_midi <in.wav|mp3> <out.mid>";
    let audio_path = args.next().expect(usage);
    let midi_path = args.next().expect(usage);

    let audio =
        read_audio_mono(&std::fs::read(&audio_path).expect("read audio")).expect("decode audio");
    let model = BasicPitch::new().expect("model");
    let notes = transcribe(&audio, &model, &NoteCreationOptions::default()).expect("transcription");
    println!("{} notes", notes.len());
    std::fs::write(&midi_path, notes_to_midi_bytes(&notes)).expect("write midi");
}
