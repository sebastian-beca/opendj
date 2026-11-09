//! Favorites, the Player bar's heart: one click puts the track playing in
//! the Favorites playlist, or takes it out, as Liked Songs in other music
//! players. The list is made at the top of Playlists the first time; its id
//! is kept in the settings, so renaming or moving it keeps it the heart's.

use slint::{ComponentHandle, ModelRc, VecModel};

use super::{Browser, COLLECTION};
use crate::{settings, LibraryEdit};

/// The setting holding the Favorites list's id.
const FAVORITES: &str = "favorites_list";
/// Its name when it is made.
const NAME: &str = "Favorites";

impl Browser {
    /// The Favorites list's id, while it exists.
    fn favorites(&self) -> Option<i64> {
        let id = settings::load()[FAVORITES].as_i64()?;
        let db = self.db.as_ref()?;
        db.playlists()
            .ok()?
            .iter()
            .any(|p| p.id == id)
            .then_some(id)
    }

    /// The Favorites list, made at the top of Playlists if there is none.
    fn favorites_or_make(&mut self) -> Option<i64> {
        if let Some(id) = self.favorites() {
            return Some(id);
        }
        let id = match self.db.as_ref()?.create_top_playlist(NAME) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("can't make Favorites: {e}");
                return None;
            }
        };
        settings::set(FAVORITES, serde_json::json!(id));
        if self.tree == COLLECTION && !self.health_view {
            self.reload_tree(None);
        } else {
            self.collection_stale = true;
        }
        Some(id)
    }

    /// The heart: the track on `deck` goes into Favorites, or comes out if
    /// it is in already (every time it is there).
    pub(super) fn toggle_favorite(&mut self, deck: usize) {
        if !self.saved() {
            return;
        }
        let (Some(index), Some(Some(track))) = (self.deck_index(deck), self.loaded.get(deck))
        else {
            return;
        };
        let track = *track;
        let Some(list) = self.favorites_or_make() else {
            return;
        };
        let Some(db) = &mut self.db else { return };
        let tracks = db.playlist_tracks(list).unwrap_or_default();
        let places: Vec<usize> = tracks
            .iter()
            .enumerate()
            .filter(|(_, &t)| t == track)
            .map(|(i, _)| i)
            .collect();
        if places.is_empty() {
            self.add_to_playlist(&[index], list);
        } else {
            // From the last, so the places before stay where they are.
            for &place in places.iter().rev() {
                if let Err(e) = db.remove_from_playlist(list, place) {
                    eprintln!("can't take the track out of Favorites: {e}");
                }
            }
            self.refresh_crates();
            if self.nodes.get(self.current_node).and_then(|n| n.playlist) == Some(list) {
                self.refresh_rows();
            }
        }
        self.show_favorites();
    }

    /// Lights the heart for each deck whose track is in Favorites.
    pub(super) fn show_favorites(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let tracks = self
            .favorites()
            .and_then(|list| self.db.as_ref()?.playlist_tracks(list).ok())
            .unwrap_or_default();
        let on: Vec<bool> = self
            .loaded
            .iter()
            .map(|t| t.is_some_and(|t| tracks.contains(&t)))
            .collect();
        app.global::<LibraryEdit>()
            .set_deck_favorites(ModelRc::new(VecModel::from(on)));
    }
}
