use midly::num::{u4, u7, u15, u24, u28};
use midly::{
    Format, Fps, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind,
};

use super::*;

const TICKS_PER_QUARTER: u16 = 480;

fn note_on(channel: u8, key: u8) -> TrackEventKind<'static> {
    TrackEventKind::Midi {
        channel: u4::new(channel),
        message: MidiMessage::NoteOn {
            key: u7::new(key),
            vel: u7::new(100),
        },
    }
}

fn note_off(channel: u8, key: u8) -> TrackEventKind<'static> {
    TrackEventKind::Midi {
        channel: u4::new(channel),
        message: MidiMessage::NoteOff {
            key: u7::new(key),
            vel: u7::new(0),
        },
    }
}

fn smf_bytes(tracks: Vec<Vec<(u32, TrackEventKind<'static>)>>) -> Vec<u8> {
    let mut smf = Smf::new(Header {
        format: if tracks.len() > 1 {
            Format::Parallel
        } else {
            Format::SingleTrack
        },
        timing: Timing::Metrical(u15::new(TICKS_PER_QUARTER)),
    });
    for track in tracks {
        smf.tracks.push(
            track
                .into_iter()
                .map(|(delta, kind)| TrackEvent {
                    delta: u28::new(delta),
                    kind,
                })
                .collect(),
        );
    }
    let mut bytes = Vec::new();
    smf.write(&mut bytes).expect("in-memory write");
    bytes
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn converts_ticks_to_seconds_with_default_tempo() {
    // No tempo event → 120 BPM: one quarter (480 ticks) = 0.5 s.
    let bytes = smf_bytes(vec![vec![(480, note_on(0, 60)), (480, note_off(0, 60))]]);
    let notes = notes_from_midi(&bytes).expect("parse");
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].midi, 60);
    assert_close(notes[0].onset_s, 0.5);
    assert_close(notes[0].offset_s, 1.0);
}

#[test]
fn tempo_change_applies_from_its_tick() {
    // 250_000 µs/qn (240 BPM) from tick 480: a note at tick 960 sits one
    // quarter at 0.5 s plus one quarter at 0.25 s into the piece.
    let tempo = TrackEventKind::Meta(MetaMessage::Tempo(u24::new(250_000)));
    let bytes = smf_bytes(vec![
        vec![(480, tempo)],
        vec![(960, note_on(0, 60)), (480, note_off(0, 60))],
    ]);
    let notes = notes_from_midi(&bytes).expect("parse");
    assert_close(notes[0].onset_s, 0.75);
    assert_close(notes[0].offset_s, 1.0);
}

#[test]
fn same_tick_tempo_change_last_one_wins() {
    let slow = TrackEventKind::Meta(MetaMessage::Tempo(u24::new(1_000_000)));
    let fast = TrackEventKind::Meta(MetaMessage::Tempo(u24::new(250_000)));
    let bytes = smf_bytes(vec![vec![
        (0, slow),
        (0, fast),
        (480, note_on(0, 60)),
        (480, note_off(0, 60)),
    ]]);
    let notes = notes_from_midi(&bytes).expect("parse");
    assert_close(notes[0].onset_s, 0.25);
}

#[test]
fn note_on_with_zero_velocity_acts_as_note_off() {
    let off_via_on = TrackEventKind::Midi {
        channel: u4::new(0),
        message: MidiMessage::NoteOn {
            key: u7::new(60),
            vel: u7::new(0),
        },
    };
    let bytes = smf_bytes(vec![vec![(0, note_on(0, 60)), (480, off_via_on)]]);
    let notes = notes_from_midi(&bytes).expect("parse");
    assert_eq!(notes.len(), 1);
    assert_close(notes[0].offset_s, 0.5);
}

#[test]
fn percussion_channel_is_excluded() {
    let bytes = smf_bytes(vec![vec![
        (0, note_on(PERCUSSION_CHANNEL, 36)),
        (480, note_off(PERCUSSION_CHANNEL, 36)),
    ]]);
    assert_eq!(notes_from_midi(&bytes).expect("parse"), Vec::new());
}

#[test]
fn dangling_note_on_is_dropped() {
    let bytes = smf_bytes(vec![vec![(0, note_on(0, 60))]]);
    assert_eq!(notes_from_midi(&bytes).expect("parse"), Vec::new());
}

#[test]
fn overlapping_same_key_notes_pair_fifo() {
    let bytes = smf_bytes(vec![vec![
        (0, note_on(0, 60)),
        (240, note_on(0, 60)),
        (240, note_off(0, 60)),
        (240, note_off(0, 60)),
    ]]);
    let notes = notes_from_midi(&bytes).expect("parse");
    assert_eq!(notes.len(), 2);
    assert_close(notes[0].onset_s, 0.0);
    assert_close(notes[0].offset_s, 0.5);
    assert_close(notes[1].onset_s, 0.25);
    assert_close(notes[1].offset_s, 0.75);
}

#[test]
fn notes_are_sorted_by_onset_across_tracks() {
    let bytes = smf_bytes(vec![
        vec![(960, note_on(0, 72)), (480, note_off(0, 72))],
        vec![(0, note_on(1, 40)), (480, note_off(1, 40))],
    ]);
    let notes = notes_from_midi(&bytes).expect("parse");
    assert_eq!(
        notes.iter().map(|n| n.midi).collect::<Vec<_>>(),
        vec![40, 72]
    );
}

#[test]
fn timecode_timing_is_rejected() {
    let mut smf = Smf::new(Header {
        format: Format::SingleTrack,
        timing: Timing::Timecode(Fps::Fps24, 4),
    });
    smf.tracks.push(Vec::new());
    let mut bytes = Vec::new();
    smf.write(&mut bytes).expect("in-memory write");
    let error = notes_from_midi(&bytes).expect_err("timecode must be rejected");
    assert!(matches!(error, GroundTruthError::TimecodeTiming));
}

#[test]
fn garbage_bytes_are_a_parse_error() {
    let error = notes_from_midi(b"not a midi file").expect_err("garbage must fail");
    assert!(matches!(error, GroundTruthError::Parse(_)));
    assert!(error.to_string().starts_with("invalid MIDI file"));
}
