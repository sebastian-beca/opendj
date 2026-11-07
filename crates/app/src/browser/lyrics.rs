//! The side panel's Lyrics tab: the lyrics of the track playing (or the
//! one selected, or a deck's), read from an `.lrc` file beside it or from
//! its tags (see [`opendj_engine::lyrics`]). Timed lyrics follow the deck
//! the track is on: the line sung now is lit, and a click on a line plays
//! from it.

use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;

use opendj_control::{ControlHandle, ControlRegistry};
use opendj_engine::lyrics::{self, Lyrics, Origin, Timing};
use opendj_engine::{deck_group, Source, Track, NUM_DECKS};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::Browser;
use crate::{AppWindow, LyricLine, LyricsUi};

/// The side panel's Lyrics tab. Past Prep's Cues (4) and Checklist (5),
/// which come and go with the layout.
pub(super) const LYRICS_TAB: usize = 6;

/// What the tab follows.
const PLAYING: i32 = 0;
const SELECTED: i32 = 1;
/// 2 and 3 are decks A and B.
const FIRST_DECK: i32 = 2;

/// A deck's controls the tab reads and sets.
struct DeckPlay {
    play: ControlHandle,
    playposition: ControlHandle,
    duration: ControlHandle,
}

pub(super) struct LyricsState {
    subject: i32,
    decks: Vec<DeckPlay>,
    /// The deck the Playing choice last followed, kept while it plays on.
    playing: Option<usize>,
    /// The track shown, where it is in `library`, and the deck it is on.
    shown: Option<(usize, Arc<Track>)>,
    deck: Option<usize>,
    /// The lyrics being read for the track shown, by generation.
    job: Option<(u64, Receiver<Option<Lyrics>>)>,
    generation: u64,
    lyrics: Option<Lyrics>,
    current: i32,
}

impl LyricsState {
    pub(super) fn new(registry: &ControlRegistry) -> Self {
        let decks = (0..NUM_DECKS)
            .map(|d| {
                let g = deck_group(d);
                let c = |k: &str| registry.get(&g, k).expect("deck control");
                DeckPlay {
                    play: c("play"),
                    playposition: c("playposition"),
                    duration: c("duration"),
                }
            })
            .collect();
        Self {
            subject: PLAYING,
            decks,
            playing: None,
            shown: None,
            deck: None,
            job: None,
            generation: 0,
            lyrics: None,
            current: -1,
        }
    }
}

/// m:ss for a line's time.
fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0).floor() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// What the tab says about where the lyrics came from.
fn describe(l: &Lyrics) -> String {
    let timing = match l.timing() {
        Timing::Words => "Timed by word",
        Timing::Lines => "Timed by line",
        Timing::Plain => "Not timed",
    };
    let from = match &l.origin {
        Origin::Lrc(p) => p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "an .lrc file".into()),
        Origin::Tag => "the file's tags".into(),
    };
    format!("{timing} · {from}")
}

impl Browser {
    pub(super) fn install_lyrics(
        app: &AppWindow,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let ui = app.global::<LyricsUi>();
        let w = with.clone();
        ui.on_set_subject(move |s| {
            w(&|b| {
                b.lyrics.subject = s.clamp(PLAYING, FIRST_DECK + 1);
                if let Some(app) = b.app.upgrade() {
                    app.global::<LyricsUi>().set_subject(b.lyrics.subject);
                    b.poll_lyrics(&app, LYRICS_TAB);
                }
            })
        });
        let w = with.clone();
        ui.on_seek(move |line| w(&|b| b.seek_to_line(line)));
    }

    /// The deck `index` (in `library`) is loaded on, the first if several.
    fn deck_with(&self, index: usize) -> Option<usize> {
        let id = self.library[index].id;
        (0..NUM_DECKS).find(|&d| self.loaded[d] == Some(id))
    }

    /// The track the tab is to show, and the deck it is on.
    fn lyrics_subject(&mut self, app: &AppWindow) -> (Option<usize>, Option<usize>) {
        match self.lyrics.subject {
            PLAYING => {
                let playing =
                    |d: usize| self.loaded[d].is_some() && self.lyrics.decks[d].play.get_bool();
                // Stay with the deck followed while it plays; else the first
                // deck playing; else the last one followed, paused; else the
                // first deck with a track.
                let deck = self
                    .lyrics
                    .playing
                    .filter(|&d| playing(d))
                    .or_else(|| (0..NUM_DECKS).find(|&d| playing(d)))
                    .or(self.lyrics.playing.filter(|&d| self.loaded[d].is_some()))
                    .or_else(|| (0..NUM_DECKS).find(|&d| self.loaded[d].is_some()));
                self.lyrics.playing = deck;
                (deck.and_then(|d| self.deck_track(d)), deck)
            }
            SELECTED => {
                let index = usize::try_from(app.get_current_track())
                    .ok()
                    .and_then(|r| self.rows.get(r).copied().flatten());
                (index, index.and_then(|i| self.deck_with(i)))
            }
            d => {
                let deck = (d - FIRST_DECK) as usize;
                (self.deck_track(deck), Some(deck))
            }
        }
    }

