//! Transcribed notes → MusicXML, the interchange format every engraving
//! tool (OpenSheetMusicDisplay, MuseScore, ...) renders as sheet music.
//!
//! The conversion makes the notation decisions a score needs beyond raw
//! notes: onsets/durations quantize to a 16th grid on a beat time-line -
//! uniform at a caller-given tempo, or tracked through the performance's
//! rubato by [`track_beats`] - with `<sound tempo>` marks emitted where the
//! tracked tempo moves. A Krumhansl-style key detector picks the key
//! signature (so B♭ material is spelled with flats, not A♯), and each part
//! is a piano grand staff split at middle C. Time signature is fixed 4/4;
//! notes are truncated at barlines rather than tied - a readable
//! approximation, not full engraving-grade rhythm transcription.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::score::TranscribedNote;
use crate::tempo::track_beats;

/// One named instrument in the score.
pub struct ScorePart {
    pub name: String,
    pub notes: Vec<TranscribedNote>,
}

/// Grid units per quarter note (16th-note resolution).
const DIVISIONS: usize = 4;
/// 4/4 time: units per measure.
const MEASURE_UNITS: usize = 16;
/// Notes below middle C go to the bass staff.
const TREBLE_SPLIT_MIDI: u8 = 60;

/// Expressible note lengths in grid units, longest first.
const NOTE_TYPES: [(usize, &str, bool); 8] = [
    (16, "whole", false),
    (12, "half", true),
    (8, "half", false),
    (6, "quarter", true),
    (4, "quarter", false),
    (3, "eighth", true),
    (2, "eighth", false),
    (1, "16th", false),
];

/// Render one or more instrument parts as a MusicXML score. `bpm` fixes a
/// uniform beat grid; `None` tracks the beat through the performance
/// ([`track_beats`]), so rubato and ritardandi land on the right beats.
pub fn parts_to_musicxml(parts: &[ScorePart], bpm: Option<f64>) -> String {
    let beats = match bpm {
        Some(bpm) => uniform_beats(parts, bpm),
        None => {
            let mut all: Vec<TranscribedNote> =
                parts.iter().flat_map(|part| part.notes.iter().copied()).collect();
            all.sort_by(|a, b| a.onset_s.total_cmp(&b.onset_s));
            fold_to_notation_pulse(track_beats(&all))
        }
    };
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <score-partwise version=\"3.1\">\n<part-list>\n",
    );
    for (index, part) in parts.iter().enumerate() {
        let _ = writeln!(
            xml,
            "<score-part id=\"P{}\"><part-name>{}</part-name></score-part>",
            index + 1,
            escape(&part.name),
        );
    }
    xml.push_str("</part-list>\n");
    for (index, part) in parts.iter().enumerate() {
        let _ = writeln!(xml, "<part id=\"P{}\">", index + 1);
        xml.push_str(&part_measures(&part.notes, &beats));
        xml.push_str("</part>\n");
    }
    xml.push_str("</score-partwise>\n");
    xml
}

/// Fold a tracked pulse down to a quarter-note range for barring. The
/// tracker often locks onto the eighth-note pulse - right for alignment,
/// but notated as-is it would halve every measure and double every note
/// value.
///
/// The decision is regional: each pass halves the pulse wherever the
/// *smoothed* local tempo (a ±8-beat window, so single rubato beats don't
/// flip it) still exceeds 115 BPM, until every region sits in the
/// ~57-115 range a 4/4 quarter occupies. A sixteenth lock takes two
/// passes; the third confirms stability. A tempo-octave choice, never a
/// timing change.
fn fold_to_notation_pulse(beats: Vec<f64>) -> Vec<f64> {
    const FOLD_ABOVE_BPM: f64 = 115.0;
    let mut beats = beats;
    for _ in 0..3 {
        let smoothed_fast = |i: usize| {
            let lo = i.saturating_sub(8);
            let hi = (i + 8).min(beats.len() - 1);
            60.0 * (hi - lo) as f64 / (beats[hi] - beats[lo]) > FOLD_ABOVE_BPM
        };
        let mut out = vec![beats[0]];
        let mut merged = false;
        let mut i = 0;
        while i + 1 < beats.len() {
            if smoothed_fast(i) && i + 2 < beats.len() {
                out.push(beats[i + 2]);
                merged = true;
                i += 2;
            } else {
                out.push(beats[i + 1]);
                i += 1;
            }
        }
        beats = out;
        if !merged {
            break;
        }
    }
    beats
}

