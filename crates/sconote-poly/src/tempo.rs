//! Beat-period (BPM) estimation from transcribed note onsets.
//!
//! Chord-clustered onsets yield inter-onset intervals; the winning tempo is
//! the beat period under which those intervals sit closest to whole numbers
//! of beats. Tempo "octave" ambiguity (60 vs 120) is inherent to the
//! problem, so candidates are confined to a musically likely range —
//! misjudging the octave only changes notation granularity, not the notes.

use crate::score::TranscribedNote;

const MIN_BPM: f64 = 70.0;
const MAX_BPM: f64 = 160.0;
const BPM_STEP: f64 = 0.5;
/// Onsets closer than this are one rhythmic event (a chord).
const CLUSTER_S: f64 = 0.05;
/// With too few events there is no rhythm to measure.
const MIN_EVENTS: usize = 8;
/// How far apart (in events) interval pairs are still considered.
const MAX_SPAN: usize = 4;
/// An interval must land within this of a whole beat multiple to count.
const TOLERANCE_S: f64 = 0.035;
pub const DEFAULT_BPM: f64 = 120.0;

/// Estimate the tempo of a transcription; [`DEFAULT_BPM`] when there is
/// not enough rhythmic evidence.
pub fn estimate_bpm(notes: &[TranscribedNote]) -> f64 {
    let mut onsets: Vec<f64> = notes.iter().map(|note| note.onset_s).collect();
    onsets.sort_by(f64::total_cmp);
    let mut events: Vec<f64> = Vec::new();
    for onset in onsets {
        if events.last().is_none_or(|&last| onset - last > CLUSTER_S) {
            events.push(onset);
        }
    }
    if events.len() < MIN_EVENTS {
        return DEFAULT_BPM;
    }

    let mut intervals: Vec<f64> = Vec::new();
    for (i, &event) in events.iter().enumerate() {
        for later in &events[i + 1..(i + 1 + MAX_SPAN).min(events.len())] {
            let interval = later - event;
            if (0.1..4.0).contains(&interval) {
                intervals.push(interval);
            }
        }
    }

    let mut best = (DEFAULT_BPM, f64::MIN);
    let mut bpm = MIN_BPM;
    while bpm <= MAX_BPM {
        let period = 60.0 / bpm;
        let score: f64 = intervals
            .iter()
            .map(|&interval| {
                let beats = (interval / period).round().max(1.0);
                let error = (interval - beats * period).abs();
                // Short multiples are stronger evidence than long ones.
                (TOLERANCE_S - error).max(0.0) / beats
            })
            .sum();
        if score > best.1 {
            best = (bpm, score);
        }
        bpm += BPM_STEP;
    }
    best.0
}

#[cfg(test)]
#[path = "tempo_test.rs"]
mod tempo_test;
