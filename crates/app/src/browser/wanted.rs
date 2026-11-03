//! The Wanted list: tracks the DJ means to get. Only the DJ puts tracks on
//! it: Buy on a store result, Copy to openDJ on a streaming playlist, or
//! Add to Wanted on streaming tracks and playlists (Spotify, TIDAL…) and on
//! the streamed tracks standing in lists brought in from another app. A
//! track leaves it when the collection gets it, or with Remove from Wanted.
//!
//! The same menus offer Add to Downloads when an add-on source downloads
//! (see `StoreUi.downloads-to`); a store's cart can join them later, beside
//! these two.

use std::cell::RefCell;
use std::rc::Rc;

use opendj_library::ListEntry;
use slint::ComponentHandle;

use super::{Browser, Contents, StoreRow};
use crate::{AppWindow, WantedUi};

/// A track to want: its service or store, artist, title and page.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Want {
    pub store: String,
    pub artist: String,
    pub title: String,
    pub page: String,
}

impl Want {
    /// "Artist - Title", what a download source searches for.
    pub fn search_text(&self) -> String {
        search_text(&self.artist, &self.title)
    }
}

fn search_text(artist: &str, title: &str) -> String {
    if artist.is_empty() {
        title.to_string()
    } else {
        format!("{artist} - {title}")
    }
}

/// What "Add to Wanted" on a tree's list does now.
pub(super) enum ListWants {
    /// Its tracks the collection hasn't got.
    Now(Vec<Want>),
    /// A streaming playlist whose tracks haven't come yet: asked for, and
    /// wanted once they come.
    Later,
}

impl Browser {
    pub(super) fn install_wanted(app: &AppWindow, b: &Rc<RefCell<Self>>) {
        let ui = app.global::<WantedUi>();
        let w = Rc::downgrade(b);
        ui.on_add_tracks(move |first, last| {
            if let Some(b) = w.upgrade() {
                b.borrow_mut()
                    .want_rows(first.max(0) as usize, last.max(0) as usize);
            }
        });
        let w = Rc::downgrade(b);
        ui.on_add_list(move |row| {
            if let Some(b) = w.upgrade() {
                let mut b = b.borrow_mut();
                if let Some(node) = b.tree_node(row) {
                    b.want_list(node);
                }
            }
        });
        let w = Rc::downgrade(b);
        ui.on_list_streamed(move |row| {
            w.upgrade()
                .and_then(|b| {
                    let b = b.try_borrow().ok()?;
                    Some(b.list_streamed(b.tree_node(row)?))
                })
                .unwrap_or(false)
        });
        let w = Rc::downgrade(b);
        ui.on_remove(move |first, last| {
            if let Some(b) = w.upgrade() {
                b.borrow_mut()
                    .unwant_rows(first.max(0) as usize, last.max(0) as usize);
            }
        });
        let w = Rc::downgrade(b);
        ui.on_open(move |row| {
            if let Some(b) = w.upgrade() {
                b.borrow_mut().open_wanted(row.max(0) as usize);
            }
        });
    }

    /// The node shown at `row` of the tree.
    fn tree_node(&self, row: i32) -> Option<usize> {
        self.visible_nodes.get(usize::try_from(row).ok()?).copied()
    }

    /// Whether the list shows the Wanted list.
    pub(super) fn shows_wanted(&self) -> bool {
        self.nodes
            .get(self.current_node)
            .is_some_and(|n| matches!(n.contents, Contents::Wanted))
    }

    /// The streamed tracks in rows `first` to `last` (every picked row,
    /// when `first` is one of several) that the collection hasn't got.
    pub(super) fn wants_at(&self, first: usize, last: usize) -> Vec<Want> {
        if self.shows_store_rows() {
            return Vec::new();
        }
        let service = self.shown_stream().map(|st| st.name.to_string());
        self.chosen_rows(first.min(last), first.max(last))
            .into_iter()
            .filter(|&row| self.rows.get(row).is_some_and(Option::is_none))
            .filter_map(|row| {
                let t = self.stream_track(row)?;
                // A streamed track in an imported list shows its service
                // where the album would be.
                let store = match (&service, self.placed.entries.is_some()) {
                    (_, true) | (None, _) => t.album.clone(),
                    (Some(s), false) => s.clone(),
                };
                Some(Want {
                    store,
                    artist: t.artist(),
                    title: t.title.clone(),
                    page: t.page.clone(),
                })
            })
            .collect()
    }

