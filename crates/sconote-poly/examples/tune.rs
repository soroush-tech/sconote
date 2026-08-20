//! Tuning driver: transcribe a real recording, auto-align it against MIDI
//! ground truth (the recording starts at an unknown point in the song), and
//! report note-level accuracy for a grid of thresholds.
//!
//! Usage: cargo run -p sconote-poly --example tune -- <session.wav|mp3> <file.mid>...

use std::collections::HashMap;
use std::time::Instant;

use sconote_poly::{
    compute_activations, notes_from_midi, read_audio_mono, score_notes, BasicPitch,
    GroundTruthNote, NoteCreationOptions, TranscribedNote,
};

const ONSET_TOLERANCE_S: f64 = 0.15;
/// Ignore partially-covered reference notes this close to the clip edges.
const EDGE_MARGIN_S: f64 = 0.5;

fn main() {
    let mut args = std::env::args().skip(1);
    let audio_path = args
        .next()
        .expect("usage: tune <session.wav|mp3> <file.mid>...");
    let midi_paths: Vec<String> = args.collect();
    assert!(!midi_paths.is_empty(), "give at least one MIDI file");

    let audio =
        read_audio_mono(&std::fs::read(&audio_path).expect("read audio")).expect("decode audio");
    let duration_s = audio.samples.len() as f64 / f64::from(audio.sample_rate);
    println!("recording: {duration_s:.1} s at {} Hz", audio.sample_rate);

    let model = BasicPitch::new().expect("model");
    let t0 = Instant::now();
    let activations = compute_activations(&audio, &model).expect("inference");
    println!("inference: {:.1} s", t0.elapsed().as_secs_f64());

    let notes = activations.to_notes(&NoteCreationOptions::default());
    println!("transcribed notes (default thresholds): {}", notes.len());

    // Align against each candidate MIDI; keep the best.
    let mut best: Option<(String, Vec<GroundTruthNote>, f64, usize)> = None;
    for path in &midi_paths {
        let bytes = std::fs::read(path).expect("read midi");
        let Ok(reference) = notes_from_midi(&bytes) else {
            println!("{path}: unparseable, skipped");
            continue;
        };
        let (offset, matched) = best_offset(&reference, &notes);
        println!("{path}: best offset {offset:+.2} s, {matched} onset hits");
        if best.as_ref().is_none_or(|&(_, _, _, m)| matched > m) {
            best = Some((path.clone(), reference, offset, matched));
        }
    }
    let (path, reference, offset, _) = best.expect("no parseable MIDI");
    println!("\nusing {path} at offset {offset:+.2} s");

    // Threshold grid on the cached activations.
    println!(
        "\n{:>5} {:>5} {:>7} {:>7} {:>9} {:>9} {:>7}",
        "onset", "frame", "pred", "match", "precision", "recall", "f1"
    );
    let mut grid_best = (0.0, NoteCreationOptions::default());
    for onset_threshold in [0.3, 0.4, 0.5, 0.6] {
        for frame_threshold in [0.2, 0.3, 0.4] {
            let options = NoteCreationOptions {
                onset_threshold,
                frame_threshold,
                ..NoteCreationOptions::default()
            };
            let notes = activations.to_notes(&options);
            let report = score_clip(&reference, &notes, offset, duration_s);
            println!(
                "{:>5.2} {:>5.2} {:>7} {:>7} {:>9.3} {:>9.3} {:>7.3}",
                onset_threshold,
                frame_threshold,
                notes.len(),
                report.matched,
                report.precision(),
                report.recall(),
                report.f1()
            );
            if report.f1() > grid_best.0 {
                grid_best = (report.f1(), options);
            }
        }
    }

    // The overtone-ghost filter, swept alone on top of the defaults.
    println!(
        "\n{:<14} {:>6} {:>7} {:>9} {:>9} {:>7}",
        "overtone", "pred", "match", "precision", "recall", "f1"
    );
    for overtone_ghost_energy_ratio in [0.0_f32, 0.4, 0.6, 0.8, 1.0] {
        let options = NoteCreationOptions {
            overtone_ghost_energy_ratio,
            ..NoteCreationOptions::default()
        };
        let notes = activations.to_notes(&options);
        let report = score_clip(&reference, &notes, offset, duration_s);
        println!(
            "{overtone_ghost_energy_ratio:<14.2} {:>6} {:>7} {:>9.3} {:>9.3} {:>7.3}",
            notes.len(),
            report.matched,
            report.precision(),
            report.recall(),
            report.f1()
        );
    }

    // Minimum note length × overtone ratio: the two interact - a lower
    // length gate recovers fast notes but lets in short ghosts the
    // overtone filter must then catch.
    println!(
        "\n{:>7} {:>8} {:>6} {:>7} {:>9} {:>9} {:>7}",
        "min_len", "overtone", "pred", "match", "precision", "recall", "f1"
    );
    for min_note_len_frames in [3_usize, 5, 7, 9, 11] {
        for overtone_ghost_energy_ratio in [0.4_f32, 0.6, 0.8] {
            let options = NoteCreationOptions {
                min_note_len_frames,
                overtone_ghost_energy_ratio,
                ..NoteCreationOptions::default()
            };
            let notes = activations.to_notes(&options);
            let report = score_clip(&reference, &notes, offset, duration_s);
            println!(
                "{min_note_len_frames:>7} {overtone_ghost_energy_ratio:>8.2} {:>6} {:>7} {:>9.3} {:>9.3} {:>7.3}",
                notes.len(),
                report.matched,
                report.precision(),
                report.recall(),
                report.f1()
            );
        }
    }

    // Retrigger dip ratio × strict no-dip bar: the dip admits genuine
    // re-strikes at the plain threshold, so the no-dip bar can afford to
    // be stricter (1.1 means a re-articulation always needs a dip).
    println!(
        "\n{:>5} {:>7} {:>6} {:>7} {:>9} {:>9} {:>7}",
        "dip", "bar", "pred", "match", "precision", "recall", "f1"
    );
    for retrigger_dip_ratio in [0.0_f32, 0.5, 0.6, 0.7, 0.8, 0.9] {
        for retrigger_onset_threshold in [0.7_f32, 0.8, 0.9, 1.1] {
            let options = NoteCreationOptions {
                retrigger_dip_ratio,
                retrigger_onset_threshold,
                ..NoteCreationOptions::default()
            };
            let notes = activations.to_notes(&options);
            let report = score_clip(&reference, &notes, offset, duration_s);
            println!(
                "{retrigger_dip_ratio:>5.2} {retrigger_onset_threshold:>7.2} {:>6} {:>7} {:>9.3} {:>9.3} {:>7.3}",
                notes.len(),
                report.matched,
                report.precision(),
                report.recall(),
                report.f1()
            );
        }
    }

    // Detail at the best setting: what is missed, per octave.
    let (f1, options) = grid_best;
    let notes = activations.to_notes(&options);
    let report = score_clip(&reference, &notes, offset, duration_s);
    println!(
        "\nbest: onset={} frame={} → f1={f1:.3} ({} matched, {} missed, {} spurious)",
        options.onset_threshold,
        options.frame_threshold,
        report.matched,
        report.missed.len(),
        report.spurious.len()
    );
    let mut missed_by_octave: HashMap<u8, usize> = HashMap::new();
    for note in &report.missed {
        *missed_by_octave.entry(note.midi / 12).or_default() += 1;
    }
    let mut octaves: Vec<_> = missed_by_octave.into_iter().collect();
    octaves.sort();
    println!("missed by octave (midi/12): {octaves:?}");
}

