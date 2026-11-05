//! The track end warning: a playing deck's overview flashes and its remaining
//! time turns red when its track is about to end, as Mixxx's does.
//!
//! The warning time is picked in Settings › Decks and kept here, so the
//! decks can read it every frame without reading the settings file.

use std::sync::atomic::{AtomicU32, Ordering};

use crate::settings;

/// The choices in Settings, in seconds; 0 is off.
pub const CHOICES: [u32; 6] = [0, 10, 20, 30, 45, 60];
/// The labels for [`CHOICES`].
pub const LABELS: [&str; 6] = ["Off", "10 s", "20 s", "30 s", "45 s", "60 s"];
/// Mixxx's default.
const DEFAULT: u32 = 30;
/// The last stretch, where the flash speeds up.
const HURRY: f64 = 10.0;

pub const KEY: &str = "track_end_warning";

static SECONDS: AtomicU32 = AtomicU32::new(DEFAULT);

/// Reads the saved warning time; called once at start.
pub fn load() {
    let s = settings::number(KEY, DEFAULT as f64);
    SECONDS.store(nearest(s), Ordering::Relaxed);
}

/// The warning time in seconds, 0 when off.
pub fn seconds() -> u32 {
    SECONDS.load(Ordering::Relaxed)
}

/// Which of [`CHOICES`] is in use.
pub fn chosen() -> usize {
    CHOICES.iter().position(|&c| c == seconds()).unwrap_or(0)
}

/// Picks one of [`CHOICES`] and saves it.
pub fn choose(index: usize) {
    let s = CHOICES[index.min(CHOICES.len() - 1)];
    SECONDS.store(s, Ordering::Relaxed);
    settings::set(KEY, serde_json::json!(s));
}

/// The choice closest to a saved number, so a hand-edited value still works.
fn nearest(s: f64) -> u32 {
    CHOICES
        .iter()
        .copied()
        .min_by(|a, b| (*a as f64 - s).abs().total_cmp(&(*b as f64 - s).abs()))
        .unwrap_or(DEFAULT)
}

/// Whether a deck warns (`.0`) and whether its flash is lit this instant
/// (`.1`), `remaining` seconds from the end. A deck warns only while it
/// plays outside a loop: a paused or looping track won't run out.
pub fn warning(remaining: f64, playing: bool, looping: bool, warn_seconds: u32) -> (bool, bool) {
    if warn_seconds == 0 || !playing || looping || remaining <= 0.0 {
        return (false, false);
    }
    if remaining > warn_seconds as f64 {
        return (false, false);
    }
    // Lit for the first half of each second counting down; twice as fast
    // in the last stretch. Following the track's time keeps it in step with
    // the clock shown beside it.
    let rate = if remaining <= HURRY { 2.0 } else { 1.0 };
    let phase = (remaining * rate).fract();
    (true, phase >= 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_before_the_warning_time() {
        assert_eq!(warning(31.0, true, false, 30), (false, false));
        assert!(warning(30.0, true, false, 30).0);
        assert!(warning(0.2, true, false, 30).0);
    }

    #[test]
    fn quiet_when_off_paused_looping_or_ended() {
        assert_eq!(warning(5.0, true, false, 0), (false, false));
        assert_eq!(warning(5.0, false, false, 30), (false, false));
        assert_eq!(warning(5.0, true, true, 30), (false, false));
        assert_eq!(warning(0.0, true, false, 30), (false, false));
    }

    #[test]
    fn flashes_once_a_second_then_faster() {
        // 20.9 s to go: lit; 20.4 s: dark.
        assert!(warning(20.9, true, false, 30).1);
        assert!(!warning(20.4, true, false, 30).1);
        // In the last ten seconds, twice a second: lit at 5.3 (10.6) and
        // 5.8 (11.6), dark at 5.1 (10.2).
        assert!(warning(5.3, true, false, 30).1);
        assert!(warning(5.8, true, false, 30).1);
        assert!(!warning(5.1, true, false, 30).1);
    }

    #[test]
    fn saved_values_snap_to_a_choice() {
        assert_eq!(nearest(30.0), 30);
        assert_eq!(nearest(0.0), 0);
        assert_eq!(nearest(40.0), 45);
        assert_eq!(nearest(500.0), 60);
    }
}
