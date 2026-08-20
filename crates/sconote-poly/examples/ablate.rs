//! Ablation driver: which note-creation stage invents the extra notes?
//! Re-extracts notes from one cached activation matrix under each
//! combination of the two heuristics, scored against MIDI ground truth, with
//! a count of held notes that got re-triggered mid-sustain.
//!
//! Expects a render of the reference MIDI (no time offset to solve for) -
//! for a real recording use the `tune` example instead.
//!
//! Usage: cargo run --release -p sconote-poly --example ablate -- <render.wav> <reference.mid>

use sconote_poly::{
    BasicPitch, GroundTruthNote, NoteCreationOptions, TranscribedNote, compute_activations,
    notes_from_midi, read_audio_mono, score_notes,
};

const ONSET_TOLERANCE_S: f64 = 0.05;

fn main() {
    let mut args = std::env::args().skip(1);
    let usage = "usage: ablate <render.wav> <reference.mid>";
    let wav_path = args.next().expect(usage);
    let midi_path = args.next().expect(usage);

    let audio =
        read_audio_mono(&std::fs::read(&wav_path).expect("read audio")).expect("decode audio");
    let reference =
        notes_from_midi(&std::fs::read(&midi_path).expect("read midi")).expect("parse midi");
    let model = BasicPitch::new().expect("model");
    let activations = compute_activations(&audio, &model).expect("inference");
    println!("reference: {} notes", reference.len());

    println!(
        "\n{:>5} {:>9} {:>8} {:>6} {:>6} {:>10} {:>7} {:>6} {:>7}",
        "onset", "retrigger", "melodia", "notes", "match", "precision", "recall", "f1", "splits"
    );
    for melodia_trick in [true, false] {
        for onset_threshold in [0.5, 0.6, 0.7] {
            // The first entry is the ungated baseline (Basic Pitch's single
            // threshold); a bar below it would make no sense.
            let mut bars = vec![onset_threshold, 0.6, 0.7, 0.8, 0.9];
            bars.retain(|&bar| bar >= onset_threshold);
            bars.dedup();
            for retrigger_onset_threshold in bars {
                let options = NoteCreationOptions {
                    onset_threshold,
                    retrigger_onset_threshold,
                    melodia_trick,
                    ..NoteCreationOptions::default()
                };
                let notes = activations.to_notes(&options);
                let report = score_notes(&reference, &notes, ONSET_TOLERANCE_S);
                println!(
                    "{onset_threshold:>5.2} {retrigger_onset_threshold:>9.2} \
                     {melodia_trick:>8} {:>6} {:>6} {:>10.3} {:>7.3} {:>6.3} {:>7}",
                    notes.len(),
                    report.matched,
                    report.precision(),
                    report.recall(),
                    report.f1(),
                    retriggered(&reference, &notes),
                );
            }
        }
    }

    // The onset-pass ghost filter, swept alone on top of the defaults.
    println!(
        "\n{:<16} {:>6} {:>6} {:>10} {:>7} {:>6} {:>7}",
        "ghost ratio", "notes", "match", "precision", "recall", "f1", "splits"
    );
    for onset_ghost_energy_ratio in [0.0, 0.4, 0.6, 0.8, 1.0] {
        let options = NoteCreationOptions {
            onset_ghost_energy_ratio,
            ..NoteCreationOptions::default()
        };
        let notes = activations.to_notes(&options);
        let report = score_notes(&reference, &notes, ONSET_TOLERANCE_S);
        println!(
            "{onset_ghost_energy_ratio:<16.2} {:>6} {:>6} {:>10.3} {:>7.3} {:>6.3} {:>7}",
            notes.len(),
            report.matched,
            report.precision(),
            report.recall(),
            report.f1(),
            retriggered(&reference, &notes),
        );
    }

    // The octave-strike veto, swept alone on top of the defaults.
    println!(
        "\n{:<16} {:>6} {:>6} {:>10} {:>7} {:>6} {:>7}",
        "octave veto", "notes", "match", "precision", "recall", "f1", "splits"
    );
    for retrigger_octave_veto in [0.0_f32, 0.5, 0.75, 1.0, 1.25, 1.5] {
        let options = NoteCreationOptions {
            retrigger_octave_veto,
            ..NoteCreationOptions::default()
        };
        let notes = activations.to_notes(&options);
        let report = score_notes(&reference, &notes, ONSET_TOLERANCE_S);
        println!(
            "{retrigger_octave_veto:<16.2} {:>6} {:>6} {:>10.3} {:>7.3} {:>6.3} {:>7}",
            notes.len(),
            report.matched,
            report.precision(),
            report.recall(),
            report.f1(),
            retriggered(&reference, &notes),
        );
    }

    // The overtone-ghost filter, swept alone on top of the defaults.
    println!(
        "\n{:<16} {:>6} {:>6} {:>10} {:>7} {:>6} {:>7}",
        "overtone ratio", "notes", "match", "precision", "recall", "f1", "splits"
    );
    for overtone_ghost_energy_ratio in [0.0_f32, 0.4, 0.6, 0.8, 1.0] {
        let options = NoteCreationOptions {
            overtone_ghost_energy_ratio,
            ..NoteCreationOptions::default()
        };
        let notes = activations.to_notes(&options);
        let report = score_notes(&reference, &notes, ONSET_TOLERANCE_S);
        println!(
            "{overtone_ghost_energy_ratio:<16.2} {:>6} {:>6} {:>10.3} {:>7.3} {:>6.3} {:>7}",
            notes.len(),
            report.matched,
            report.precision(),
            report.recall(),
            report.f1(),
            retriggered(&reference, &notes),
        );
    }

    // Where the gate costs recall: by how long the pitch had been silent.
    for retrigger_onset_threshold in [0.5, 0.7, 0.8] {
        println!("\nretrigger bar {retrigger_onset_threshold:.2}, recall by preceding silence:");
        let notes = activations.to_notes(&NoteCreationOptions {
            retrigger_onset_threshold,
            ..NoteCreationOptions::default()
        });
        recall_by_silence(&reference, &notes);
    }
}

