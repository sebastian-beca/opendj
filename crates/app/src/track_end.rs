//! The track end warning: when a playing deck's track is about to end, its
//! remaining time in the header blinks red, its jog's ring blinks in the
//! deck color, or both.
//!
//! The warning time is picked in Settings › Decks and kept here, so the
//! decks can read it every frame without reading the settings file.

use std::sync::atomic::{AtomicU32, Ordering};

use crate::settings;

/// The choices in Settings, in seconds; 0 is off.
pub const CHOICES: [u32; 6] = [0, 10, 20, 30, 45, 60];
/// The labels for [`CHOICES`].
pub const LABELS: [&str; 6] = ["Off", "10 s", "20 s", "30 s", "45 s", "60 s"];
const DEFAULT: u32 = 30;

pub const KEY: &str = "track_end_warning";

static SECONDS: AtomicU32 = AtomicU32::new(DEFAULT);

/// What blinks, as Settings › Decks › Warning type picks it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Style {
    Time = 0,
    Deck = 1,
    Both = 2,
}

/// The labels for [`Style`], in order.
pub const STYLE_LABELS: [&str; 3] = ["Time", "Deck", "Both"];
pub const STYLE_KEY: &str = "track_end_style";

static STYLE: AtomicU32 = AtomicU32::new(Style::Time as u32);

impl Style {
    fn from_index(i: usize) -> Self {
        match i {
            1 => Style::Deck,
            2 => Style::Both,
            _ => Style::Time,
        }
    }
    /// The remaining time blinks.
    pub fn time(self) -> bool {
        self != Style::Deck
    }
    /// The jog's ring blinks.
    pub fn ring(self) -> bool {
        self != Style::Time
    }
}

pub fn style() -> Style {
    Style::from_index(STYLE.load(Ordering::Relaxed) as usize)
}

pub fn choose_style(index: usize) {
    let s = Style::from_index(index);
    STYLE.store(s as u32, Ordering::Relaxed);
    settings::set(STYLE_KEY, serde_json::json!(s as u32));
}

/// Reads the saved warning time; called once at start.
pub fn load() {
    let s = settings::number(KEY, DEFAULT as f64);
    SECONDS.store(nearest(s), Ordering::Relaxed);
    let style = Style::from_index(settings::number(STYLE_KEY, 0.0).max(0.0) as usize);
    STYLE.store(style as u32, Ordering::Relaxed);
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

/// Whether a deck's remaining time is lit red this instant, `remaining`
/// seconds from the end. It blinks once a second through the last
/// `warn_seconds`, following the track's clock so it keeps step with the
/// time shown. Only while the deck plays outside a loop: a paused or looping
/// track won't run out.
pub fn flash(remaining: f64, playing: bool, looping: bool, warn_seconds: u32) -> bool {
    if warn_seconds == 0 || !playing || looping || remaining <= 0.0 {
        return false;
    }
    remaining <= warn_seconds as f64 && remaining.fract() >= 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_before_the_warning_time() {
        assert!(!flash(31.7, true, false, 30));
        assert!(flash(29.7, true, false, 30));
        assert!(flash(0.6, true, false, 30));
    }

    #[test]
    fn quiet_when_off_paused_looping_or_ended() {
        assert!(!flash(5.7, true, false, 0));
        assert!(!flash(5.7, false, false, 30));
        assert!(!flash(5.7, true, true, 30));
        assert!(!flash(0.0, true, false, 30));
    }

    #[test]
    fn blinks_once_a_second() {
        assert!(flash(20.9, true, false, 30));
        assert!(!flash(20.4, true, false, 30));
        assert!(flash(5.5, true, false, 30));
        assert!(!flash(5.1, true, false, 30));
    }

    #[test]
    fn styles_pick_what_blinks() {
        assert!(Style::Time.time() && !Style::Time.ring());
        assert!(!Style::Deck.time() && Style::Deck.ring());
        assert!(Style::Both.time() && Style::Both.ring());
        assert_eq!(Style::from_index(7), Style::Time);
    }

    #[test]
    fn saved_values_snap_to_a_choice() {
        assert_eq!(nearest(30.0), 30);
        assert_eq!(nearest(0.0), 0);
        assert_eq!(nearest(40.0), 45);
        assert_eq!(nearest(500.0), 60);
    }
}
