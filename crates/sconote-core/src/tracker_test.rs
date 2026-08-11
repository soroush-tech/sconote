use super::*;
use crate::test_signals::{SAMPLE_RATE, WINDOW, noise, sine};

fn tracker() -> NoteTracker {
    NoteTracker::new(SAMPLE_RATE, WINDOW)
}

/// Feeds the signal one full analysis window per call.
fn feed_windows(tracker: &mut NoteTracker, signal: &[f32]) -> Vec<TrackerUpdate> {
    signal
        .chunks(WINDOW)
        .map(|chunk| tracker.process(chunk))
        .collect()
}

fn started_names(updates: &[TrackerUpdate]) -> Vec<&str> {
    updates
        .iter()
        .filter_map(|update| update.note_started.as_ref())
        .map(|event| event.note_name.as_str())
        .collect()
}

mod process {
    use super::*;

    #[test]
    fn confirms_a_held_note_exactly_once() {
        let mut tracker = tracker();
        let updates = feed_windows(&mut tracker, &sine(440.0, WINDOW * 6));
        assert_eq!(started_names(&updates), ["A4"]);
    }

    #[test]
    fn does_not_confirm_before_hold_windows_elapse() {
        let mut tracker = tracker();
        let too_short = WINDOW * (HOLD_WINDOWS as usize - 1);
        let updates = feed_windows(&mut tracker, &sine(440.0, too_short));
        assert_eq!(started_names(&updates), Vec::<&str>::new());
    }

    #[test]
    fn passes_a_live_event_through_for_every_window() {
        let mut tracker = tracker();
        let updates = feed_windows(&mut tracker, &sine(440.0, WINDOW * 4));
        assert!(updates.iter().all(|update| update.live.is_some()));
    }

    #[test]
    fn accumulates_small_chunks_like_the_engine() {
        let mut tracker = tracker();
        let signal = sine(440.0, WINDOW * HOLD_WINDOWS as usize);
        let updates: Vec<_> = signal
            .chunks(128)
            .map(|chunk| tracker.process(chunk))
            .collect();
        assert_eq!(started_names(&updates), ["A4"]);
    }

    #[test]
    fn confirms_the_same_pitch_again_after_a_release() {
        let mut tracker = tracker();
        let mut updates = feed_windows(&mut tracker, &sine(440.0, WINDOW * 4));
        updates.extend(feed_windows(
            &mut tracker,
            &vec![0.0; WINDOW * HOLD_WINDOWS as usize],
        ));
        updates.extend(feed_windows(&mut tracker, &sine(440.0, WINDOW * 4)));
        assert_eq!(started_names(&updates), ["A4", "A4"]);
    }

    #[test]
    fn brief_dropout_does_not_duplicate_the_note() {
        let mut tracker = tracker();
        let mut updates = feed_windows(&mut tracker, &sine(440.0, WINDOW * 4));
        updates.extend(feed_windows(&mut tracker, &vec![0.0; WINDOW]));
        updates.extend(feed_windows(&mut tracker, &sine(440.0, WINDOW * 4)));
        assert_eq!(started_names(&updates), ["A4"]);
    }

    #[test]
    fn confirms_a_new_pitch_played_legato() {
        let mut tracker = tracker();
        let mut updates = feed_windows(&mut tracker, &sine(440.0, WINDOW * 4));
        updates.extend(feed_windows(&mut tracker, &sine(523.251, WINDOW * 4)));
        assert_eq!(started_names(&updates), ["A4", "C5"]);
    }

    #[test]
    fn alternating_pitches_never_confirm() {
        let mut tracker = tracker();
        let a4 = sine(440.0, WINDOW);
        let b4 = sine(493.883, WINDOW);
        let mut updates = Vec::new();
        for _ in 0..4 {
            updates.push(tracker.process(&a4));
            updates.push(tracker.process(&b4));
        }
        assert_eq!(started_names(&updates), Vec::<&str>::new());
    }

    #[test]
    fn noise_confirms_nothing() {
        let mut tracker = tracker();
        let updates = feed_windows(&mut tracker, &noise(WINDOW * 6));
        assert_eq!(started_names(&updates), Vec::<&str>::new());
    }
}
