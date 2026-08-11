//! Frequency ⇄ musical note conversion (12-tone equal temperament, A4 = 440 Hz).

pub const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// Fractional MIDI number for a frequency: 69 + 12·log2(f/440).
pub fn midi_from_frequency(frequency_hz: f32) -> f32 {
    69.0 + 12.0 * (frequency_hz / 440.0).log2()
}

/// A detected frequency resolved to the nearest equal-temperament note.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    /// Nearest MIDI note number (A4 = 69).
    pub midi: i32,
    /// Deviation from that note in cents, in (-50, 50].
    pub cents_offset: f32,
}

impl Note {
    pub fn from_frequency(frequency_hz: f32) -> Note {
        let midi_fractional = midi_from_frequency(frequency_hz);
        let midi = midi_fractional.round() as i32;
        Note {
            midi,
            cents_offset: (midi_fractional - midi as f32) * 100.0,
        }
    }

    /// Scientific pitch name, e.g. "A4", "C#5".
    pub fn name(&self) -> String {
        let pitch_class = NOTE_NAMES[self.midi.rem_euclid(12) as usize];
        format!("{}{}", pitch_class, self.midi.div_euclid(12) - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a4_is_midi_69() {
        let note = Note::from_frequency(440.0);
        assert_eq!(note.midi, 69);
        assert_eq!(note.name(), "A4");
        assert!(note.cents_offset.abs() < 0.01);
    }

    #[test]
    fn c4_is_midi_60() {
        let note = Note::from_frequency(261.626);
        assert_eq!(note.midi, 60);
        assert_eq!(note.name(), "C4");
    }

    #[test]
    fn sharp_names_and_octaves() {
        assert_eq!(Note::from_frequency(466.164).name(), "A#4"); // Bb4
        assert_eq!(Note::from_frequency(92.499).name(), "F#2");
        assert_eq!(Note::from_frequency(4186.0).name(), "C8");
    }

    #[test]
    fn cents_offset_of_sharp_a4() {
        // 445 Hz is ≈ +19.56 cents above A4
        let note = Note::from_frequency(445.0);
        assert_eq!(note.midi, 69);
        assert!((note.cents_offset - 19.56).abs() < 0.05);
    }

    #[test]
    fn rounds_to_nearest_note_at_boundary() {
        // Quarter tone between A4 and A#4 (~452.9 Hz) → ±50 cents edge
        let just_below = Note::from_frequency(452.0);
        assert_eq!(just_below.midi, 69);
        let just_above = Note::from_frequency(454.0);
        assert_eq!(just_above.midi, 70);
    }
}