/// Score with reference times shifted into recording time, keeping only
/// reference notes the clip fully covers.
fn score_clip(
    reference: &[GroundTruthNote],
    predicted: &[TranscribedNote],
    offset: f64,
    duration_s: f64,
) -> sconote_poly::ScoreReport {
    let clipped: Vec<GroundTruthNote> = reference
        .iter()
        .filter(|note| {
            note.onset_s - offset >= EDGE_MARGIN_S
                && note.onset_s - offset <= duration_s - EDGE_MARGIN_S
        })
        .map(|note| GroundTruthNote {
            midi: note.midi,
            onset_s: note.onset_s - offset,
            offset_s: note.offset_s - offset,
        })
        .collect();
    score_notes(&clipped, predicted, ONSET_TOLERANCE_S)
}

/// Find the reference-time offset maximizing onset hits: coarse 0.1 s scan
/// over the whole song, then a 0.01 s refinement around the winner.
fn best_offset(reference: &[GroundTruthNote], predicted: &[TranscribedNote]) -> (f64, usize) {
    let mut by_pitch: HashMap<u8, Vec<f64>> = HashMap::new();
    for note in reference {
        by_pitch.entry(note.midi).or_default().push(note.onset_s);
    }
    for onsets in by_pitch.values_mut() {
        onsets.sort_by(f64::total_cmp);
    }
    let count_hits = |offset: f64| {
        predicted
            .iter()
            .filter(|p| {
                by_pitch.get(&p.midi).is_some_and(|onsets| {
                    let target = p.onset_s + offset;
                    let i = onsets.partition_point(|&o| o < target);
                    [i.wrapping_sub(1), i]
                        .iter()
                        .filter_map(|&j| onsets.get(j))
                        .any(|&o| (o - target).abs() <= ONSET_TOLERANCE_S)
                })
            })
            .count()
    };
    // The recording may begin with silence before playback starts, so the
    // offset can be as negative as the whole recording length.
    let scan_start = -predicted.iter().map(|n| n.offset_s).fold(0.0, f64::max) - 5.0;
    let song_end = reference.iter().map(|n| n.offset_s).fold(0.0, f64::max);
    let coarse = (0..)
        .map(|i| scan_start + 0.1 * f64::from(i))
        .take_while(|&offset| offset < song_end)
        .map(|offset| (offset, count_hits(offset)))
        .max_by_key(|&(_, hits)| hits)
        .expect("non-empty scan");
    (-20..=20)
        .map(|i| coarse.0 + 0.01 * f64::from(i))
        .map(|offset| (offset, count_hits(offset)))
        .max_by_key(|&(_, hits)| hits)
        .expect("non-empty scan")
}
