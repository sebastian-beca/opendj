//! Library Health: checks that find what needs fixing in the collection
//! (audiobooks and podcasts, files outside the stage folders, tracks not
//! analysed or missing their tags, cover or rating, files gone, long
//! recordings), so it can be fixed in batches. Each check is also a
//! search (`is:notmusic`, `is:stray`…), and a track can be set to be
//! ignored by a check, so it stops showing up there.
//!
//! What the DJ decided is kept in the library: tracks ignored per check,
//! tracks taken out of the library (their files stay, and an import or a
//! scan doesn't bring them back), and which files have a cover.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use mixxx_engine::{Source, Track};
use rusqlite::{params, OptionalExtension};

use crate::{now, row_id, track_id, Error, Library};

/// Tracks longer than this (in seconds) are recordings of whole sets or
/// long ambient pieces, rather than tracks to mix.
pub const LONG_TRACK: f64 = 20.0 * 60.0;

/// Tracks in one folder sharing a title, from which they are taken to be
/// an audiobook's chapters.
const CHAPTERS: usize = 4;

pub(crate) const HEALTH_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS health_ignored (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    check_name TEXT NOT NULL,
    PRIMARY KEY (track_id, check_name)
);
CREATE TABLE IF NOT EXISTS removed (
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    removed INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS covers (
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    file_mtime INTEGER NOT NULL,
    has_cover INTEGER NOT NULL
);
";

/// Something Library Health looks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Check {
    NotMusic,
    Stray,
    NotAnalyzed,
    MissingInfo,
    NoCover,
    NoRating,
    Missing,
    Long,
    Duplicate,
}

impl Check {
    pub const ALL: [Check; 9] = [
        Check::NotMusic,
        Check::Stray,
        Check::Duplicate,
        Check::NotAnalyzed,
        Check::MissingInfo,
        Check::NoCover,
        Check::NoRating,
        Check::Missing,
        Check::Long,
    ];

    /// The name kept in the library and used in searches (`is:notmusic`).
    pub fn name(self) -> &'static str {
        match self {
            Check::NotMusic => "notmusic",
            Check::Stray => "stray",
            Check::NotAnalyzed => "unanalyzed",
            Check::MissingInfo => "noinfo",
            Check::NoCover => "nocover",
            Check::NoRating => "unrated",
            Check::Missing => "missing",
            Check::Long => "long",
            Check::Duplicate => "duplicate",
        }
    }

    /// The check a search word names, with a few other spellings.
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "notmusic" | "nonmusic" | "audiobook" | "podcast" => Check::NotMusic,
            "stray" | "strays" => Check::Stray,
            "unanalyzed" | "unanalysed" | "notanalyzed" => Check::NotAnalyzed,
            "noinfo" | "untagged" | "incomplete" => Check::MissingInfo,
            "nocover" | "noart" => Check::NoCover,
            "unrated" => Check::NoRating,
            "missing" => Check::Missing,
            "long" => Check::Long,
            "duplicate" | "dupe" | "duplicates" => Check::Duplicate,
            _ => return None,
        })
    }

    /// What the Health page calls it.
    pub fn title(self) -> &'static str {
        match self {
            Check::NotMusic => "Not music",
            Check::Stray => "Strays",
            Check::NotAnalyzed => "Not analyzed",
            Check::MissingInfo => "Missing info",
            Check::NoCover => "No cover",
            Check::NoRating => "No rating",
            Check::Missing => "Missing files",
            Check::Long => "Long tracks",
            Check::Duplicate => "Duplicates",
        }
    }

    /// One line on what it finds.
    pub fn about(self) -> &'static str {
        match self {
            Check::NotMusic => {
                "Audiobooks, podcasts and spoken word that came in with another library."
            }
            Check::Stray => "Files outside your Inbox, Triage and Collection folders.",
            Check::NotAnalyzed => "No tempo or key yet.",
            Check::MissingInfo => "No artist, title, album, year or genre.",
            Check::NoCover => "No picture in the file's tags or its folder.",
            Check::NoRating => "No stars yet.",
            Check::Missing => "The file isn't where the library expects it.",
            Check::Long => "Over 20 minutes: recorded sets and long pieces.",
            Check::Duplicate => "The same track more than once.",
        }
    }

    /// Whether a track this check finds counts against the other checks:
    /// an audiobook isn't also "missing a cover", and its chapters, which
    /// share a title, aren't duplicates.
    fn needs_music(self) -> bool {
        !matches!(self, Check::NotMusic | Check::Missing)
    }
}

