//! The Player layout's search box in the top bar: the best matches from the
//! collection in a dropdown, each with Play next, Add to queue and Play now.
//! The queue is the Automix list, so all three put the track there; Play now
//! then has Automix mix into it as soon as it is waiting on the other deck.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use opendj_engine::{Track, NUM_DECKS};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::automix::AutoMix;
use crate::browser::Browser;
use crate::remote::held_count;
use crate::settings::show_key;
use crate::{AppWindow, QuickRow, QuickSearch};

/// How many matches the dropdown shows.
const SHOWN: usize = 8;
/// How many matches are ranked to pick those from.
const RANKED: usize = 400;
/// How long Play now waits for Automix to have the track ready.
const PLAY_NOW_WAIT: Duration = Duration::from_secs(20);

/// What a result's button asks for (QuickSearch.act's `mode`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    PlayNext,
    AddToQueue,
    PlayNow,
}

impl Action {
    fn from_mode(mode: i32) -> Self {
        match mode {
            1 => Action::AddToQueue,
            2 => Action::PlayNow,
            _ => Action::PlayNext,
        }
    }
}

pub struct Quick {
    /// Presses from the dropdown, carried out on the next tick, when the
    /// decks' tracks and Automix are at hand.
    jobs: Rc<RefCell<Vec<(usize, Action)>>>,
    /// The track Play now waits to mix into, and since when.
    play_now: Option<(usize, Instant)>,
}

impl Quick {
    pub fn install(app: &AppWindow, browser: &Rc<RefCell<Browser>>) -> Self {
        let jobs: Rc<RefCell<Vec<(usize, Action)>>> = Rc::default();
        let g = app.global::<QuickSearch>();
        let b = browser.clone();
        let weak = app.as_weak();
        g.on_search(move |text| {
            let (Ok(b), Some(app)) = (b.try_borrow(), weak.upgrade()) else {
                return;
            };
            let g = app.global::<QuickSearch>();
            let text = text.trim();
            if text.is_empty() {
                g.set_rows(ModelRc::default());
                g.set_total(0);
                return;
            }
            let (mut found, total) = b.find_tracks(text, 0, RANKED);
            rank(&mut found, text, |t| t.title.as_str());
            let rows: Vec<QuickRow> = found.iter().take(SHOWN).map(|t| row(&b, t)).collect();
            g.set_rows(ModelRc::new(VecModel::from(rows)));
            g.set_total(total as i32);
        });
        let j = jobs.clone();
        g.on_act(move |id, mode| {
            if id >= 0 {
                j.borrow_mut().push((id as usize, Action::from_mode(mode)));
            }
        });
        Quick {
            jobs,
            play_now: None,
        }
    }

    /// Carries out the dropdown's presses, and Play now's mix once its
    /// track waits on the other deck.
    pub fn tick(
        &mut self,
        app: &AppWindow,
        automix: &mut AutoMix,
        browser: &mut Browser,
        tracks: &[Option<Arc<Track>>; NUM_DECKS],
    ) {
        let jobs = std::mem::take(&mut *self.jobs.borrow_mut());
        for (id, action) in jobs {
            let at = match action {
                Action::AddToQueue => None,
                Action::PlayNext | Action::PlayNow => Some(held_count(automix, browser, tracks)),
            };
            let title = browser.track_by_id(id).map(|t| t.title.clone());
            if !browser.add_to_automix_at(id, at) {
                continue;
            }
            let title = title.unwrap_or_default();
            let note = match action {
                Action::PlayNext => format!("{title} plays next"),
                Action::AddToQueue => format!("{title} added to Up Next"),
                Action::PlayNow => format!("Mixing into {title}…"),
            };
            app.global::<QuickSearch>().set_note(note.into());
            if action == Action::PlayNow {
                self.play_now = Some((id, Instant::now()));
                if !automix.is_on() {
                    automix.set_enabled(true);
                }
            }
        }
        if let Some((id, since)) = self.play_now {
            let (playing, waiting) = automix.held();
            if playing == Some(id) || since.elapsed() > PLAY_NOW_WAIT {
                self.play_now = None;
            } else if waiting == Some(id) && automix.seconds_to_mix(tracks).is_some() {
                automix.mix_now();
                self.play_now = None;
            }
        }
    }
}

/// Puts tracks whose title has every typed word first, then those whose
/// title starts with the first word ahead of the rest of those, keeping the
/// collection's title order otherwise.
fn rank<T>(found: &mut [T], text: &str, title_of: impl Fn(&T) -> &str) {
    let words: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
    found.sort_by_cached_key(|t| {
        let title = title_of(t).to_lowercase();
        let all = words.iter().all(|w| title.contains(w.as_str()));
        let starts = words.first().is_some_and(|w| title.starts_with(w.as_str()));
        (!all, !starts)
    });
}

/// A dropdown row for `t`.
fn row(b: &Browser, t: &Track) -> QuickRow {
    let cover = b.cover_of(t);
    let key = show_key(&t.key, b.camelot());
    let mut detail = Vec::new();
    if t.bpm > 0.0 {
        detail.push(format!("{:.1}", t.bpm));
    }
    if !key.is_empty() {
        detail.push(key);
    }
    let s = t.duration.max(0.0).round() as u64;
    detail.push(format!("{}:{:02}", s / 60, s % 60));
    let initials: String = t
        .artist
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase();
    QuickRow {
        id: t.id as i32,
        title: t.title.as_str().into(),
        artist: t.artist.as_str().into(),
        detail: SharedString::from(detail.join(" · ")),
        has_cover: cover.is_some(),
        cover: cover.unwrap_or_default(),
        initials: initials.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{rank, Action};

    #[test]
    fn titles_first() {
        // Afterglow is by Lumen Drift: found by its artist only.
        let mut found = vec!["Afterglow", "Continental Drift", "Drift Theory"];
        rank(&mut found, "drift", |t| t);
        assert_eq!(found, ["Drift Theory", "Continental Drift", "Afterglow"]);
    }

    #[test]
    fn modes() {
        assert_eq!(Action::from_mode(0), Action::PlayNext);
        assert_eq!(Action::from_mode(1), Action::AddToQueue);
        assert_eq!(Action::from_mode(2), Action::PlayNow);
        assert_eq!(Action::from_mode(9), Action::PlayNext);
    }
}
