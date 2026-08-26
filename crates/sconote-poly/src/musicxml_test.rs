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
        Some(BPM),
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
    // Truncated to one quarter (4 units) at the end of measure 1 - and with
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
        Some(BPM),
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
fn fixed_bpm_states_its_tempo_once() {
    // Two measures of notes: the constant tempo appears exactly once.
    let notes = (0..8)
        .map(|i| note(72, f64::from(i) * 0.5, f64::from(i) * 0.5 + 0.4))
        .collect();
    let xml = single_part(notes);
    assert_eq!(xml.matches("<sound tempo=").count(), 1);
    assert!(xml.contains("<sound tempo=\"120.0\"/>"));
}

#[test]
fn tracked_tempo_restates_itself_through_a_ritardando() {
    // Four measures of quarter beats at 100 BPM, then four measures slowing
    // ~4% per beat - tracked notation must re-state the tempo, lower.
    let mut onsets = Vec::new();
    let mut t = 0.0;
    let mut spacing = 0.6;
    for i in 0..32 {
        onsets.push(t);
        if i >= 16 {
            spacing *= 1.04;
        }
        t += spacing;
    }
    let notes: Vec<TranscribedNote> = onsets
        .iter()
        .map(|&onset| note(72, onset, onset + 0.2))
        .collect();
    let xml = parts_to_musicxml(
        &[ScorePart {
            name: "Piano".into(),
            notes: notes.clone(),
        }],
        None,
    );
    let tempos: Vec<f64> = xml
        .match_indices("<sound tempo=\"")
        .filter_map(|(start, tag)| {
            let rest = &xml[start + tag.len()..];
            rest[..rest.find('"')?].parse().ok()
        })
        .collect();
    assert!(tempos.len() >= 2, "expected tempo changes, got {tempos:?}");
    assert!(
        tempos.last().unwrap() < &(tempos[0] * 0.85),
        "final tempo should be well below the opening: {tempos:?}"
    );
    // With the beat tracked, every onset still lands on its own quarter:
    // 32 beats of quarters = exactly 8 full measures.
    assert!(xml.contains("<measure number=\"8\">"));
    assert!(!xml.contains("<measure number=\"9\">"));
}

#[test]
fn subdivided_fast_pulse_folds_to_quarter_barring() {
    // Notes every 0.2 s: the tracker locks a 150 BPM pulse with two events
    // per beat - an eighth-pulse lock, so notation folds it to 75.
    let notes: Vec<TranscribedNote> = (0..32)
        .map(|i| note(72, f64::from(i) * 0.2, f64::from(i) * 0.2 + 0.1))
        .collect();
    let xml = parts_to_musicxml(
        &[ScorePart {
            name: "Piano".into(),
            notes,
        }],
        None,
    );
    assert!(
        xml.contains("<sound tempo=\"75"),
        "expected folded tempo: {xml}"
    );
}

#[test]
fn a_moderate_pulse_keeps_its_barring() {
    // ~109 BPM sits inside the quarter-note band - no fold.
    let notes: Vec<TranscribedNote> = (0..16)
        .map(|i| note(72, f64::from(i) * 0.55, f64::from(i) * 0.55 + 0.2))
        .collect();
    let xml = parts_to_musicxml(
        &[ScorePart {
            name: "Piano".into(),
            notes,
        }],
        None,
    );
    assert!(
        xml.contains("<sound tempo=\"109"),
        "expected unfolded tempo"
    );
}

#[test]
fn a_note_held_under_a_running_figure_becomes_its_own_voice() {
    // Held E4 with a 16th run above it (each run note ringing past its
    // neighbor, as transcriptions do): the hold keeps its full length in
    // a voice of its own, the run stays 16ths in voice 1.
    let xml = single_part(run_over(vec![note(64, 0.0, 2.0)]));
    assert!(
        xml.contains(
            "<step>E</step><octave>4</octave></pitch><duration>16</duration><voice>4</voice><type>whole</type>"
        ),
        "expected the held E4 as a whole note in a hold voice"
    );
    assert!(xml.contains("<voice>1</voice><type>16th</type>"));
}

/// `held` plus a G4-C5-E5 16th figure over it filling the first bar.
fn run_over(mut held: Vec<TranscribedNote>) -> Vec<TranscribedNote> {
    for i in 0..15 {
        let onset = 0.125 + f64::from(i) * 0.125;
        held.push(note(67 + (i % 3) as u8 * 5, onset, onset + 0.5));
    }
    held
}

