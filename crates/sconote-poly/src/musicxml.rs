//! Transcribed notes → MusicXML, the interchange format every engraving
//! tool (OpenSheetMusicDisplay, MuseScore, …) renders as sheet music.
//!
//! The conversion makes the notation decisions a score needs beyond raw
//! notes: onsets/durations quantize to a 16th-note grid at the given tempo,
//! a Krumhansl-style key detector picks the key signature (so B♭ material
//! is spelled with flats, not A♯), and each part is a piano grand staff
//! split at middle C. Time signature is fixed 4/4; notes are truncated at
//! barlines rather than tied — a readable approximation, not full
//! engraving-grade rhythm transcription.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::score::TranscribedNote;

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

/// Render one or more instrument parts as a MusicXML score at `bpm`.
pub fn parts_to_musicxml(parts: &[ScorePart], bpm: f64) -> String {
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
        xml.push_str(&part_measures(&part.notes, bpm));
        xml.push_str("</part>\n");
    }
    xml.push_str("</score-partwise>\n");
    xml
}

/// A chord on the quantized grid; `duration` never crosses a barline.
struct GridChord {
    onset: usize,
    duration: usize,
    midis: Vec<u8>,
}

fn part_measures(notes: &[TranscribedNote], bpm: f64) -> String {
    let fifths = key_fifths(notes);
    let use_flats = fifths < 0;
    let units_per_second = bpm / 60.0 * DIVISIONS as f64;

    let (treble, bass): (Vec<&TranscribedNote>, Vec<&TranscribedNote>) = notes
        .iter()
        .partition(|note| note.midi >= TREBLE_SPLIT_MIDI);
    let staves = [
        grid_chords(&treble, units_per_second),
        grid_chords(&bass, units_per_second),
    ];

    let last_unit = staves
        .iter()
        .flatten()
        .map(|chord| chord.onset + chord.duration)
        .max()
        .unwrap_or(0);
    let measures = (last_unit.div_ceil(MEASURE_UNITS)).max(1);

    let mut xml = String::new();
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
        xml.push_str(&voice_xml(&staves[0], measure, 1, 1, use_flats));
        let _ = writeln!(xml, "<backup><duration>{MEASURE_UNITS}</duration></backup>");
        xml.push_str(&voice_xml(&staves[1], measure, 2, 2, use_flats));
        xml.push_str("</measure>\n");
    }
    xml
}

/// Quantize one staff's notes into non-overlapping chords on the grid.
fn grid_chords(notes: &[&TranscribedNote], units_per_second: f64) -> Vec<GridChord> {
    // Group by quantized onset; a chord ends where its longest member does.
    let mut by_onset: BTreeMap<usize, (usize, Vec<u8>)> = BTreeMap::new();
    for note in notes {
        let onset = (note.onset_s * units_per_second).round() as usize;
        let end = ((note.offset_s * units_per_second).round() as usize).max(onset + 1);
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
/// order, gaps filled with rests, padded to exactly [`MEASURE_UNITS`].
fn voice_xml(
    chords: &[GridChord],
    measure: usize,
    voice: usize,
    staff: usize,
    use_flats: bool,
) -> String {
    let start = measure * MEASURE_UNITS;
    let end = start + MEASURE_UNITS;
    let mut xml = String::new();
    let mut cursor = start;
    for chord in chords {
        if chord.onset < start || chord.onset >= end {
            continue;
        }
        write_rests(&mut xml, chord.onset - cursor, voice, staff);
        cursor = chord.onset;
        // Longest expressible length that fits the chord and the measure.
        let fit = chord.duration.min(end - cursor);
        let (duration, kind, dotted) = note_type(fit);
        for (position, &midi) in chord.midis.iter().enumerate() {
            let (step, alter, octave) = spell(midi, use_flats);
            let chord_tag = if position > 0 { "<chord/>" } else { "" };
            let alter_tag = if alter == 0 {
                String::new()
            } else {
                format!("<alter>{alter}</alter>")
            };
            let dot_tag = if dotted { "<dot/>" } else { "" };
            let _ = writeln!(
                xml,
                "<note>{chord_tag}<pitch><step>{step}</step>{alter_tag}\
                 <octave>{octave}</octave></pitch>\
                 <duration>{duration}</duration><voice>{voice}</voice>\
                 <type>{kind}</type>{dot_tag}<staff>{staff}</staff></note>",
            );
        }
        cursor += duration;
    }
    write_rests(&mut xml, end - cursor, voice, staff);
    xml
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
/// Key-signature fifths for each major tonic pitch class (C, D♭, D, …).
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