/// Recall split by how long the same pitch had been silent before each
/// reference note. This is the axis the re-articulation gate acts on, and
/// the one tempo moves: the faster the piece, the more of its repeated notes
/// land in the short-silence buckets where the strict bar applies.
fn recall_by_silence(reference: &[GroundTruthNote], notes: &[TranscribedNote]) {
    const BUCKETS: [(&str, f64); 5] = [
        ("still ringing", 0.0),
        ("<100 ms", 0.1),
        ("<250 ms", 0.25),
        ("<500 ms", 0.5),
        ("500 ms+", f64::INFINITY),
    ];
    let report = score_notes(reference, notes, ONSET_TOLERANCE_S);
    let missed = |note: &GroundTruthNote| {
        report
            .missed
            .iter()
            .any(|m| m.midi == note.midi && m.onset_s == note.onset_s)
    };

    let mut totals = [(0_usize, 0_usize); BUCKETS.len()];
    for (index, note) in reference.iter().enumerate() {
        // Silence since this pitch last stopped - negative when the previous
        // note of the same pitch is still sounding, huge on a first hearing.
        let last_offset = reference[..index]
            .iter()
            .filter(|earlier| earlier.midi == note.midi)
            .map(|earlier| earlier.offset_s)
            .fold(f64::MIN, f64::max);
        let silence = note.onset_s - last_offset;
        let bucket = BUCKETS.iter().position(|&(_, cap)| silence < cap).unwrap_or(4);
        totals[bucket].0 += 1;
        totals[bucket].1 += usize::from(!missed(note));
    }
    for (&(label, _), &(total, found)) in BUCKETS.iter().zip(&totals) {
        if total > 0 {
            println!(
                "  {label:>13}: {found:>4}/{total:<4} recall {:.3}",
                found as f64 / total as f64
            );
        }
    }
}

/// Notes starting well after the onset of a reference note of the same pitch
/// that is still sounding - i.e. one held note emitted as several.
fn retriggered(reference: &[GroundTruthNote], notes: &[TranscribedNote]) -> usize {
    notes
        .iter()
        .filter(|note| {
            reference.iter().any(|held| {
                held.midi == note.midi
                    && held.onset_s + ONSET_TOLERANCE_S < note.onset_s
                    && held.offset_s > note.onset_s
            })
        })
        .count()
}