/// The fixed-tempo beat grid: anchored at zero, one beat per `60 / bpm`
/// seconds, covering every note. Always at least two beats.
fn uniform_beats(parts: &[ScorePart], bpm: f64) -> Vec<f64> {
    let period = 60.0 / bpm;
    let end = parts
        .iter()
        .flat_map(|part| &part.notes)
        .map(|note| note.offset_s)
        .fold(0.0, f64::max);
    let mut beats = vec![0.0];
    while beats.len() < 2 || *beats.last().expect("beats starts non-empty") < end {
        beats.push(beats.last().expect("beats starts non-empty") + period);
    }
    beats
}

/// A chord on the quantized grid; `duration` never crosses a barline.
struct GridChord {
    onset: usize,
    duration: usize,
    midis: Vec<u8>,
}

fn part_measures(notes: &[TranscribedNote], beats: &[f64]) -> String {
    let fifths = key_fifths(notes);
    let use_flats = fifths < 0;

    // The split is at middle C, except that a held note with nothing
    // sounding below it is the bass line wherever it sits - the left hand
    // of the C major prelude holds middle C for eight bars.
    let placed = place_notes(notes, beats);
    let (treble, bass): (Vec<&Placed>, Vec<&Placed>) = placed
        .iter()
        .partition(|p| p.note.midi >= TREBLE_SPLIT_MIDI && !is_lowest_hold(p, &placed));
    // Per staff: a running voice plus hold voices, so a note sounding
    // through other notes keeps its length instead of being clipped into
    // the figuration (the same split engraved scores make).
    let (treble_run, treble_holds) = split_hold_voices(&treble);
    let (bass_run, bass_holds) = split_hold_voices(&bass);
    // (chords, MusicXML voice number, staff)
    let streams = [
        (grid_chords(&treble_run, beats), 1, 1),
        (grid_chords(&treble_holds[0], beats), 3, 1),
        (grid_chords(&treble_holds[1], beats), 5, 1),
        (grid_chords(&bass_run, beats), 2, 2),
        (grid_chords(&bass_holds[0], beats), 4, 2),
        (grid_chords(&bass_holds[1], beats), 6, 2),
    ];

    let last_unit = streams
        .iter()
        .flat_map(|(chords, ..)| chords)
        .map(|chord| chord.onset + chord.duration)
        .max()
        .unwrap_or(0);
    let measures = (last_unit.div_ceil(MEASURE_UNITS)).max(1);

    let mut xml = String::new();
    let mut emitted_tempo = f64::NAN;
    for measure in 0..measures {
        let _ = writeln!(xml, "<measure number=\"{}\">", measure + 1);
        if measure == 0 {
            let _ = writeln!(
                xml,
                "<attributes>\
                 <divisions>{DIVISIONS}</divisions>\
                 <key><fifths>{fifths}</fifths></key>\
                 <time><beats>4</beats><beat-type>4</beat-type></time>\
                 <staves>2</staves>\
                 <clef number=\"1\"><sign>G</sign><line>2</line></clef>\
                 <clef number=\"2\"><sign>F</sign><line>4</line></clef>\
                 </attributes>",
            );
        }
        // A visible metronome mark opens the score; afterwards the tempo is
        // re-stated (invisibly, for playback) only when it moves.
        let tempo = measure_tempo(beats, measure);
        if measure == 0 {
            let _ = writeln!(
                xml,
                "<direction placement=\"above\"><direction-type>\
                 <metronome><beat-unit>quarter</beat-unit>\
                 <per-minute>{}</per-minute></metronome>\
                 </direction-type><sound tempo=\"{tempo:.1}\"/></direction>",
                tempo.round(),
            );
            emitted_tempo = tempo;
        } else if (tempo - emitted_tempo).abs() > emitted_tempo * 0.02 {
            let _ = writeln!(xml, "<sound tempo=\"{tempo:.1}\"/>");
            emitted_tempo = tempo;
        }
        // A voice appears only in measures where it has notes, except that
        // a staff with nothing at all shows its running voice's rests.
        let start = measure * MEASURE_UNITS;
        let end = start + MEASURE_UNITS;
        let sounds =
            |chords: &[GridChord]| chords.iter().any(|c| c.onset >= start && c.onset < end);
        let staff_sounds = |staff: usize| {
            streams
                .iter()
                .any(|(chords, _, s)| *s == staff && sounds(chords))
        };
        let mut first = true;
        for (chords, voice, staff) in &streams {
            if !sounds(chords) && (*voice > 2 || staff_sounds(*staff)) {
                continue;
            }
            if !first {
                let _ = writeln!(xml, "<backup><duration>{MEASURE_UNITS}</duration></backup>");
            }
            first = false;
            xml.push_str(&voice_xml(chords, measure, *voice, *staff, use_flats));
        }
        xml.push_str("</measure>\n");
    }
    xml
}

