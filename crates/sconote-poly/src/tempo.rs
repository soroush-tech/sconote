//! Tempo from transcribed note onsets: a global BPM estimate
//! ([`estimate_bpm`]) and a beat grid that follows tempo drift
//! ([`track_beats`]).
//!
//! Chord-clustered onsets yield inter-onset intervals; the winning tempo is
//! the beat period under which those intervals sit closest to whole numbers
//! of beats. Tempo "octave" ambiguity (60 vs 120) is inherent to the
//! problem, so candidates are confined to a musically likely range —
//! misjudging the octave only changes notation granularity, not the notes.

use crate::score::TranscribedNote;

const MIN_BPM: f64 = 40.0;
const MAX_BPM: f64 = 200.0;
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

/// Note onsets collapsed into rhythmic events: sorted, with anything closer
/// than [`CLUSTER_S`] (a chord) merged into one.
fn onset_events(notes: &[TranscribedNote]) -> Vec<f64> {
    let mut onsets: Vec<f64> = notes.iter().map(|note| note.onset_s).collect();
    onsets.sort_by(f64::total_cmp);
    let mut events: Vec<f64> = Vec::new();
    for onset in onsets {
        if events.last().is_none_or(|&last| onset - last > CLUSTER_S) {
            events.push(onset);
        }
    }
    events
}

/// Estimate the tempo of a transcription; [`DEFAULT_BPM`] when there is
/// not enough rhythmic evidence.
pub fn estimate_bpm(notes: &[TranscribedNote]) -> f64 {
    let events = onset_events(notes);
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

/// How far from the predicted beat an onset may sit and still capture it,
/// as a fraction of the current beat period. Below half a period, so a
/// subdivision halfway between beats is never mistaken for the beat.
const BEAT_SNAP_WINDOW: f64 = 0.3;
/// Weight of the newest inter-beat interval in the running period — caps
/// tempo drift at ~9% per beat while riding out one early or late onset.
const PERIOD_BLEND: f64 = 0.3;

/// Beat times of a performance whose tempo may drift (rubato, ritardando).
///
/// A phase-locked walk: beats start at the first rhythmic event and step by
/// the running period; each predicted beat snaps to the nearest onset event
/// within [`BEAT_SNAP_WINDOW`] (updating the period), or coasts on the
/// prediction across rests and off-beat stretches. With too little
/// rhythmic evidence the grid is uniform at [`estimate_bpm`]'s tempo. The
/// returned grid is strictly increasing, has at least two beats, and
/// covers the last note offset.
pub fn track_beats(notes: &[TranscribedNote]) -> Vec<f64> {
    let mut period = 60.0 / estimate_bpm(notes);
    let events = onset_events(notes);
    let end = notes.iter().map(|note| note.offset_s).fold(0.0, f64::max);

    let mut beats = vec![events.first().copied().unwrap_or(0.0)];
    if events.len() >= MIN_EVENTS {
        let last_event = *events.last().expect("events checked non-empty");
        while *beats.last().expect("beats starts non-empty") < last_event {
            let last = *beats.last().expect("beats starts non-empty");
            let predicted = last + period;
            let split = events.partition_point(|&event| event < predicted);
            let nearest = [split.checked_sub(1), Some(split)]
                .into_iter()
                .flatten()
                .filter_map(|i| events.get(i).copied())
                .filter(|&event| event > last)
                .min_by(|a, b| {
                    (a - predicted).abs().total_cmp(&(b - predicted).abs())
                });
            let next = match nearest {
                Some(event) if (event - predicted).abs() <= period * BEAT_SNAP_WINDOW => {
                    period = (1.0 - PERIOD_BLEND) * period + PERIOD_BLEND * (event - last);
                    event
                }
                _ => predicted,
            };
            beats.push(next);
        }
    }
    while beats.len() < 2 || *beats.last().expect("beats starts non-empty") < end {
        beats.push(beats.last().expect("beats starts non-empty") + period);
    }
    beats
}

#[cfg(test)]
#[path = "tempo_test.rs"]
mod tempo_test;