#[test]
fn the_lowest_held_note_is_the_bass_line_even_above_middle_c() {
    // Middle C held under the figure, as the left hand of the C major
    // prelude: it belongs on the bass staff although it is not below 60.
    let xml = single_part(run_over(vec![note(60, 0.0, 2.0)]));
    assert!(
        xml.contains(
            "<step>C</step><octave>4</octave></pitch><duration>16</duration><voice>4</voice><type>whole</type><stem>down</stem><staff>2</staff>"
        ),
        "expected the held C4 on the bass staff"
    );
    assert!(
        !xml.contains("<staff>2</staff><beam"),
        "the figure stays treble"
    );
}

#[test]
fn a_staff_carried_by_a_hold_voice_shows_no_running_voice_rests() {
    // The bass staff holds only C4: its running voice (2) would add a
    // whole rest on top of the note, so it is left out of the measure.
    let xml = single_part(run_over(vec![note(60, 0.0, 2.0)]));
    assert!(!xml.contains("<voice>2</voice>"));
}

#[test]
fn a_hold_above_a_lower_hold_stays_on_the_treble_staff() {
    // C4 and E4 both held: C4 is the bass line, E4 the treble's own hold
    // voice - and neither is clipped to the other's onset.
    let xml = single_part(run_over(vec![note(60, 0.0, 2.0), note(64, 0.125, 2.0)]));
    assert!(xml.contains(
        "<step>C</step><octave>4</octave></pitch><duration>16</duration><voice>4</voice><type>whole</type><stem>down</stem><staff>2</staff>"
    ));
    assert!(xml.contains(
        "<step>E</step><octave>4</octave></pitch><duration>3</duration><tie type=\"start\"/><voice>3</voice><type>eighth</type><dot/><stem>down</stem><staff>1</staff>"
    ));
}

#[test]
fn overlapping_holds_on_one_staff_take_separate_voices() {
    // Two overlapping holds under the figure, both below middle C: the
    // second would clip the first in a shared voice, so it gets voice 6.
    let xml = single_part(run_over(vec![note(43, 0.0, 2.0), note(59, 0.125, 2.0)]));
    assert!(xml.contains(
        "<step>G</step><octave>2</octave></pitch><duration>16</duration><voice>4</voice><type>whole</type><stem>down</stem><staff>2</staff>"
    ));
    assert!(xml.contains(
        "<step>B</step><octave>3</octave></pitch><duration>3</duration><tie type=\"start\"/><voice>6</voice><type>eighth</type><dot/><stem>down</stem><staff>2</staff>"
    ));
}

#[test]
fn short_ringing_notes_stay_in_the_running_voice() {
    // A plain 16th run where every note rings over the next two - none of
    // it is a hold, so no voice-3 stream appears.
    let notes: Vec<TranscribedNote> = (0..12)
        .map(|i| note(72, f64::from(i) * 0.125, f64::from(i) * 0.125 + 0.4))
        .collect();
    let xml = single_part(notes);
    assert!(!xml.contains("<voice>3</voice>"));
}

#[test]
fn sixteenth_runs_are_beamed_per_quarter_group() {
    // Six 16ths from unit 1: engraving beams units 1-3 and 4-6 separately,
    // breaking at the quarter-note boundary.
    let notes: Vec<TranscribedNote> = (0..6)
        .map(|i| {
            let onset = 0.125 + f64::from(i) * 0.125;
            note(72, onset, onset + 0.125)
        })
        .collect();
    let xml = single_part(notes);
    assert_eq!(xml.matches("<beam number=\"1\">begin").count(), 2, "{xml}");
    assert_eq!(xml.matches("<beam number=\"1\">continue").count(), 2);
    assert_eq!(xml.matches("<beam number=\"1\">end").count(), 2);
    // 16ths carry a second beam level.
    assert_eq!(xml.matches("<beam number=\"2\">begin").count(), 2);
}

#[test]
fn an_isolated_sixteenth_is_not_beamed() {
    let xml = single_part(vec![note(72, 0.0, 0.125)]);
    assert!(!xml.contains("<beam"));
}