/// Whether a track looks like it isn't music, from where it is and its tags
/// alone (chapters are found across tracks, in [`Facts::new`]).
pub fn looks_like_not_music(t: &Track) -> bool {
    let genre = t.genre.trim().to_lowercase();
    if [
        "audiobook",
        "audiobooks",
        "audio book",
        "hörbuch",
        "hörbücher",
        "podcast",
        "podcasts",
        "speech",
        "spoken word",
        "spoken",
        "books & spoken",
        "books and spoken",
    ]
    .contains(&genre.as_str())
    {
        return true;
    }
    if t.album.to_lowercase().contains("abridged") {
        return true;
    }
    let title = t.title.trim().to_lowercase();
    if ["chapter ", "kapitel ", "capítulo ", "chapitre "]
        .iter()
        .any(|w| title.starts_with(w))
    {
        return true;
    }
    let Source::File(path) = &t.source else {
        return false;
    };
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if ["m4b", "aax", "aa"].contains(&ext.as_str()) {
        return true;
    }
    let p = path.to_string_lossy().to_lowercase();
    [
        "com.apple.bkagentservice",
        "com.apple.podcasts",
        "/podcasts/",
        "/audiobooks/",
        "/audible/",
        "/movies/",
        "/voice memos/",
    ]
    .iter()
    .any(|w| p.contains(w))
}

/// What the checks go by beyond each track: the DJ's choices, what was
/// found across tracks, and which files have a cover.
#[derive(Debug, Default)]
pub struct Facts {
    /// Ids of tracks that look like they aren't music.
    not_music: HashSet<usize>,
    /// Ids of tracks each check is told to leave alone.
    ignored: HashMap<Check, HashSet<usize>>,
    /// Whether each file has a cover, for those looked at.
    covers: HashMap<usize, bool>,
    /// The DJ has set up the stage folders, so a track outside them is a stray.
    stages: bool,
}

