use super::*;

const BPM: f64 = 120.0; // 0.125 s per 16th unit

fn note(midi: u8, onset_s: f64, offset_s: f64) -> TranscribedNote {
    TranscribedNote {
        midi,
        onset_s,
        offset_s,
    }
}

fn single_part(notes: Vec<TranscribedNote>) -> String {
    parts_to_musicxml(
        &[ScorePart {
            name: "Piano".into(),
            notes,
        }],
        BPM,
    )
}

#[test]
fn produces_a_grand_staff_score_with_part_name() {
    let xml = single_part(vec![note(60, 0.0, 0.5)]);
    assert!(xml.contains("<part-name>Piano</part-name>"));
    assert!(xml.contains("<staves>2</staves>"));
    assert!(xml.contains("<clef number=\"1\"><sign>G</sign>"));
    assert!(xml.contains("<clef number=\"2\"><sign>F</sign>"));
    assert!(xml.contains("<time><beats>4</beats><beat-type>4</beat-type></time>"));
}

#[test]
fn quantizes_onset_and_duration_to_the_grid() {
    // Onset 0.26 s → unit 2 (an eighth in); duration 0.24 s → 2 units.
    let xml = single_part(vec![note(72, 0.26, 0.50)]);
    // Leading eighth rest, then an eighth note.
    assert!(xml.contains("<rest/><duration>2</duration><voice>1</voice><type>eighth</type>"));
    assert!(
        xml.contains("<step>C</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice><type>eighth</type>")
    );
}

#[test]
fn simultaneous_notes_share_a_chord() {
    let xml = single_part(vec![note(60, 0.0, 0.5), note(64, 0.01, 0.5)]);
    assert_eq!(xml.matches("<chord/>").count(), 1);
}

#[test]
fn low_notes_go_to_the_bass_staff() {
    let xml = single_part(vec![note(48, 0.0, 0.5), note(72, 0.0, 0.5)]);
    assert!(xml.contains("<octave>3</octave></pitch><duration>4</duration><voice>2</voice><type>quarter</type><staff>2</staff>"));
    assert!(xml.contains("<octave>5</octave></pitch><duration>4</duration><voice>1</voice><type>quarter</type><staff>1</staff>"));
}

#[test]
fn flat_key_material_is_spelled_with_flats() {
    // A B♭ major scale, weighted naturally toward the tonic.
    let scale = [70, 72, 74, 75, 77, 79, 81, 82, 70, 70];
    let notes = scale
        .iter()
        .enumerate()
        .map(|(i, &midi)| note(midi, i as f64 * 0.5, i as f64 * 0.5 + 0.4))
        .collect();
    let xml = single_part(notes);
    assert!(xml.contains("<fifths>-2</fifths>"), "expected B-flat major");
    assert!(xml.contains("<step>B</step><alter>-1</alter>"));
    assert!(!xml.contains("<step>A</step><alter>1</alter>"));
}

#[test]
fn sharp_key_material_keeps_sharp_spelling() {
    // G major with a prominent F#.
    let scale = [67, 69, 71, 72, 74, 76, 78, 79, 67, 67];
    let notes = scale
        .iter()
        .enumerate()
        .map(|(i, &midi)| note(midi, i as f64 * 0.5, i as f64 * 0.5 + 0.4))
        .collect();
    let xml = single_part(notes);
    assert!(xml.contains("<fifths>1</fifths>"), "expected G major");
    assert!(xml.contains("<step>F</step><alter>1</alter>"));
}

#[test]
fn note_is_truncated_at_the_barline() {
    // A whole-measure-crossing note: starts at beat 4, lasts 2 beats.
    let xml = single_part(vec![note(72, 1.5, 2.5)]);
    // Truncated to one quarter (4 units) at the end of measure 1 — and with
    // the tail dropped, nothing extends into a second measure.
    assert!(xml.contains(
        "<octave>5</octave></pitch><duration>4</duration><voice>1</voice><type>quarter</type>"
    ));
    assert!(!xml.contains("<measure number=\"2\">"));
}

#[test]
fn each_part_appears_separately() {
    let xml = parts_to_musicxml(
        &[
            ScorePart {
                name: "Guitar & Friends".into(),
                notes: vec![note(64, 0.0, 0.5)],
            },
            ScorePart {
                name: "Bass".into(),
                notes: vec![note(40, 0.0, 0.5)],
            },
        ],
        BPM,
    );
    assert!(xml.contains("<part-name>Guitar &amp; Friends</part-name>"));
    assert!(xml.contains("<part-name>Bass</part-name>"));
    assert!(xml.contains("<part id=\"P1\">"));
    assert!(xml.contains("<part id=\"P2\">"));
}

#[test]
fn empty_input_is_a_valid_single_measure_of_rests() {
    let xml = single_part(Vec::new());
    assert!(xml.contains("<measure number=\"1\">"));
    assert!(xml.contains("<rest/><duration>16</duration>"));
    assert!(!xml.contains("<measure number=\"2\">"));
    assert!(xml.contains("<fifths>0</fifths>"));
}

#[test]
fn measures_fill_exactly_with_rests() {
    // One 16th note at the very start: measure must pad 15 more units as
    // dotted-half + quarter + eighth + 16th... whichever standard shapes.
    let xml = single_part(vec![note(72, 0.0, 0.125)]);
    let voice1: Vec<&str> = xml
        .lines()
        .filter(|line| line.contains("<voice>1</voice>"))
        .collect();
    let total: usize = voice1
        .iter()
        .filter_map(|line| {
            let start = line.find("<duration>")? + "<duration>".len();
            let end = line[start..].find("</duration>")? + start;
            line[start..end].parse::<usize>().ok()
        })
        .sum();
    assert_eq!(total, 16, "voice 1 must fill the measure: {voice1:?}");
}
