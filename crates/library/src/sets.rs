//! Crates: the DJ's long-lived lists at the top of the tree, a track in
//! as many as it fits (Yoga, Journeys, Dance), as against playlists, which
//! sit in folders. Also what Collection Health finds in playlists: crates
//! left empty, and entries for tracks taken out of the library.

use std::collections::HashMap;

use crate::{track_id, Error, Library, PlaylistKind};

/// What Collection Health finds in the DJ's lists.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListHealth {
    /// Crates with no tracks (playlists are left alone: one made for a gig
    /// starts empty).
    pub empty: Vec<i64>,
    /// Entries in lists for tracks removed from the library.
    pub dead: usize,
}

impl ListHealth {
    pub fn is_clean(&self) -> bool {
        self.empty.is_empty() && self.dead == 0
    }
}

/// Whether a list is a crate: one of the DJ's own lists at the top of the
/// tree, not in a folder.
pub fn is_crate(p: &crate::Playlist) -> bool {
    p.kind == PlaylistKind::List && p.parent.is_none() && p.source.is_none()
}

impl Library {
    /// The crates: the DJ's own lists at the top of the tree, all but the
    /// Inbox list downloads are noted in.
    fn crate_ids(&self) -> Result<HashMap<i64, String>, Error> {
        let inbox = self.inbox_list_id();
        Ok(self
            .playlists()?
            .into_iter()
            .filter(|p| is_crate(p) && Some(p.id) != inbox)
            .map(|p| (p.id, p.name))
            .collect())
    }

    /// The crates each track is in, by track id, each crate once, in name
    /// order.
    pub fn crates_by_track(&self) -> Result<HashMap<usize, Vec<String>>, Error> {
        let crates = self.crate_ids()?;
        let mut q = self
            .db
            .prepare("SELECT DISTINCT playlist_id, track_id FROM playlist_tracks")?;
        let mut out: HashMap<usize, Vec<String>> = HashMap::new();
        for row in q.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
            let (list, track) = row?;
            if let Some(name) = crates.get(&list) {
                out.entry(track_id(track)).or_default().push(name.clone());
            }
        }
        for names in out.values_mut() {
            names.sort_by_key(|n| n.to_lowercase());
            names.dedup();
        }
        Ok(out)
    }

    /// Empty crates and entries for removed tracks in the DJ's lists.
    pub fn list_health(&self) -> Result<ListHealth, Error> {
        let crates = self.crate_ids()?;
        let mut q = self.db.prepare(
            "SELECT p.id FROM playlists p WHERE p.kind = 0 AND p.source IS NULL
               AND NOT EXISTS (SELECT 1 FROM playlist_tracks t WHERE t.playlist_id = p.id)",
        )?;
        let empty = q
            .query_map([], |r| r.get::<_, i64>(0))?
            .filter_map(|r| r.ok())
            .filter(|id| crates.contains_key(id))
            .collect();
        let dead: i64 = self.db.query_row(
            "SELECT COUNT(*) FROM playlist_tracks
              WHERE track_id IN (SELECT track_id FROM removed)
                AND playlist_id IN (SELECT id FROM playlists WHERE kind = 0)",
            [],
            |r| r.get(0),
        )?;
        Ok(ListHealth {
            empty,
            dead: dead as usize,
        })
    }

    /// Deletes the empty crates and the entries for removed tracks that
    /// [`Library::list_health`] found; returns how many of each went.
    pub fn tidy_lists(&mut self) -> Result<(usize, usize), Error> {
        let health = self.list_health()?;
        let tx = self.db.transaction()?;
        for id in &health.empty {
            tx.execute("DELETE FROM playlists WHERE id = ?1", [id])?;
        }
        let dead = tx.execute(
            "DELETE FROM playlist_tracks
              WHERE track_id IN (SELECT track_id FROM removed)
                AND playlist_id IN (SELECT id FROM playlists WHERE kind = 0)",
            [],
        )?;
        tx.commit()?;
        Ok((health.empty.len(), dead))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::file_track;

    fn library() -> (Library, Vec<usize>) {
        let mut lib = Library::in_memory().unwrap();
        lib.sync(&[
            file_track("/m/a.mp3", "A"),
            file_track("/m/b.mp3", "B"),
            file_track("/m/c.mp3", "C"),
        ])
        .unwrap();
        let ids = lib.tracks(|_| true).unwrap().iter().map(|t| t.id).collect();
        (lib, ids)
    }

    #[test]
    fn crates_are_lists_at_the_top() {
        let (mut lib, ids) = library();
        let yoga = lib
            .create_playlist("Yoga", None, PlaylistKind::List)
            .unwrap();
        let folder = lib
            .create_playlist("Gigs", None, PlaylistKind::Folder)
            .unwrap();
        let friday = lib
            .create_playlist("Friday", Some(folder), PlaylistKind::List)
            .unwrap();
        let inbox = lib.inbox_list().unwrap();
        lib.add_to_playlist(yoga, &[ids[0], ids[1]]).unwrap();
        lib.add_to_playlist(friday, &[ids[0], ids[2]]).unwrap();
        lib.add_to_playlist(inbox, &[ids[2]]).unwrap();
        let crates = lib.crates_by_track().unwrap();
        assert_eq!(crates[&ids[0]], ["Yoga"]);
        assert_eq!(crates[&ids[1]], ["Yoga"]);
        assert!(!crates.contains_key(&ids[2]));
    }

    #[test]
    fn tidies_empty_crates_and_removed_tracks() {
        let (mut lib, ids) = library();
        let empty = lib
            .create_playlist("Old", None, PlaylistKind::List)
            .unwrap();
        let yoga = lib
            .create_playlist("Yoga", None, PlaylistKind::List)
            .unwrap();
        let gig = lib
            .create_playlist("Gig", None, PlaylistKind::Folder)
            .unwrap();
        lib.create_playlist("Set", Some(gig), PlaylistKind::List)
            .unwrap();
        lib.add_to_playlist(yoga, &[ids[0], ids[1]]).unwrap();
        lib.remove_tracks(&[ids[1]]).unwrap();
        let health = lib.list_health().unwrap();
        // The gig's playlist is empty too, but a new one is meant to be.
        assert_eq!(
            health,
            ListHealth {
                empty: vec![empty],
                dead: 1
            }
        );
        assert_eq!(lib.tidy_lists().unwrap(), (1, 1));
        assert!(lib.list_health().unwrap().is_clean());
        assert_eq!(lib.playlist_tracks(yoga).unwrap(), [ids[0]]);
    }
}