#[test]
fn the_score_opens_with_a_metronome_mark() {
    let xml = single_part(vec![note(60, 0.0, 0.5)]);
    assert!(xml.contains("<metronome><beat-unit>quarter</beat-unit><per-minute>120</per-minute>"));
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

#[test]
fn an_off_beat_hold_is_written_as_tied_pieces_split_at_the_beat() {
    // E4 from the second 16th to the third beat (7 units): a dotted eighth
    // up to the beat, tied to a quarter - as engraved, not a rounded value.
    let xml = single_part(run_over(vec![note(60, 0.0, 2.0), note(64, 0.125, 1.0)]));
    assert!(xml.contains(
        "<step>E</step><octave>4</octave></pitch><duration>3</duration><tie type=\"start\"/><voice>3</voice><type>eighth</type><dot/><stem>down</stem><staff>1</staff><notations><tied type=\"start\" orientation=\"under\" placement=\"below\"/></notations></note>"
    ));
    assert!(xml.contains(
        "<step>E</step><octave>4</octave></pitch><duration>4</duration><tie type=\"stop\"/><voice>3</voice><type>quarter</type><stem>down</stem><staff>1</staff><notations><tied type=\"stop\" orientation=\"under\" placement=\"below\"/></notations></note>"
    ));
}

#[test]
fn an_expressible_length_on_the_beat_is_not_tied() {
    let xml = single_part(vec![note(72, 0.0, 0.75)]);
    assert!(xml.contains("<duration>6</duration><voice>1</voice><type>quarter</type><dot/>"));
    assert!(!xml.contains("<tie "));
}

#[test]
fn a_length_with_no_single_value_is_tied_from_the_beat() {
    // 7 units from the downbeat: dotted quarter tied to a 16th.
    let xml = single_part(vec![note(72, 0.0, 0.875)]);
    assert!(xml.contains(
        "<duration>6</duration><tie type=\"start\"/><voice>1</voice><type>quarter</type><dot/>"
    ));
    assert!(
        xml.contains("<duration>1</duration><tie type=\"stop\"/><voice>1</voice><type>16th</type>")
    );
}

#[test]
fn a_figure_dipping_below_middle_c_over_a_bass_hold_stays_treble() {
    // Bar 16 of the C major prelude: the right hand's A3-C4-F4 figure over
    // a held F3. A3 is below middle C but sits above the bass line, so it
    // is engraved on the treble staff with the rest of the figure.
    let mut notes = vec![note(53, 0.0, 2.0)];
    for i in 0..12 {
        let onset = 0.25 + i as f64 * 0.125;
        notes.push(note([57, 60, 65][i % 3], onset, onset + 0.2));
    }
    let xml = single_part(notes);
    assert!(xml.contains(
        "<step>A</step><octave>3</octave></pitch><duration>1</duration><voice>1</voice><type>16th</type><staff>1</staff>"
    ));
    assert!(!xml.contains("<octave>3</octave></pitch><duration>1</duration><voice>2</voice>"));
}

#[test]
fn a_low_run_with_nothing_held_below_it_is_bass() {
    // A left-hand run alone: no bass line under it, so pitch decides.
    let notes: Vec<TranscribedNote> = (0..8u8)
        .map(|i| note(48 + i, f64::from(i) * 0.125, f64::from(i) * 0.125 + 0.1))
        .collect();
    let xml = single_part(notes);
    assert!(xml.contains("<voice>2</voice><type>16th</type><staff>2</staff>"));
    assert!(
        !xml.contains("<voice>1</voice><type>16th</type>"),
        "expected no treble notes"
    );
}

#[test]
fn voices_sharing_a_staff_get_fixed_stems_and_tie_sides() {
    // E4 held under the figure on the treble staff, which the figure
    // shares: figure stems up, hold stems down with its tie under. Holds
    // stem down even alone on their staff (the C4 bass line).
    let xml = single_part(run_over(vec![note(60, 0.0, 2.0), note(64, 0.125, 1.0)]));
    assert!(xml.contains("<voice>1</voice><type>16th</type><stem>up</stem><staff>1</staff>"));
    assert!(xml.contains("<voice>3</voice><type>eighth</type><dot/><stem>down</stem><staff>1</staff><notations><tied type=\"start\" orientation=\"under\" placement=\"below\"/>"));
    assert!(xml.contains("<voice>4</voice><type>whole</type><stem>down</stem><staff>2</staff>"));
}
