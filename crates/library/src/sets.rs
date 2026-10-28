//! Crates and sets, the two kinds of playlist: a crate is a long-lived
//! list a track can sit in alongside others (Yoga, Journeys, Dance), and a
//! set is a folder made for one gig from a template of lists (Staging, Set,
//! Live set). Also what Library Health finds in playlists: lists left
//! empty, and entries for tracks taken out of the library.

use std::collections::HashMap;

use rusqlite::params;

use crate::{now, track_id, Error, Library, PlaylistKind};

/// The lists a new set gets when the DJ hasn't changed the template.
pub const TEMPLATE: [&str; 3] = ["Staging", "Set", "Live set"];

/// The list of a set that History's "Add to Set List…" fills.
pub const LIVE_SET: &str = "Live set";

/// What Library Health finds in the DJ's lists.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListHealth {
    /// Crates with no tracks (lists in a set are left alone: a new set
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

impl Library {
    /// Makes a set folder named `name` at the top of the tree with a list
    /// for each of `lists`, and returns the folder's id.
    pub fn create_set(&mut self, name: &str, lists: &[String]) -> Result<i64, Error> {
        let tx = self.db.transaction()?;
        let position: i64 = tx.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM playlists WHERE parent IS NULL",
            [],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT INTO playlists (parent, name, kind, position, created)
             VALUES (NULL, ?1, ?2, ?3, ?4)",
            params![name, PlaylistKind::Set.to_i64(), position, now()],
        )?;
        let set = tx.last_insert_rowid();
        let names = lists.iter().map(|l| l.trim()).filter(|l| !l.is_empty());
        for (position, list) in names.enumerate() {
            tx.execute(
                "INSERT INTO playlists (parent, name, kind, position, created)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    set,
                    list,
                    PlaylistKind::List.to_i64(),
                    position as i64,
                    now()
                ],
            )?;
        }
        tx.commit()?;
        Ok(set)
    }

    /// The set folders, newest first: (id, name).
    pub fn sets(&self) -> Result<Vec<(i64, String)>, Error> {
        let mut q = self.db.prepare(
            "SELECT id, name FROM playlists WHERE kind = ?1 ORDER BY created DESC, id DESC",
        )?;
        let sets = q
            .query_map([PlaylistKind::Set.to_i64()], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect();
        sets
    }

    /// The set's Live set list, made at its end when the set has none.
    pub fn live_set(&self, set: i64) -> Result<i64, Error> {
        let mut q = self
            .db
            .prepare("SELECT id, name FROM playlists WHERE parent = ?1 AND kind = ?2")?;
        let lists: Vec<(i64, String)> = q
            .query_map(params![set, PlaylistKind::List.to_i64()], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .collect::<Result<_, _>>()?;
        if let Some((id, _)) = lists
            .iter()
            .find(|(_, n)| n.trim().eq_ignore_ascii_case(LIVE_SET))
        {
            return Ok(*id);
        }
        self.create_playlist(LIVE_SET, Some(set), PlaylistKind::List)
    }

    /// The lists that are crates: the DJ's lists outside any set.
    fn crate_ids(&self) -> Result<HashMap<i64, String>, Error> {
        let all = self.playlists()?;
        let kind: HashMap<i64, (Option<i64>, PlaylistKind)> =
            all.iter().map(|p| (p.id, (p.parent, p.kind))).collect();
        let in_set = |mut parent: Option<i64>| {
            while let Some(id) = parent {
                match kind.get(&id) {
                    Some((_, PlaylistKind::Set)) => return true,
                    Some((up, _)) => parent = *up,
                    None => return false,
                }
            }
            false
        };
        Ok(all
            .iter()
            .filter(|p| p.kind == PlaylistKind::List && !in_set(p.parent))
            .map(|p| (p.id, p.name.clone()))
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
    fn a_set_gets_its_lists_and_a_live_set() {
        let (mut lib, _) = library();
        let lists: Vec<String> = ["Staging", "Set", " ", "Openers"]
            .map(String::from)
            .to_vec();
        let set = lib.create_set("Fri 31 Oct", &lists).unwrap();
        let tree = lib.playlists().unwrap();
        let names: Vec<(&str, PlaylistKind)> = tree
            .iter()
            .filter(|p| p.kind != PlaylistKind::Smart)
            .map(|p| (p.name.as_str(), p.kind))
            .collect();
        assert_eq!(
            names,
            [
                ("Fri 31 Oct", PlaylistKind::Set),
                ("Staging", PlaylistKind::List),
                ("Set", PlaylistKind::List),
                ("Openers", PlaylistKind::List),
            ]
        );
        assert_eq!(lib.sets().unwrap(), [(set, "Fri 31 Oct".to_string())]);
        // No Live set in this template: one is made, and found after.
        let live = lib.live_set(set).unwrap();
        assert_eq!(lib.live_set(set).unwrap(), live);
        assert_eq!(lib.playlists().unwrap().len(), tree.len() + 1);
    }

    #[test]
    fn crates_are_lists_outside_sets() {
        let (mut lib, ids) = library();
        let yoga = lib
            .create_playlist("Yoga", None, PlaylistKind::List)
            .unwrap();
        let folder = lib
            .create_playlist("Styles", None, PlaylistKind::Folder)
            .unwrap();
        let dance = lib
            .create_playlist("Dance", Some(folder), PlaylistKind::List)
            .unwrap();
        let set = lib.create_set("Gig", &TEMPLATE.map(String::from)).unwrap();
        let live = lib.live_set(set).unwrap();
        lib.add_to_playlist(yoga, &[ids[0], ids[1]]).unwrap();
        lib.add_to_playlist(dance, &[ids[0], ids[0]]).unwrap();
        lib.add_to_playlist(live, &[ids[2]]).unwrap();
        let crates = lib.crates_by_track().unwrap();
        assert_eq!(crates[&ids[0]], ["Dance", "Yoga"]);
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
        lib.create_set("Gig", &TEMPLATE.map(String::from)).unwrap();
        lib.add_to_playlist(yoga, &[ids[0], ids[1]]).unwrap();
        lib.remove_tracks(&[ids[1]]).unwrap();
        let health = lib.list_health().unwrap();
        // The set's lists are empty too, but a new set is meant to be.
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
