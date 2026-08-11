//! MIDI file → ground-truth note list.
//!
//! A Standard MIDI File is the reference for what a transcription *should*
//! find: every pitched note with its onset/offset in wall-clock seconds.
//! Percussion (channel 10) carries no pitch and is excluded.

use std::collections::{HashMap, VecDeque};

use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

/// MIDI channel 10 (0-indexed 9) is percussion in General MIDI.
const PERCUSSION_CHANNEL: u8 = 9;
/// Tempo before the first tempo event, per the SMF spec (120 BPM).
const DEFAULT_TEMPO_US_PER_QN: u32 = 500_000;

/// One pitched note the audio is expected to contain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundTruthNote {
    /// MIDI note number (A4 = 69).
    pub midi: u8,
    pub onset_s: f64,
    pub offset_s: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum GroundTruthError {
    #[error("invalid MIDI file: {0}")]
    Parse(#[from] midly::Error),
    #[error("SMPTE timecode timing is not supported")]
    TimecodeTiming,
}

/// Extract every pitched note from a Standard MIDI File, sorted by onset.
///
/// Tempo events from all tracks form one global tempo map (correct for
/// format 0 and 1 files). Note-on/note-off pairs match FIFO per
/// channel + key; dangling note-ons are dropped.
pub fn notes_from_midi(bytes: &[u8]) -> Result<Vec<GroundTruthNote>, GroundTruthError> {
    let smf = Smf::parse(bytes)?;
    let ticks_per_quarter = match smf.header.timing {
        Timing::Metrical(ticks) => u32::from(ticks.as_int()),
        Timing::Timecode(..) => return Err(GroundTruthError::TimecodeTiming),
    };

    let mut tempo_changes: Vec<(u64, u32)> = Vec::new();
    // (absolute tick, channel, key, is_note_on)
    let mut events: Vec<(u64, u8, u8, bool)> = Vec::new();
    for track in &smf.tracks {
        let mut tick: u64 = 0;
        for event in track {
            tick += u64::from(event.delta.as_int());
            match event.kind {
                TrackEventKind::Meta(MetaMessage::Tempo(us_per_qn)) => {
                    tempo_changes.push((tick, us_per_qn.as_int()));
                }
                TrackEventKind::Midi { channel, message }
                    if channel.as_int() != PERCUSSION_CHANNEL =>
                {
                    match message {
                        MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                            events.push((tick, channel.as_int(), key.as_int(), true));
                        }
                        // A note-on with velocity 0 is a note-off by convention.
                        MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                            events.push((tick, channel.as_int(), key.as_int(), false));
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    let tempo_map = TempoMap::new(ticks_per_quarter, tempo_changes);
    // Offs sort before ons at the same tick, so back-to-back repeats of the
    // same key close the first note before opening the next.
    events.sort_by_key(|&(tick, _, _, is_on)| (tick, is_on));

    let mut open: HashMap<(u8, u8), VecDeque<u64>> = HashMap::new();
    let mut notes = Vec::new();
    for (tick, channel, key, is_on) in events {
        if is_on {
            open.entry((channel, key)).or_default().push_back(tick);
        } else if let Some(onset_tick) = open.get_mut(&(channel, key)).and_then(VecDeque::pop_front)
        {
            notes.push(GroundTruthNote {
                midi: key,
                onset_s: tempo_map.seconds_at(onset_tick),
                offset_s: tempo_map.seconds_at(tick),
            });
        }
    }
    notes.sort_by(|a, b| a.onset_s.total_cmp(&b.onset_s).then(a.midi.cmp(&b.midi)));
    Ok(notes)
}

/// Tick → seconds conversion under a piecewise-constant tempo.
struct TempoMap {
    /// Segments ordered by tick; the first always starts at tick 0.
    segments: Vec<TempoSegment>,
}

struct TempoSegment {
    start_tick: u64,
    start_s: f64,
    s_per_tick: f64,
}

impl TempoMap {
    fn new(ticks_per_quarter: u32, mut changes: Vec<(u64, u32)>) -> TempoMap {
        let s_per_tick = |us_per_qn: u32| f64::from(us_per_qn) / f64::from(ticks_per_quarter) / 1e6;
        changes.sort_by_key(|&(tick, _)| tick);
        let mut segments = Vec::with_capacity(changes.len() + 1);
        let mut current = TempoSegment {
            start_tick: 0,
            start_s: 0.0,
            s_per_tick: s_per_tick(DEFAULT_TEMPO_US_PER_QN),
        };
        for (tick, us_per_qn) in changes {
            let start_s = current.start_s + (tick - current.start_tick) as f64 * current.s_per_tick;
            if tick > current.start_tick {
                segments.push(current);
            }
            // Same-tick change replaces the current segment (last one wins).
            current = TempoSegment {
                start_tick: tick,
                start_s,
                s_per_tick: s_per_tick(us_per_qn),
            };
        }
        segments.push(current);
        TempoMap { segments }
    }

    fn seconds_at(&self, tick: u64) -> f64 {
        // Never underflows: segments[0].start_tick == 0 by construction.
        let index = self.segments.partition_point(|seg| seg.start_tick <= tick) - 1;
        let seg = &self.segments[index];
        seg.start_s + (tick - seg.start_tick) as f64 * seg.s_per_tick
    }
}

#[cfg(test)]
#[path = "ground_truth_test.rs"]
mod ground_truth_test;

#[cfg(test)]
#[path = "ground_truth_spec.rs"]
mod ground_truth_spec;