impl Facts {
    pub fn new<'a>(
        tracks: impl IntoIterator<Item = &'a Track>,
        ignored: HashMap<Check, HashSet<usize>>,
        covers: HashMap<usize, bool>,
        stages: bool,
    ) -> Self {
        let mut not_music = HashSet::new();
        // Chapters: several tracks in one folder sharing a title.
        let mut titles: HashMap<(PathBuf, String), Vec<usize>> = HashMap::new();
        for t in tracks {
            if looks_like_not_music(t) {
                not_music.insert(t.id);
            }
            if let Source::File(path) = &t.source {
                let title = t.title.trim().to_lowercase();
                if !title.is_empty() {
                    let dir = path.parent().unwrap_or(Path::new("")).to_path_buf();
                    titles.entry((dir, title)).or_default().push(t.id);
                }
            }
        }
        for ids in titles.into_values().filter(|ids| ids.len() >= CHAPTERS) {
            not_music.extend(ids);
        }
        Facts {
            not_music,
            ignored,
            covers,
            stages,
        }
    }

    /// Whether `check` finds `t`. `duplicates` are the ids of tracks that
    /// are in the library more than once.
    pub fn finds(&self, check: Check, t: &Track, duplicates: &HashSet<usize>) -> bool {
        if self.ignored.get(&check).is_some_and(|i| i.contains(&t.id)) {
            return false;
        }
        if check.needs_music() && (t.info.missing || self.not_music(t)) {
            return false;
        }
        match check {
            Check::NotMusic => self.not_music(t),
            Check::Stray => self.stages && t.info.stage.is_empty(),
            Check::NotAnalyzed => t.bpm <= 0.0 || t.key.is_empty(),
            Check::MissingInfo => [&t.artist, &t.title, &t.album, &t.info.year, &t.genre]
                .iter()
                .any(|f| f.trim().is_empty()),
            Check::NoCover => self.covers.get(&t.id) == Some(&false),
            Check::NoRating => t.rating == 0,
            Check::Missing => t.info.missing,
            Check::Long => t.duration > LONG_TRACK,
            Check::Duplicate => duplicates.contains(&t.id),
        }
    }

    /// Whether `t` looks like it isn't music and the DJ hasn't said it is.
    fn not_music(&self, t: &Track) -> bool {
        self.not_music.contains(&t.id)
            && !self
                .ignored
                .get(&Check::NotMusic)
                .is_some_and(|i| i.contains(&t.id))
    }

    /// Whether `t`'s file has been looked at for a cover.
    pub fn cover_known(&self, id: usize) -> bool {
        self.covers.contains_key(&id)
    }

    /// Notes whether a track has a cover.
    pub fn set_cover(&mut self, id: usize, has: bool) {
        self.covers.insert(id, has);
    }

    /// Tells `check` to leave these tracks alone, or (with `on` false) to
    /// look at them again.
    pub fn set_ignored(&mut self, check: Check, ids: &[usize], on: bool) {
        let set = self.ignored.entry(check).or_default();
        for id in ids {
            if on {
                set.insert(*id);
            } else {
                set.remove(id);
            }
        }
    }
}

/// The artist and title in a file name such as "01 - Artist - Title
/// (Remix).mp3": a leading track number is left out, and what comes
/// before the first " - " is the artist. `None` when there is no " - ".
pub fn from_file_name(path: &Path) -> Option<(String, String)> {
    let stem = path.file_stem()?.to_string_lossy().replace('_', " ");
    let mut rest = stem.trim();
    // "01 ", "01. ", "01 - ", "1-02 "
    let digits = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '-'))
        .unwrap_or(0);
    // A number that starts a name ("808 State") isn't a track number: those
    // have a leading zero, two digits at most, or a "." or " - " after them.
    let marked = rest[digits..].starts_with(['.', ')']) || rest[digits..].starts_with(" - ");
    let number = &rest[..digits];
    if digits > 0 && digits <= 5 && (marked || number.starts_with('0') || digits <= 2) {
        let after = rest[digits..].trim_start_matches(['.', ' ', ')']);
        let after = after.strip_prefix("- ").unwrap_or(after);
        if after.len() < rest.len() && !after.is_empty() {
            rest = after.trim();
        }
    }
    let (artist, title) = rest.split_once(" - ")?;
    let (artist, title) = (artist.trim(), title.trim());
    (!artist.is_empty() && !title.is_empty()).then(|| (artist.to_string(), title.to_string()))
}