    /// The wanted tracks in rows `first` to `last` of the Wanted list.
    fn wanted_at(&self, first: usize, last: usize) -> Vec<&opendj_library::Wanted> {
        if !self.shows_wanted() {
            return Vec::new();
        }
        self.chosen_rows(first.min(last), first.max(last))
            .into_iter()
            .filter_map(|row| match self.store.rows.get(row) {
                Some(StoreRow::Wanted(w)) => Some(w),
                _ => None,
            })
            .collect()
    }

    /// "Artist - Title" for each track in rows `first` to `last`, for a
    /// download source to look for: streamed tracks, or wanted ones.
    pub fn download_names(&self, first: usize, last: usize) -> Vec<String> {
        if self.shows_wanted() {
            return self
                .wanted_at(first, last)
                .into_iter()
                .map(|w| search_text(&w.artist, &w.title))
                .collect();
        }
        if self.shows_store_rows() {
            return Vec::new();
        }
        self.chosen_rows(first.min(last), first.max(last))
            .into_iter()
            .filter_map(|row| self.stream_track(row))
            // The first artist only: a download source finds more that way.
            .map(|t| search_text(t.artists.first().map_or("", String::as_str), &t.title))
            .collect()
    }

    /// Puts tracks on the Wanted list; returns how many weren't on it.
    fn want(&mut self, wants: &[Want]) -> usize {
        let Some(db) = &self.db else { return 0 };
        let mut added = 0;
        for w in wants.iter().filter(|w| !w.page.is_empty()) {
            match db.add_wanted(&w.store, &w.artist, &w.title, &w.page) {
                Ok(true) => added += 1,
                Ok(false) => {}
                Err(e) => {
                    eprintln!("can't add to the wanted list: {e}");
                    break;
                }
            }
        }
        self.store.wanted = db.wanted().unwrap_or_default();
        added
    }

    /// Says what adding (to Wanted or Downloads) did: under the streaming
    /// service's playlists, or in the header of the list shown.
    pub fn wanted_note(&mut self, text: String) {
        if self.shown_stream().is_some() {
            self.set_stream_status(text);
        } else if let Some(Contents::Playlist(id)) = self
            .nodes
            .get(self.current_node)
            .map(|n| n.contents.clone())
        {
            self.export_status(id, text);
        }
        // The tree's Wanted count.
        self.refresh_tree();
    }

    /// "Add to Wanted" on streamed tracks in the list.
    fn want_rows(&mut self, first: usize, last: usize) {
        let wants = self.wants_at(first, last);
        if wants.is_empty() {
            return;
        }
        let added = self.want(&wants);
        let text = added_text(added, wants.len());
        self.wanted_note(text);
    }

    /// The tracks the collection hasn't got in the tree's node `index`:
    /// a streaming playlist's, or the streamed tracks standing in an
    /// imported list. None when it isn't such a list.
    /// A streaming playlist not fetched yet is asked for; with
    /// `want_later` its tracks go on the Wanted list once they come.
    pub(super) fn list_wants(&mut self, index: usize, want_later: bool) -> Option<ListWants> {
        match self.nodes.get(index)?.contents.clone() {
            Contents::Stream(key) => {
                let st = self.shown_stream()?;
                let Some(tracks) = st.state.tracks.get(&key).cloned() else {
                    let st = self.shown_stream_mut()?;
                    if st.state.loading.insert(key.clone()) {
                        st.service
                            .send(crate::spotify::Request::Tracks(key.clone()));
                    }
                    if want_later {
                        st.wanted_later.insert(key);
                    }
                    return Some(ListWants::Later);
                };
                let name = st.name.to_string();
                let found = self.spotify_matches(&tracks);
                Some(ListWants::Now(
                    tracks
                        .iter()
                        .zip(found)
                        .filter(|(_, f)| f.is_none())
                        .map(|(t, _)| Want {
                            store: name.clone(),
                            artist: t.artist(),
                            title: t.title.clone(),
                            page: t.page.clone(),
                        })
                        .collect(),
                ))
            }
            Contents::Playlist(id) => {
                let entries = self.db.as_ref()?.playlist_entries(id).ok()?;
                Some(ListWants::Now(
                    entries
                        .into_iter()
                        .filter_map(|e| match e {
                            ListEntry::Streamed(s) => Some(Want {
                                store: s.service_name().to_string(),
                                artist: s.artist.clone(),
                                title: s.title.clone(),
                                page: s.page_url()?,
                            }),
                            ListEntry::Track(_) => None,
                        })
                        .collect(),
                ))
            }
            _ => None,
        }
    }

