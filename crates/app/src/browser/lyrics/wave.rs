//! Lyrics along the bottom of each deck's big waveform: the timed lyrics
//! of the track on the deck, read when it loads (see
//! [`opendj_engine::lyrics::find_with`]), each line a strip from when it
//! is sung to when it ends. The lane itself is drawn in Slint
//! (`LyricsLane`), scrolling with the waveform.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver};

use opendj_engine::lyrics::{self, Line, Lyrics};
use opendj_engine::{Source, NUM_DECKS};
use slint::{ComponentHandle, Model, ModelRc, VecModel};

use super::super::Browser;
use super::store;
use crate::{settings, AppWindow, DeckLines, WaveLine, WaveLyrics};

/// The setting: Settings › Decks › Lyrics on waveforms, on at first.
pub const SHOWN: &str = "waveform_lyrics";

/// The setting, read once (the settings file isn't read every frame) and
/// then kept by [`set_shown`]: 0 not read yet, 1 off, 2 on.
static SHOWN_NOW: AtomicU8 = AtomicU8::new(0);

pub fn shown() -> bool {
    match SHOWN_NOW.load(Ordering::Relaxed) {
        0 => {
            let on = settings::flag(SHOWN, true);
            set_shown(on);
            on
        }
        n => n == 2,
    }
}

/// Settings › Decks › Lyrics on waveforms was switched.
pub fn set_shown(on: bool) {
    SHOWN_NOW.store(if on { 2 } else { 1 }, Ordering::Relaxed);
}

#[derive(Default)]
struct Deck {
    /// The track whose lyrics the lane has or is reading.
    id: Option<usize>,
    job: Option<Receiver<Option<Lyrics>>>,
}

#[derive(Default)]
pub(in crate::browser) struct WaveState {
    decks: Vec<Deck>,
}

impl WaveState {
    /// Reads every deck's lyrics again, as after new ones were saved.
    pub(super) fn reload(&mut self) {
        for d in &mut self.decks {
            d.id = None;
            d.job = None;
        }
    }
}

/// The lines of timed lyrics that are sung, with when each ends.
pub(super) fn wave_lines(lines: &[Line]) -> Vec<WaveLine> {
    lines
        .iter()
        .enumerate()
        .filter(|(_, l)| !l.text.trim().is_empty())
        .filter_map(|(i, l)| {
            let start = l.start?;
            let until = lines.get(i + 1).and_then(|n| n.start);
            let end = l.word_times(until).last().map_or(start, |w| w.1);
            Some(WaveLine {
                start: start as f32,
                end: end.max(start + 0.2) as f32,
                text: l.text.trim().into(),
            })
        })
        .collect()
}

impl Browser {
    /// Keeps each deck's lyrics lane up to date; called every frame.
    pub(in crate::browser) fn poll_wave_lyrics(&mut self, app: &AppWindow) {
        let ui = app.global::<WaveLyrics>();
        let on = shown();
        if ui.get_shown() != on {
            ui.set_shown(on);
        }
        let model = ui.get_decks();
        if model.row_count() != NUM_DECKS {
            ui.set_decks(ModelRc::new(VecModel::from(vec![
                DeckLines::default();
                NUM_DECKS
            ])));
        }
        let model = ui.get_decks();
        if self.lyrics.wave.decks.len() != NUM_DECKS {
            self.lyrics.wave.decks = (0..NUM_DECKS).map(|_| Deck::default()).collect();
        }
        for d in 0..NUM_DECKS {
            let id = if on { self.loaded[d] } else { None };
            if id != self.lyrics.wave.decks[d].id {
                let path = self.deck_track(d).filter(|_| id.is_some()).and_then(|i| {
                    match &self.library[i].source {
                        Source::File(p) => Some(p.clone()),
                        _ => None,
                    }
                });
                let deck = &mut self.lyrics.wave.decks[d];
                deck.id = id;
                deck.job = path.map(|path| {
                    let (tx, rx) = mpsc::channel();
                    let store = store();
                    std::thread::spawn(move || {
                        let _ = tx.send(lyrics::find_with(&path, store.as_deref()));
                    });
                    rx
                });
                model.set_row_data(d, DeckLines::default());
            }
            let deck = &mut self.lyrics.wave.decks[d];
            if let Some(found) = deck.job.as_ref().and_then(|rx| rx.try_recv().ok()) {
                deck.job = None;
                let lines = found.map(|l| wave_lines(&l.lines)).unwrap_or_default();
                model.set_row_data(
                    d,
                    DeckLines {
                        lines: ModelRc::new(VecModel::from(lines)),
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_run_until_sung() {
        let lines = lyrics::parse("[00:10.00]Hi there\n[00:11.00]\n[00:20.00]Next").unwrap();
        let w = wave_lines(&lines);
        assert_eq!(w.len(), 2, "breaks have no strip");
        assert_eq!(w[0].text, "Hi there");
        assert_eq!(w[0].start, 10.0);
        // Ends by the next line's start.
        assert!(w[0].end <= 11.0 && w[0].end > 10.0);
        assert!(lyrics::parse("Plain")
            .map(|l| wave_lines(&l))
            .unwrap()
            .is_empty());
    }
}
