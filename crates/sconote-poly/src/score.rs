//! Note-level scoring of a transcription against MIDI ground truth.
//!
//! A predicted note counts as correct when it has the reference note's exact
//! MIDI pitch and its onset lies within a tolerance window — the standard
//! note-level transcription metric (offsets are deliberately ignored; decay
//! tails make them unreliable in real audio). Matching is greedy nearest-
//! onset per reference note, in onset order.

use crate::ground_truth::GroundTruthNote;

/// One note produced by a transcription.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TranscribedNote {
    /// MIDI note number (A4 = 69).
    pub midi: u8,
    pub onset_s: f64,
    /// End of the note; not scored (decay tails make offsets unreliable),
    /// but kept for display.
    pub offset_s: f64,
}

/// Outcome of [`score_notes`]: what matched and, for debugging a tuning run,
/// exactly which notes were missed or invented.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreReport {
    pub matched: usize,
    /// Reference notes no prediction accounted for.
    pub missed: Vec<GroundTruthNote>,
    /// Predictions no reference note accounts for.
    pub spurious: Vec<TranscribedNote>,
}

impl ScoreReport {
    pub fn precision(&self) -> f64 {
        ratio(self.matched, self.matched + self.spurious.len())
    }

    pub fn recall(&self) -> f64 {
        ratio(self.matched, self.matched + self.missed.len())
    }

    pub fn f1(&self) -> f64 {
        let (p, r) = (self.precision(), self.recall());
        if p + r == 0.0 {
            0.0
        } else {
            2.0 * p * r / (p + r)
        }
    }
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

/// Score `predicted` against `reference`: a prediction matches a reference
/// note when pitches are equal and onsets differ by at most
/// `onset_tolerance_s`. Each note matches at most once.
pub fn score_notes(
    reference: &[GroundTruthNote],
    predicted: &[TranscribedNote],
    onset_tolerance_s: f64,
) -> ScoreReport {
    let mut predicted_matched = vec![false; predicted.len()];
    let mut reference_order: Vec<usize> = (0..reference.len()).collect();
    reference_order.sort_by(|&a, &b| reference[a].onset_s.total_cmp(&reference[b].onset_s));

    let mut matched = 0;
    let mut missed = Vec::new();
    for &reference_index in &reference_order {
        let target = reference[reference_index];
        let best = predicted
            .iter()
            .enumerate()
            .filter(|&(i, p)| !predicted_matched[i] && p.midi == target.midi)
            .map(|(i, p)| (i, (p.onset_s - target.onset_s).abs()))
            .filter(|&(_, distance)| distance <= onset_tolerance_s)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        match best {
            Some((i, _)) => {
                predicted_matched[i] = true;
                matched += 1;
            }
            None => missed.push(target),
        }
    }
    let spurious = predicted
        .iter()
        .zip(&predicted_matched)
        .filter(|&(_, &was_matched)| !was_matched)
        .map(|(&note, _)| note)
        .collect();
    ScoreReport {
        matched,
        missed,
        spurious,
    }
}

#[cfg(test)]
#[path = "score_test.rs"]
mod score_test;
