//! One-off: print transcribed notes in a time window of a recording.

use std::fs::File;

use sconote_poly::{BasicPitch, NoteCreationOptions, compute_activations, read_wav_mono};

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

fn main() {
    let mut args = std::env::args().skip(1);
    let wav_path = args.next().expect("usage: peek <wav> <from_s> <to_s>");
    let from: f64 = args.next().expect("from").parse().expect("from");
    let to: f64 = args.next().expect("to").parse().expect("to");

    let audio = read_wav_mono(File::open(&wav_path).expect("open")).expect("decode");
    let model = BasicPitch::new().expect("model");
    let activations = compute_activations(&audio, &model).expect("inference");
    for note in activations.to_notes(&NoteCreationOptions::default()) {
        if note.onset_s >= from && note.onset_s < to {
            println!(
                "{:7.2}s  {:>3}{}  ({:.2}s long)",
                note.onset_s,
                NOTE_NAMES[usize::from(note.midi) % 12],
                i32::from(note.midi) / 12 - 1,
                note.offset_s - note.onset_s,
            );
        }
    }
}