    /// Keeps the Lyrics tab up to date; called every frame.
    pub(super) fn poll_lyrics(&mut self, app: &AppWindow, tab: usize) {
        if tab != LYRICS_TAB {
            return;
        }
        let (index, deck) = self.lyrics_subject(app);
        let now = index.map(|i| (i, self.library[i].clone()));
        let other = match (&now, &self.lyrics.shown) {
            (Some((i, t)), Some((j, u))) => i != j || t.id != u.id,
            (None, None) => false,
            _ => true,
        };
        let ui = app.global::<LyricsUi>();
        if other {
            self.lyrics.shown = now.clone();
            self.lyrics.lyrics = None;
            self.lyrics.current = -1;
            self.lyrics.generation += 1;
            self.lyrics.job = None;
            ui.set_has_track(now.is_some());
            ui.set_title(
                now.as_ref()
                    .map(|(_, t)| {
                        if t.artist.is_empty() {
                            t.title.clone()
                        } else {
                            format!("{} · {}", t.artist, t.title)
                        }
                    })
                    .unwrap_or_default()
                    .into(),
            );
            ui.set_source(SharedString::new());
            ui.set_lines(ModelRc::default());
            ui.set_current(-1);
            if let Some((_, t)) = &now {
                if let Source::File(path) = &t.source {
                    let (tx, rx) = mpsc::channel();
                    let path = path.clone();
                    std::thread::spawn(move || {
                        let _ = tx.send(lyrics::find(&path));
                    });
                    self.lyrics.job = Some((self.lyrics.generation, rx));
                }
            }
        }
        if deck != self.lyrics.deck {
            self.lyrics.deck = deck;
        }
        if let Some((generation, rx)) = &self.lyrics.job {
            if let Ok(found) = rx.try_recv() {
                if *generation == self.lyrics.generation {
                    self.show_lyrics(&ui, found);
                }
                self.lyrics.job = None;
            }
        }
        // The line sung now, while the track is on a deck.
        let current = match (&self.lyrics.lyrics, deck) {
            (Some(l), Some(d)) => {
                let p = &self.lyrics.decks[d];
                l.line_at(p.playposition.get() * p.duration.get())
                    .map_or(-1, |i| i as i32)
            }
            _ => -1,
        };
        ui.set_can_seek(
            deck.is_some()
                && self
                    .lyrics
                    .lyrics
                    .as_ref()
                    .is_some_and(|l| l.timing() != Timing::Plain),
        );
        if current != self.lyrics.current {
            self.lyrics.current = current;
            ui.set_current(current);
        }
    }

    fn show_lyrics(&mut self, ui: &LyricsUi, found: Option<Lyrics>) {
        match &found {
            Some(l) => {
                let lines: Vec<LyricLine> = l
                    .lines
                    .iter()
                    .map(|line| LyricLine {
                        time: line.start.map(clock).unwrap_or_default().into(),
                        text: line.text.clone().into(),
                    })
                    .collect();
                ui.set_source(describe(l).into());
                ui.set_lines(ModelRc::new(VecModel::from(lines)));
            }
            None => {
                ui.set_source(SharedString::new());
                ui.set_lines(ModelRc::default());
            }
        }
        self.lyrics.current = -1;
        ui.set_current(-1);
        self.lyrics.lyrics = found;
    }

    /// Plays the deck the track is on from the start of `line`.
    fn seek_to_line(&mut self, line: i32) {
        let (Some(l), Some(d)) = (&self.lyrics.lyrics, self.lyrics.deck) else {
            return;
        };
        let Some(start) = usize::try_from(line)
            .ok()
            .and_then(|i| l.lines.get(i))
            .and_then(|l| l.start)
        else {
            return;
        };
        let p = &self.lyrics.decks[d];
        let duration = p.duration.get();
        if duration > 0.0 {
            p.playposition.set((start / duration).clamp(0.0, 1.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn clock_reads_as_minutes() {
        assert_eq!(clock(0.0), "0:00");
        assert_eq!(clock(61.9), "1:01");
        assert_eq!(clock(600.0), "10:00");
    }

    #[test]
    fn describes_where_from() {
        let lines = lyrics::parse("[00:01.00]<00:01.00>Hi <00:01.50>there").unwrap();
        let l = Lyrics {
            lines,
            origin: Origin::Lrc(PathBuf::from("/music/Song.lrc")),
        };
        assert_eq!(describe(&l), "Timed by word · Song.lrc");
        let l = Lyrics {
            lines: lyrics::parse("Just words").unwrap(),
            origin: Origin::Tag,
        };
        assert_eq!(describe(&l), "Not timed · the file's tags");
    }
}