impl Library {
    /// The tracks each check is told to leave alone.
    pub fn health_ignored(&self) -> Result<HashMap<Check, HashSet<usize>>, Error> {
        let mut out: HashMap<Check, HashSet<usize>> = HashMap::new();
        let mut q = self
            .db
            .prepare("SELECT track_id, check_name FROM health_ignored")?;
        let rows = q.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (id, name) = row?;
            if let Some(check) = Check::from_name(&name) {
                out.entry(check).or_default().insert(track_id(id));
            }
        }
        Ok(out)
    }

    /// Tells `check` to leave these tracks alone, or (with `on` false) to
    /// look at them again.
    pub fn set_health_ignored(
        &mut self,
        check: Check,
        ids: &[usize],
        on: bool,
    ) -> Result<(), Error> {
        let tx = self.db.transaction()?;
        {
            let mut add = tx.prepare(
                "INSERT OR IGNORE INTO health_ignored (track_id, check_name) VALUES (?1, ?2)",
            )?;
            let mut take =
                tx.prepare("DELETE FROM health_ignored WHERE track_id = ?1 AND check_name = ?2")?;
            for row in ids.iter().filter_map(|&id| row_id(id)) {
                if on {
                    add.execute(params![row, check.name()])?;
                } else {
                    take.execute(params![row, check.name()])?;
                }
            }
        }
        tx.commit()
    }

    /// Takes tracks out of the library. Their files stay where they are,
    /// and their rows are kept (so lists and history still know them),
    /// but they no longer show, and a scan or an import doesn't bring them
    /// back; adding the file again does.
    pub fn remove_tracks(&mut self, ids: &[usize]) -> Result<usize, Error> {
        let tx = self.db.transaction()?;
        let mut n = 0;
        {
            let mut add =
                tx.prepare("INSERT OR IGNORE INTO removed (track_id, removed) VALUES (?1, ?2)")?;
            let at = now();
            for row in ids.iter().filter_map(|&id| row_id(id)) {
                n += add.execute(params![row, at])?;
            }
        }
        tx.commit()?;
        Ok(n)
    }

    /// Brings back tracks taken out with [`Library::remove_tracks`]: these,
    /// or all of them when `ids` is `None`. Returns how many came back.
    pub fn restore_tracks(&mut self, ids: Option<&[usize]>) -> Result<usize, Error> {
        match ids {
            None => Ok(self.db.execute("DELETE FROM removed", [])?),
            Some(ids) => {
                let tx = self.db.transaction()?;
                let mut n = 0;
                for row in ids.iter().filter_map(|&id| row_id(id)) {
                    n += tx.execute("DELETE FROM removed WHERE track_id = ?1", [row])?;
                }
                tx.commit()?;
                Ok(n)
            }
        }
    }

    /// How many tracks have been taken out of the library.
    pub fn removed_count(&self) -> Result<usize, Error> {
        self.db
            .query_row("SELECT COUNT(*) FROM removed", [], |r| r.get::<_, i64>(0))
            .map(|n| n as usize)
    }

    /// Whether each track's file has a cover, as last looked at, with the
    /// file's modification time then (to look again once it changes).
    pub fn covers(&self) -> Result<HashMap<usize, (i64, bool)>, Error> {
        let mut q = self
            .db
            .prepare("SELECT track_id, file_mtime, has_cover FROM covers")?;
        let rows = q.query_map([], |r| {
            Ok((
                track_id(r.get(0)?),
                (r.get::<_, i64>(1)?, r.get::<_, bool>(2)?),
            ))
        })?;
        rows.collect()
    }

    /// Notes whether tracks' files have a cover: id, the file's
    /// modification time and the answer.
    pub fn save_covers(&mut self, found: &[(usize, i64, bool)]) -> Result<(), Error> {
        let tx = self.db.transaction()?;
        {
            let mut put = tx.prepare(
                "INSERT INTO covers (track_id, file_mtime, has_cover) VALUES (?1, ?2, ?3)
                 ON CONFLICT(track_id) DO UPDATE SET file_mtime = ?2, has_cover = ?3",
            )?;
            for (id, mtime, has) in found {
                if let Some(row) = row_id(*id) {
                    put.execute(params![row, mtime, has])?;
                }
            }
        }
        tx.commit()
    }

    /// Points a track at its file's new place (a missing file found
    /// elsewhere). A scan may have added the file there as a track of its
    /// own already: that one gives way if nothing was done with it yet (no
    /// lists, cues, plays or rating), so the track keeps its history; else
    /// it is left to the duplicates check and this fails. The track's
    /// stage is cleared, for [`Library::assign_stages`] to give it the one
    /// of its new folder.
    pub fn relink(&mut self, id: usize, path: &Path) -> Result<(), Error> {
        let Some(row) = row_id(id) else { return Ok(()) };
        let p = crate::path_text(path);
        let tx = self.db.transaction()?;
        let other: Option<(i64, bool)> = tx
            .query_row(
                "SELECT id,
                        EXISTS (SELECT 1 FROM playlist_tracks WHERE track_id = tracks.id)
                        OR EXISTS (SELECT 1 FROM cues WHERE track_id = tracks.id)
                        OR play_count > 0 OR rating > 0
                 FROM tracks WHERE path = ?1",
                [&p],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((other, used)) = other {
            if used {
                return Err(Error::QueryReturnedNoRows);
            }
            tx.execute("DELETE FROM tracks WHERE id = ?1", [other])?;
        }
        tx.execute(
            "UPDATE tracks SET path = ?2, file_mtime = 0, stage = '' WHERE id = ?1",
            params![row, p],
        )?;
        tx.commit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::file_track;

    fn track(id: usize, path: &str, title: &str) -> Track {
        let mut t = file_track(path, title);
        t.id = id;
        t
    }

    #[test]
    fn finds_audiobooks_by_place_genre_and_chapters() {
        let book = track(
            1,
            "/Users/s/Library/Containers/com.apple.BKAgentService/b.mp3",
            "Breath",
        );
        assert!(looks_like_not_music(&book));
        let mut genre = track(2, "/m/a.mp3", "Talk");
        genre.genre = "Audiobook".into();
        assert!(looks_like_not_music(&genre));
        let m4b = track(3, "/m/book.m4b", "Book");
        assert!(looks_like_not_music(&m4b));
        let song = track(4, "/m/DB/Library/x.mp3", "Song");
        assert!(!looks_like_not_music(&song));

        let mut tracks: Vec<Track> = (0..5)
            .map(|i| track(10 + i, &format!("/m/Breath/{i:02}.mp3"), "Breath"))
            .collect();
        tracks.push(song.clone());
        let facts = Facts::new(&tracks, HashMap::new(), HashMap::new(), true);
        let none = HashSet::new();
        assert!(facts.finds(Check::NotMusic, &tracks[0], &none));
        assert!(!facts.finds(Check::NotMusic, &song, &none));
        // A chapter isn't also missing its rating.
        assert!(!facts.finds(Check::NoRating, &tracks[0], &none));
        assert!(facts.finds(Check::NoRating, &song, &none));
    }

    #[test]
    fn ignoring_takes_a_track_off_a_check() {
        let mut t = track(1, "/m/DB/Library/x.mp3", "Song");
        t.genre = "Podcast".into();
        let mut facts = Facts::new([&t], HashMap::new(), HashMap::new(), true);
        let none = HashSet::new();
        assert!(facts.finds(Check::NotMusic, &t, &none));
        assert!(!facts.finds(Check::NoRating, &t, &none));
        // Marked as music: no longer found, and the other checks look at it.
        facts.set_ignored(Check::NotMusic, &[1], true);
        assert!(!facts.finds(Check::NotMusic, &t, &none));
        assert!(facts.finds(Check::NoRating, &t, &none));
        facts.set_ignored(Check::NoRating, &[1], true);
        assert!(!facts.finds(Check::NoRating, &t, &none));
    }

    #[test]
    fn strays_and_covers() {
        let mut t = track(1, "/Users/s/Downloads/x.mp3", "Song");
        let none = HashSet::new();
        let facts = Facts::new([&t], HashMap::new(), HashMap::new(), false);
        assert!(
            !facts.finds(Check::Stray, &t, &none),
            "no stage folders yet"
        );
        let mut facts = Facts::new([&t], HashMap::new(), HashMap::new(), true);
        assert!(facts.finds(Check::Stray, &t, &none));
        t.info.stage = "inbox".into();
        assert!(!facts.finds(Check::Stray, &t, &none));
        assert!(!facts.finds(Check::NoCover, &t, &none), "not looked at yet");
        facts.set_cover(1, false);
        assert!(facts.finds(Check::NoCover, &t, &none));
    }

    #[test]
    fn reads_artist_and_title_from_file_names() {
        let f = |s: &str| from_file_name(Path::new(s));
        assert_eq!(
            f("/m/Desert Dwellers - Bodhi Mandala (Drumspyder Remix).mp3"),
            Some((
                "Desert Dwellers".into(),
                "Bodhi Mandala (Drumspyder Remix)".into()
            ))
        );
        assert_eq!(
            f("/m/01 - Bicep - Glue.flac"),
            Some(("Bicep".into(), "Glue".into()))
        );
        assert_eq!(
            f("/m/02. Lane_8 - Fingerprint.mp3"),
            Some(("Lane 8".into(), "Fingerprint".into()))
        );
        assert_eq!(f("/m/track 7.mp3"), None);
        assert_eq!(
            f("/m/808 State - Pacific.mp3").map(|a| a.0),
            Some("808 State".into())
        );
    }

    #[test]
    fn removed_tracks_stay_out_until_restored() {
        let mut lib = Library::in_memory().unwrap();
        let added = lib
            .add_files(&[file_track("/m/a.mp3", "One"), file_track("/m/b.mp3", "Two")])
            .unwrap();
        let ids: Vec<usize> = added.iter().map(|t| t.id).collect();
        assert_eq!(lib.remove_tracks(&ids[..1]).unwrap(), 1);
        let left = lib.all_tracks(|_| true).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].title, "Two");
        assert_eq!(lib.removed_count().unwrap(), 1);
        // A scan finds the file again, but it stays out.
        lib.sync(&[file_track("/m/a.mp3", "One")]).unwrap();
        assert_eq!(lib.all_tracks(|_| true).unwrap().len(), 1);
        // Adding the file by hand brings it back.
        lib.add_files(&[file_track("/m/a.mp3", "One")]).unwrap();
        assert_eq!(lib.all_tracks(|_| true).unwrap().len(), 2);
        lib.remove_tracks(&ids).unwrap();
        assert_eq!(lib.restore_tracks(None).unwrap(), 2);
        assert_eq!(lib.all_tracks(|_| true).unwrap().len(), 2);
    }

    #[test]
    fn relinking_takes_over_a_fresh_scan_of_the_moved_file() {
        let mut lib = Library::in_memory().unwrap();
        let old = lib.add_file(&file_track("/m/old/a.mp3", "One")).unwrap();
        lib.set_rating(old.id, 4).unwrap();
        // The file moved, and a scan found it at its new place.
        lib.sync(&[file_track("/m/new/a.mp3", "One")]).unwrap();
        assert_eq!(lib.all_tracks(|_| true).unwrap().len(), 2);
        lib.relink(old.id, Path::new("/m/new/a.mp3")).unwrap();
        let left = lib.all_tracks(|_| true).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!((left[0].id, left[0].rating), (old.id, 4));
        // A track at the new place with ratings of its own stays.
        let other = lib.add_file(&file_track("/m/b.mp3", "Two")).unwrap();
        let moved = lib.add_file(&file_track("/m/c.mp3", "Two")).unwrap();
        lib.set_rating(moved.id, 3).unwrap();
        assert!(lib.relink(other.id, Path::new("/m/c.mp3")).is_err());
    }

    #[test]
    fn keeps_ignores_and_covers() {
        let mut lib = Library::in_memory().unwrap();
        let t = lib.add_file(&file_track("/m/a.mp3", "One")).unwrap();
        lib.set_health_ignored(Check::NoCover, &[t.id], true)
            .unwrap();
        let ignored = lib.health_ignored().unwrap();
        assert!(ignored[&Check::NoCover].contains(&t.id));
        lib.set_health_ignored(Check::NoCover, &[t.id], false)
            .unwrap();
        assert!(lib.health_ignored().unwrap().is_empty());
        lib.save_covers(&[(t.id, 42, true)]).unwrap();
        assert_eq!(lib.covers().unwrap()[&t.id], (42, true));
    }
}
