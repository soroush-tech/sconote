//! Transcribed notes → a playable Standard MIDI File.

use midly::num::{u4, u7, u15, u24, u28};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

use crate::score::TranscribedNote;

const TICKS_PER_QUARTER: u16 = 480;
/// 120 BPM: with 480 ticks per quarter, ticks map to wall-clock time at
/// exactly 960 ticks per second.
const TEMPO_US_PER_QN: u32 = 500_000;
const VELOCITY: u8 = 90;

/// Encode notes as a single-track MIDI file on a 120 BPM grid.
pub fn notes_to_midi_bytes(notes: &[TranscribedNote]) -> Vec<u8> {
    let ticks_per_second = f64::from(TICKS_PER_QUARTER) * 1e6 / f64::from(TEMPO_US_PER_QN);
    // (tick, is_note_on, midi) - offs sort before ons at the same tick so
    // back-to-back repeats of a pitch stay two separate notes.
    let mut events: Vec<(u64, bool, u8)> = Vec::with_capacity(notes.len() * 2);
    for note in notes {
        let midi = note.midi.min(127);
        let onset = (note.onset_s.max(0.0) * ticks_per_second).round() as u64;
        let offset = ((note.offset_s * ticks_per_second).round() as u64).max(onset + 1);
        events.push((onset, true, midi));
        events.push((offset, false, midi));
    }
    events.sort_by_key(|&(tick, is_on, midi)| (tick, is_on, midi));

    let mut smf = Smf::new(Header {
        format: Format::SingleTrack,
        timing: Timing::Metrical(u15::new(TICKS_PER_QUARTER)),
    });
    let mut track = vec![TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(TEMPO_US_PER_QN))),
    }];
    let mut last_tick = 0;
    for (tick, is_on, midi) in events {
        let message = if is_on {
            MidiMessage::NoteOn {
                key: u7::new(midi),
                vel: u7::new(VELOCITY),
            }
        } else {
            MidiMessage::NoteOff {
                key: u7::new(midi),
                vel: u7::new(0),
            }
        };
        track.push(TrackEvent {
            delta: u28::new((tick - last_tick) as u32),
            kind: TrackEventKind::Midi {
                channel: u4::new(0),
                message,
            },
        });
        last_tick = tick;
    }
    track.push(TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });
    smf.tracks.push(track);
    let mut bytes = Vec::new();
    // Writing into a Vec cannot fail.
    smf.write(&mut bytes).expect("in-memory write");
    bytes
}

#[cfg(test)]
#[path = "midi_export_test.rs"]
mod midi_export_test;