/// A note with its quantized span and hold status.
struct Placed<'a> {
    note: &'a TranscribedNote,
    onset: usize,
    end: usize,
    hold: bool,
}

/// Quantize every note (at least one unit long) and mark the holds. A note
/// is a *hold* when it is long (at least a beat and a half) and at least
/// two other notes strike while it sounds - the held bass of an arpeggiated
/// figure, a fugue subject's long tones. Ring-out alone does not qualify:
/// transcribed offsets are release times, so short notes routinely bleed
/// over their neighbors.
fn place_notes<'a>(notes: &'a [TranscribedNote], beats: &[f64]) -> Vec<Placed<'a>> {
    const HOLD_MIN_UNITS: usize = 6;
    let spans: Vec<(usize, usize)> = notes
        .iter()
        .map(|note| {
            let onset = grid_units(note.onset_s, beats);
            (onset, grid_units(note.offset_s, beats).max(onset + 1))
        })
        .collect();
    notes
        .iter()
        .zip(&spans)
        .map(|(note, &(onset, end))| {
            let strikes = spans
                .iter()
                .filter(|&&(other_onset, _)| other_onset > onset && other_onset < end)
                .count();
            Placed {
                note,
                onset,
                end,
                hold: end - onset >= HOLD_MIN_UNITS && strikes >= 2,
            }
        })
        .collect()
}

/// A hold with no lower note sounding during it: the bass line.
fn is_lowest_hold(p: &Placed, all: &[Placed]) -> bool {
    p.hold
        && !all.iter().any(|other| {
            other.note.midi < p.note.midi && other.onset < p.end && other.end > p.onset
        })
}

/// Split one staff's notes into the running voice and two hold voices. A
/// hold that overlaps the previous one takes the second hold voice, so
/// neither is clipped to the other's onset.
fn split_hold_voices<'a>(
    notes: &[&Placed<'a>],
) -> (Vec<&'a TranscribedNote>, [Vec<&'a TranscribedNote>; 2]) {
    let mut running = Vec::new();
    let mut holds = [Vec::new(), Vec::new()];
    let mut first_hold_end = 0;
    for p in notes {
        if !p.hold {
            running.push(p.note);
        } else if p.onset >= first_hold_end {
            holds[0].push(p.note);
            first_hold_end = p.end;
        } else {
            holds[1].push(p.note);
        }
    }
    (running, holds)
}

/// A time to grid units: each tracked beat is [`DIVISIONS`] units, with the
/// position inside (or beyond) the grid interpolated from the surrounding
/// beat pair. Times before the first beat clamp to zero.
fn grid_units(t: f64, beats: &[f64]) -> usize {
    let i = beats
        .partition_point(|&beat| beat <= t)
        .saturating_sub(1)
        .min(beats.len() - 2);
    let fraction = i as f64 + (t - beats[i]) / (beats[i + 1] - beats[i]);
    (fraction * DIVISIONS as f64).round().max(0.0) as usize
}

/// Playback tempo (quarter BPM) of one 4/4 measure: four beats of the
/// grid, the last pair's period extending past its end.
fn measure_tempo(beats: &[f64], measure: usize) -> f64 {
    let beat_time = |i: usize| {
        let last = beats.len() - 1;
        if i <= last {
            beats[i]
        } else {
            beats[last] + (beats[last] - beats[last - 1]) * (i - last) as f64
        }
    };
    let beats_per_measure = MEASURE_UNITS / DIVISIONS;
    let start = measure * beats_per_measure;
    240.0 / (beat_time(start + beats_per_measure) - beat_time(start))
}