    /// "Add to Wanted" on a list in the tree, by its node.
    fn want_list(&mut self, index: usize) {
        match self.list_wants(index, true) {
            Some(ListWants::Now(wants)) if wants.is_empty() => {
                self.wanted_note("Your collection has every track of this list.".into())
            }
            Some(ListWants::Now(wants)) => {
                let added = self.want(&wants);
                let text = added_text(added, wants.len());
                self.wanted_note(text);
            }
            Some(ListWants::Later) => {
                self.wanted_note("Adding the playlist's tracks to Wanted once it loads…".into())
            }
            None => {}
        }
    }

    /// "Add to Downloads" on a list in the tree that has no link a download
    /// source takes: "Artist - Title" for each track the collection hasn't
    /// got.
    pub fn list_download_names(&mut self, row: usize) -> Vec<String> {
        let Some(node) = self.tree_node(row as i32) else {
            return Vec::new();
        };
        match self.list_wants(node, false) {
            Some(ListWants::Now(wants)) => wants.iter().map(Want::search_text).collect(),
            Some(ListWants::Later) => {
                self.wanted_note(
                    "Loading the playlist; pick Add to Downloads again once it shows.".into(),
                );
                Vec::new()
            }
            None => Vec::new(),
        }
    }

    /// A streaming playlist asked for by "Add to Wanted" has come.
    pub(super) fn want_arrived(&mut self, stream: usize, key: &str) {
        if self.tree != self.streams[stream].source {
            return;
        }
        let Some(node) = self
            .nodes
            .iter()
            .position(|n| matches!(&n.contents, Contents::Stream(k) if k == key))
        else {
            return;
        };
        self.want_list(node);
    }

    /// Whether "Add to Wanted" fits the tree's node `index`: a streaming
    /// playlist, or a list with streamed tracks standing in it.
    pub(super) fn list_streamed(&self, index: usize) -> bool {
        match self.nodes.get(index).map(|n| &n.contents) {
            Some(Contents::Stream(_)) => self.shown_stream().is_some(),
            Some(Contents::Playlist(id)) => self.db.as_ref().is_some_and(|db| {
                db.playlist_entries(*id)
                    .is_ok_and(|e| e.iter().any(|e| matches!(e, ListEntry::Streamed(_))))
            }),
            _ => false,
        }
    }

    /// "Remove from Wanted" on rows of the Wanted list.
    fn unwant_rows(&mut self, first: usize, last: usize) {
        let ids: Vec<i64> = self.wanted_at(first, last).iter().map(|w| w.id).collect();
        if ids.is_empty() {
            return;
        }
        let Some(db) = &mut self.db else { return };
        if let Err(e) = db.remove_wanted_many(&ids) {
            eprintln!("can't take tracks off the wanted list: {e}");
        }
        self.store.wanted = db.wanted().unwrap_or_default();
        self.refresh_rows();
        self.refresh_tree();
    }

    /// "Open Page" on a row of the Wanted list.
    fn open_wanted(&mut self, row: usize) {
        if let Some(StoreRow::Wanted(w)) = self.store.rows.get(row) {
            crate::spotify::open_in_browser(&w.page_url);
        }
    }
}

/// What adding `asked` tracks to the Wanted list did, `added` of them new.
fn added_text(added: usize, asked: usize) -> String {
    let tracks = |n: usize| if n == 1 { "track" } else { "tracks" };
    match (added, asked) {
        (0, 1) => "It's on the Wanted list already.".into(),
        (0, _) => "They're on the Wanted list already.".into(),
        (a, n) if a == n => format!("Added {a} {} to Wanted.", tracks(a)),
        (a, n) => format!(
            "Added {a} {} to Wanted; {} were there already.",
            tracks(a),
            n - a
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_what_adding_did() {
        assert_eq!(added_text(1, 1), "Added 1 track to Wanted.");
        assert_eq!(added_text(0, 1), "It's on the Wanted list already.");
        assert_eq!(
            added_text(3, 5),
            "Added 3 tracks to Wanted; 2 were there already."
        );
    }

    #[test]
    fn searches_by_artist_and_title() {
        let w = Want {
            store: "TIDAL".into(),
            artist: "Satorio".into(),
            title: "Crystal Bowls".into(),
            page: String::new(),
        };
        assert_eq!(w.search_text(), "Satorio - Crystal Bowls");
    }
}