/// Quantize one staff's notes into non-overlapping chords on the grid.
fn grid_chords(notes: &[&TranscribedNote], beats: &[f64]) -> Vec<GridChord> {
    // Group by quantized onset; a chord ends where its longest member does.
    let mut by_onset: BTreeMap<usize, (usize, Vec<u8>)> = BTreeMap::new();
    for note in notes {
        let onset = grid_units(note.onset_s, beats);
        let end = grid_units(note.offset_s, beats).max(onset + 1);
        let entry = by_onset.entry(onset).or_insert((end, Vec::new()));
        entry.0 = entry.0.max(end);
        if !entry.1.contains(&note.midi) {
            entry.1.push(note.midi);
        }
    }
    let onsets: Vec<usize> = by_onset.keys().copied().collect();
    by_onset
        .iter()
        .enumerate()
        .map(|(i, (&onset, (end, midis)))| {
            // Clip at the next chord and at the barline (no ties).
            let mut clipped = *end;
            if let Some(&next) = onsets.get(i + 1) {
                clipped = clipped.min(next);
            }
            let barline = (onset / MEASURE_UNITS + 1) * MEASURE_UNITS;
            clipped = clipped.min(barline);
            let mut midis = midis.clone();
            midis.sort_unstable();
            GridChord {
                onset,
                duration: clipped.saturating_sub(onset).max(1),
                midis,
            }
        })
        .collect()
}

/// One measure of one staff as a single MusicXML voice: chords in time
/// order, gaps filled with rests, padded to exactly [`MEASURE_UNITS`],
/// eighths and 16ths beamed per quarter-note group.
fn voice_xml(
    chords: &[GridChord],
    measure: usize,
    voice: usize,
    staff: usize,
    use_flats: bool,
) -> String {
    let start = measure * MEASURE_UNITS;
    let end = start + MEASURE_UNITS;
    // Longest expressible length that fits the chord and the measure.
    let entries: Vec<(&GridChord, usize)> = chords
        .iter()
        .filter(|chord| chord.onset >= start && chord.onset < end)
        .map(|chord| (chord, chord.duration.min(end - chord.onset)))
        .collect();
    let beams = beam_tags(&entries, start);

    let mut xml = String::new();
    let mut cursor = start;
    for ((chord, fit), beam) in entries.iter().zip(&beams) {
        write_rests(&mut xml, chord.onset - cursor, voice, staff);
        cursor = chord.onset;
        let (duration, kind, dotted) = note_type(*fit);
        for (position, &midi) in chord.midis.iter().enumerate() {
            let (step, alter, octave) = spell(midi, use_flats);
            let chord_tag = if position > 0 { "<chord/>" } else { "" };
            let alter_tag = if alter == 0 {
                String::new()
            } else {
                format!("<alter>{alter}</alter>")
            };
            let dot_tag = if dotted { "<dot/>" } else { "" };
            // Beams attach to the chord's first note only.
            let beam_tag = if position == 0 { beam.as_str() } else { "" };
            let _ = writeln!(
                xml,
                "<note>{chord_tag}<pitch><step>{step}</step>{alter_tag}\
                 <octave>{octave}</octave></pitch>\
                 <duration>{duration}</duration><voice>{voice}</voice>\
                 <type>{kind}</type>{dot_tag}<staff>{staff}</staff>{beam_tag}</note>",
            );
        }
        cursor += duration;
    }
    write_rests(&mut xml, end - cursor, voice, staff);
    xml
}

/// Beam markup per chord: runs of equal-length eighths or 16ths that
/// follow each other gaplessly inside one quarter-note group share a beam
/// (two beam levels for 16ths), as engraved music groups them.
fn beam_tags(entries: &[(&GridChord, usize)], measure_start: usize) -> Vec<String> {
    let written: Vec<usize> = entries.iter().map(|&(_, fit)| note_type(fit).0).collect();
    let joined = |i: usize| {
        i > 0
            && written[i] == written[i - 1]
            && written[i] <= 2
            && entries[i - 1].0.onset + written[i - 1] == entries[i].0.onset
            && (entries[i - 1].0.onset - measure_start) / DIVISIONS
                == (entries[i].0.onset - measure_start) / DIVISIONS
    };
    (0..entries.len())
        .map(|i| {
            let to_prev = joined(i);
            let to_next = i + 1 < entries.len() && joined(i + 1);
            let state = match (to_prev, to_next) {
                (false, true) => "begin",
                (true, true) => "continue",
                (true, false) => "end",
                (false, false) => return String::new(),
            };
            let levels = if written[i] == 1 { 2 } else { 1 };
            (1..=levels)
                .map(|number| format!("<beam number=\"{number}\">{state}</beam>"))
                .collect()
        })
        .collect()
}

fn write_rests(xml: &mut String, mut gap: usize, voice: usize, staff: usize) {
    while gap > 0 {
        let (duration, kind, dotted) = note_type(gap);
        let dot_tag = if dotted { "<dot/>" } else { "" };
        let _ = writeln!(
            xml,
            "<note><rest/><duration>{duration}</duration><voice>{voice}</voice>\
             <type>{kind}</type>{dot_tag}<staff>{staff}</staff></note>",
        );
        gap -= duration;
    }
}

/// Longest expressible note length that fits in `units`.
fn note_type(units: usize) -> (usize, &'static str, bool) {
    // NOTE_TYPES ends at 1 unit, so a match always exists for units >= 1.
    NOTE_TYPES
        .iter()
        .find(|&&(length, _, _)| length <= units)
        .map(|&(length, kind, dotted)| (length, kind, dotted))
        .unwrap_or((1, "16th", false))
}

const SHARP_STEPS: [(&str, i8); 12] = [
    ("C", 0),
    ("C", 1),
    ("D", 0),
    ("D", 1),
    ("E", 0),
    ("F", 0),
    ("F", 1),
    ("G", 0),
    ("G", 1),
    ("A", 0),
    ("A", 1),
    ("B", 0),
];
const FLAT_STEPS: [(&str, i8); 12] = [
    ("C", 0),
    ("D", -1),
    ("D", 0),
    ("E", -1),
    ("E", 0),
    ("F", 0),
    ("G", -1),
    ("G", 0),
    ("A", -1),
    ("A", 0),
    ("B", -1),
    ("B", 0),
];

fn spell(midi: u8, use_flats: bool) -> (&'static str, i8, i8) {
    let class = usize::from(midi % 12);
    let (step, alter) = if use_flats {
        FLAT_STEPS[class]
    } else {
        SHARP_STEPS[class]
    };
    // MusicXML octave counts C4 as octave 4; an E♭ spelling never crosses
    // an octave boundary, so the plain MIDI octave is correct either way.
    (step, alter, (i16::from(midi) / 12 - 1) as i8)
}

/// Krumhansl-Schmuckler key profiles.
const MAJOR_PROFILE: [f64; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const MINOR_PROFILE: [f64; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];
/// Key-signature fifths for each major tonic pitch class (C, D♭, D, ...).
const MAJOR_FIFTHS: [i32; 12] = [0, -5, 2, -3, 4, -1, 6, 1, -4, 3, -2, 5];

/// Detect the key signature: correlate the duration-weighted pitch-class
/// histogram against all 24 major/minor profiles.
fn key_fifths(notes: &[TranscribedNote]) -> i32 {
    if notes.is_empty() {
        return 0;
    }
    let mut histogram = [0.0_f64; 12];
    for note in notes {
        histogram[usize::from(note.midi % 12)] += (note.offset_s - note.onset_s).max(0.05);
    }
    let mut best = (0, f64::MIN);
    for tonic in 0..12 {
        for (profile, relative_major_shift) in [(&MAJOR_PROFILE, 0), (&MINOR_PROFILE, 3)] {
            let score: f64 = (0..12)
                .map(|degree| profile[degree] * histogram[(tonic + degree) % 12])
                .sum();
            let fifths = MAJOR_FIFTHS[(tonic + relative_major_shift) % 12];
            if score > best.1 {
                best = (fifths, score);
            }
        }
    }
    best.0
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
#[path = "musicxml_test.rs"]
mod musicxml_test;
