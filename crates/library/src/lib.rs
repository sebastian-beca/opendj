//! The saved music library, as Mixxx keeps `mixxxdb.sqlite`: every track
//! with its tags, rating, colour and comment, its hot cues and beat grid,
//! the DJ's playlists and folders, and a history of what was played.
//!
//! The database is SQLite (`library.sqlite` in the settings folder), opened
//! in WAL mode so the background scan and the UI can both use it. It is the
//! source of truth once a track is in it:
//!
//! - A folder scan adds new files, and re-reads a file's tags only when the
//!   file has changed since.
//! - Analysis results are stored once, so a track loads with its grid at once.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use mixxx_engine::beats::BeatGrid;
use mixxx_engine::import::{ImportedLibrary, ImportedList, ImportedListKind};
use mixxx_engine::{Hotcue, LibraryInfo, Marker, MarkerKind, Source, Track};
use rusqlite::{params, Connection, OptionalExtension};

pub use rusqlite::Error;

pub mod query;

/// Library track ids start here, clear of the built-in sample tracks.
pub const ID_BASE: usize = 1_000_000;

/// The schema version this code writes (`PRAGMA user_version`).
/// 2 added smart lists: `playlists.query`, kind 3.
/// 3 added purchases: `tracks.store` and `store_url`, `wanted`, `inbox_log`.
/// 4 added saved loops: `cues.end_seconds`.
/// 5 added `tracks.key_source`.
/// 6 added the track info panel's fields: `tracks.remix`, `year`,
/// `track_number`, `user1`, `user2` and `gain`.
/// 7 added `tracks.disc_number`, `label`, `composer`, `remixer` and `isrc`,
/// `markers`, and `playlists.source` for lists brought in from another app.
const VERSION: i32 = 7;

/// The smart lists a new library starts with: name and search.
const STARTER_SMART_LISTS: [(&str, &str); 6] = [
    ("Mixes with deck", "~deck"),
    ("Recently added", "added:<30d"),
    ("Top rated", "rating:>=4"),
    ("Never played", "played:0"),
    ("Duplicates", "is:duplicate"),
    ("Missing files", "is:missing"),
];

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS tracks (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    artist TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL DEFAULT '',
    album TEXT NOT NULL DEFAULT '',
    genre TEXT NOT NULL DEFAULT '',
    duration REAL NOT NULL DEFAULT 0,
    bpm REAL NOT NULL DEFAULT 0,
    key TEXT NOT NULL DEFAULT '',
    rating INTEGER NOT NULL DEFAULT 0,
    color INTEGER,
    comment TEXT NOT NULL DEFAULT '',
    grouping TEXT NOT NULL DEFAULT '',
    labels TEXT NOT NULL DEFAULT '',
    play_count INTEGER NOT NULL DEFAULT 0,
    last_played INTEGER,
    -- Beat grid as BeatGrid::to_text, and where it came from:
    -- 'analysis', 'user', or another app's name for an imported grid.
    grid TEXT,
    grid_source TEXT,
    -- The file's modification time when its tags were last read.
    file_mtime INTEGER NOT NULL DEFAULT 0,
    -- 'scan', 'file' (dropped in), 'purchase' (from the inbox), or another
    -- app's name for imported tracks, whose tags a scan leaves alone.
    source TEXT NOT NULL DEFAULT 'scan',
    added INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS cues (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    number INTEGER NOT NULL,
    seconds REAL NOT NULL,
    name TEXT NOT NULL DEFAULT '',
    color INTEGER,
    -- Where a saved loop ends; NULL for a plain cue.
    end_seconds REAL,
    -- 'user', or another app's name for imported cues.
    source TEXT NOT NULL,
    PRIMARY KEY (track_id, number)
);
-- Lists and folders in one tree. kind: 0 list, 1 folder, 2 history session,
-- 3 smart list (its tracks are those its search finds, in `query`).
CREATE TABLE IF NOT EXISTS playlists (
    id INTEGER PRIMARY KEY,
    parent INTEGER REFERENCES playlists(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    kind INTEGER NOT NULL DEFAULT 0,
    position INTEGER NOT NULL DEFAULT 0,
    created INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS playlist_tracks (
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    played_at INTEGER,
    PRIMARY KEY (playlist_id, position)
);
CREATE INDEX IF NOT EXISTS playlist_tracks_track ON playlist_tracks(track_id);
-- Named sections, phrases and mix points. kind: 'section', 'phrase' or
-- 'automix'; source as for cues.
CREATE TABLE IF NOT EXISTS markers (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    seconds REAL NOT NULL,
    end_seconds REAL,
    name TEXT NOT NULL DEFAULT '',
    color INTEGER,
    source TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS markers_track ON markers(track_id);
-- Tracks the DJ went to buy (clicked Buy on a store result), until a file
-- that matches arrives in the purchase inbox.
CREATE TABLE IF NOT EXISTS wanted (
    id INTEGER PRIMARY KEY,
    store TEXT NOT NULL,
    artist TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL DEFAULT '',
    page_url TEXT NOT NULL UNIQUE,
    added INTEGER NOT NULL DEFAULT 0,
    track_id INTEGER REFERENCES tracks(id) ON DELETE SET NULL
);
-- Every file the purchase inbox has dealt with, so none is taken twice.
-- outcome: 'imported', 'duplicate' or 'failed: <why>'.
CREATE TABLE IF NOT EXISTS inbox_log (
    path TEXT PRIMARY KEY,
    outcome TEXT NOT NULL,
    track_id INTEGER,
    at INTEGER NOT NULL
);
";

/// Columns added to `tracks` after version 1: name and definition.
const ADDED_COLUMNS: [(&str, &str); 14] = [
    ("store", "store TEXT NOT NULL DEFAULT ''"),
    ("store_url", "store_url TEXT"),
    // Where the key came from: 'analysis', 'tags', or another app's name;
    // NULL when not known (keys from before version 4).
    ("key_source", "key_source TEXT"),
    ("remix", "remix TEXT NOT NULL DEFAULT ''"),
    ("year", "year TEXT NOT NULL DEFAULT ''"),
    ("track_number", "track_number INTEGER NOT NULL DEFAULT 0"),
    // VirtualDJ's User 1 and User 2: the DJ's own tags. `labels` holds
    // their words together, for searching.
    ("user1", "user1 TEXT NOT NULL DEFAULT ''"),
    ("user2", "user2 TEXT NOT NULL DEFAULT ''"),
    // The DJ's gain for the track in dB; NULL for none.
    ("gain", "gain REAL"),
    // What other DJ apps keep beyond these, so nothing they hold is lost
    // on the way in (version 7).
    ("disc_number", "disc_number INTEGER NOT NULL DEFAULT 0"),
    ("label", "label TEXT NOT NULL DEFAULT ''"),
    ("composer", "composer TEXT NOT NULL DEFAULT ''"),
    ("remixer", "remixer TEXT NOT NULL DEFAULT ''"),
    ("isrc", "isrc TEXT NOT NULL DEFAULT ''"),
];

/// Tracks the DJ linked as going well together: `track` links to `linked`. Created on every open rather than by
/// a version bump, so it doesn't clash with other schema changes.
const LINKS_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS track_links (
    track INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    linked INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    added INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (track, linked)
);
CREATE INDEX IF NOT EXISTS track_links_linked ON track_links(linked);
";

/// Stores the tags a scan reads beyond the basic ones (`?1` is the row),
/// for tracks whose tags come from their files. A rating in the tags fills
/// in one the library doesn't have.
const TAG_EXTRAS: &str = "
UPDATE tracks SET remix = ?2, year = ?3, track_number = ?4, grouping = ?5, comment = ?6,
       rating = CASE WHEN ?7 > 0 THEN ?7 ELSE rating END
WHERE id = ?1 AND source IN ('scan', 'file', 'purchase')";

fn tag_extras(row: i64, t: &Track) -> impl rusqlite::Params + '_ {
    (
        row,
        t.info.remix.as_str(),
        t.info.year.as_str(),
        t.info.track_number as i64,
        t.info.grouping.as_str(),
        t.info.comment.as_str(),
        t.rating as i64,
    )
}

/// A list, folder or history session.
#[derive(Clone, Debug, PartialEq)]
pub struct Playlist {
    pub id: i64,
    pub parent: Option<i64>,
    pub name: String,
    pub kind: PlaylistKind,
    /// A smart list's search (see [`query`]).
    pub query: Option<String>,
    /// When it was made, or for a history session when it started (Unix
    /// time).
    pub created: i64,
    /// The app it was brought in from, for lists an import keeps up to date;
    /// None for the DJ's own.
    pub source: Option<String>,
}

/// A track the DJ went to buy and hasn't received yet.
#[derive(Clone, Debug, PartialEq)]
pub struct Wanted {
    pub id: i64,
    pub store: String,
    pub artist: String,
    pub title: String,
    pub page_url: String,
}

/// Lowercase letters and digits only, for comparing names loosely.
fn simplify(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaylistKind {
    List,
    Folder,
    /// The tracks played in one session, oldest first.
    History,
    /// A saved search.
    Smart,
}

impl PlaylistKind {
    fn from_i64(k: i64) -> Self {
        match k {
            1 => Self::Folder,
            2 => Self::History,
            3 => Self::Smart,
            _ => Self::List,
        }
    }

    fn to_i64(self) -> i64 {
        match self {
            Self::List => 0,
            Self::Folder => 1,
            Self::History => 2,
            Self::Smart => 3,
        }
    }
}

/// What a sync changed.
#[derive(Debug, Default, PartialEq)]
pub struct SyncReport {
    pub added: usize,
    pub updated: usize,
    /// Lists, folders and history sessions brought in.
    pub lists: usize,
    /// List entries left out because the library doesn't have their file
    /// (streamed tracks, files that have gone).
    pub unmatched: usize,
}

pub struct Library {
    db: Connection,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn mtime(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64)
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The database id of a library track, or None for a track from elsewhere
/// (the built-in samples).
pub fn row_id(track_id: usize) -> Option<i64> {
    track_id.checked_sub(ID_BASE).map(|i| i as i64)
}

fn track_id(row: i64) -> usize {
    ID_BASE + row as usize
}

/// Where the library lives by default: `library.sqlite` in `dir`.
pub fn default_path(dir: &Path) -> PathBuf {
    dir.join("library.sqlite")
}

impl Library {
    /// Opens (creating if needed) the library at `path`.
    pub fn open(path: &Path) -> Result<Self, Error> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        Self::setup(Connection::open(path)?)
    }

    /// A library that lives only in memory, for tests.
    pub fn in_memory() -> Result<Self, Error> {
        Self::setup(Connection::open_in_memory()?)
    }

    fn setup(db: Connection) -> Result<Self, Error> {
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        // In-memory databases stay in "memory" journal mode; that is fine.
        let _: String = db.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
        db.execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = NORMAL;")?;
        let version: i32 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < VERSION {
            db.execute_batch(SCHEMA)?;
            let has_query = db
                .prepare("SELECT 1 FROM pragma_table_info('playlists') WHERE name = 'query'")?
                .exists([])?;
            if !has_query {
                db.execute_batch("ALTER TABLE playlists ADD COLUMN query TEXT")?;
            }
            let has_loop_ends = db
                .prepare("SELECT 1 FROM pragma_table_info('cues') WHERE name = 'end_seconds'")?
                .exists([])?;
            if !has_loop_ends {
                db.execute_batch("ALTER TABLE cues ADD COLUMN end_seconds REAL")?;
            }
            // The app a list was brought in from; NULL for the DJ's own.
            let has_list_source = db
                .prepare("SELECT 1 FROM pragma_table_info('playlists') WHERE name = 'source'")?
                .exists([])?;
            if !has_list_source {
                db.execute_batch("ALTER TABLE playlists ADD COLUMN source TEXT")?;
            }
            for (name, definition) in ADDED_COLUMNS {
                let has = db
                    .prepare("SELECT 1 FROM pragma_table_info('tracks') WHERE name = ?1")?
                    .exists([name])?;
                if !has {
                    db.execute_batch(&format!("ALTER TABLE tracks ADD COLUMN {definition}"))?;
                }
            }
            if version < 5 {
                // Keys brought in from another app came from there.
                db.execute_batch(
                    "UPDATE tracks SET key_source = source
                     WHERE key != '' AND key_source IS NULL
                       AND source NOT IN ('scan', 'file', 'purchase')",
                )?;
            }
            db.execute_batch(&format!("PRAGMA user_version = {VERSION}"))?;
        }
        db.execute_batch(LINKS_SCHEMA)?;
        let lib = Library { db };
        if version < 2 {
            lib.add_starter_smart_lists()?;
        }
        Ok(lib)
    }

    /// A few useful smart lists, so the DJ sees what they can do. They sit
    /// at the top level, which the browser shows under "Smart Lists".
    fn add_starter_smart_lists(&self) -> Result<(), Error> {
        for (name, query) in STARTER_SMART_LISTS {
            self.create_smart_list(name, None, query)?;
        }
        Ok(())
    }

    fn meta(&self, key: &str) -> Option<String> {
        self.db
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    }

    fn set_meta(&self, key: &str, value: &str) -> Result<(), Error> {
        self.db.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Every track whose file `exists`, with its cues and grid, in id order.
    pub fn tracks(&self, exists: impl Fn(&Path) -> bool) -> Result<Vec<Track>, Error> {
        let mut all = self.all_tracks(exists)?;
        all.retain(|t| !t.info.missing);
        Ok(all)
    }

    /// Every track, with those whose file doesn't `exist` marked missing.
    pub fn all_tracks(&self, exists: impl Fn(&Path) -> bool) -> Result<Vec<Track>, Error> {
        let mut cues: HashMap<i64, Vec<Hotcue>> = HashMap::new();
        {
            let mut q = self.db.prepare(
                "SELECT track_id, number, seconds, name, end_seconds, color FROM cues ORDER BY number",
            )?;
            let rows = q.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    Hotcue {
                        number: r.get::<_, i64>(1)? as usize,
                        seconds: r.get(2)?,
                        name: r.get(3)?,
                        end: r.get(4)?,
                        color: r.get::<_, Option<i64>>(5)?.map(|c| c as u32),
                    },
                ))
            })?;
            for row in rows {
                let (id, cue) = row?;
                cues.entry(id).or_default().push(cue);
            }
        }
        let mut markers: HashMap<i64, Vec<Marker>> = HashMap::new();
        {
            let mut q = self.db.prepare(
                "SELECT track_id, kind, seconds, end_seconds, name, color FROM markers
                 ORDER BY track_id, seconds",
            )?;
            let rows = q.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, f64>(2)?,
                    r.get::<_, Option<f64>>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                ))
            })?;
            for row in rows {
                let (id, kind, seconds, end, name, color) = row?;
                let Some(kind) = MarkerKind::from_name(&kind) else {
                    continue;
                };
                markers.entry(id).or_default().push(Marker {
                    kind,
                    seconds,
                    end,
                    name,
                    color: color.map(|c| c as u32),
                });
            }
        }
        let mut q = self.db.prepare(
            "SELECT id, path, artist, title, album, genre, duration, bpm, key, rating, color,
                    comment, grouping, labels, play_count, grid, added, key_source,
                    remix, year, track_number, user1, user2, gain, last_played,
                    disc_number, label, composer, remixer, isrc
             FROM tracks ORDER BY id",
        )?;
        let rows = q.query_map([], |r| {
            let id: i64 = r.get(0)?;
            let path: String = r.get(1)?;
            let grid: Option<String> = r.get(15)?;
            let labels: String = r.get(13)?;
            let key_source: Option<String> = r.get(17)?;
            Ok((
                id,
                PathBuf::from(path),
                grid,
                Track {
                    id: track_id(id),
                    artist: r.get(2)?,
                    title: r.get(3)?,
                    album: r.get(4)?,
                    genre: r.get(5)?,
                    duration: r.get(6)?,
                    bpm: r.get(7)?,
                    key: r.get(8)?,
                    rating: r.get::<_, i64>(9)?.clamp(0, 5) as u8,
                    first_beat: 0.0,
                    grid: BeatGrid::default(),
                    source: Source::File(PathBuf::new()),
                    waveform: Default::default(),
                    audio: None,
                    cover: None,
                    info: LibraryInfo {
                        color: r.get::<_, Option<i64>>(10)?.map(|c| c as u32),
                        comment: r.get(11)?,
                        grouping: r.get(12)?,
                        labels: labels.split_whitespace().map(str::to_string).collect(),
                        play_count: r.get::<_, i64>(14)?.max(0) as u32,
                        hotcues: Vec::new(),
                        fixed_grid: false,
                        added: r.get(16)?,
                        missing: false,
                        key_detected: key_source.map(|s| s == "analysis"),
                        remix: r.get(18)?,
                        year: r.get(19)?,
                        track_number: r.get::<_, i64>(20)?.max(0) as u32,
                        user1: r.get(21)?,
                        user2: r.get(22)?,
                        gain_db: r.get::<_, Option<f64>>(23)?.map(|g| g as f32),
                        last_played: r.get(24)?,
                        disc_number: r.get::<_, i64>(25)?.max(0) as u32,
                        label: r.get(26)?,
                        composer: r.get(27)?,
                        remixer: r.get(28)?,
                        isrc: r.get(29)?,
                        markers: Vec::new(),
                    },
                },
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, path, grid, mut t) = row?;
            t.info.missing = !exists(&path);
            t.source = Source::File(path);
            t.info.hotcues = cues.remove(&id).unwrap_or_default();
            t.info.markers = markers.remove(&id).unwrap_or_default();
            if let Some(g) = grid.map(|g| BeatGrid::from_text(&g)) {
                if !g.is_empty() {
                    t.set_grid(g);
                    t.info.fixed_grid = true;
                }
            }
            out.push(t);
        }
        Ok(out)
    }

    /// Brings the library up to date with a folder scan. Returns what
    /// changed.
    pub fn sync(&mut self, scanned: &[Track]) -> Result<SyncReport, Error> {
        let mut report = SyncReport::default();
        let tx = self.db.transaction()?;
        {
            let mut find = tx.prepare("SELECT id, file_mtime FROM tracks WHERE path = ?1")?;
            let mut insert = tx.prepare(
                "INSERT INTO tracks (path, artist, title, album, genre, duration, bpm, key,
                                     rating, file_mtime, source, added, key_source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                         CASE WHEN ?8 != '' THEN 'tags' END)",
            )?;
            let mut retag = tx.prepare(
                "UPDATE tracks SET artist = ?2, title = ?3, album = ?4, genre = ?5,
                        duration = ?6, file_mtime = ?7
                 WHERE id = ?1 AND source IN ('scan', 'file', 'purchase')",
            )?;
            let mut extras = tx.prepare(TAG_EXTRAS)?;
            for t in scanned {
                let Source::File(path) = &t.source else {
                    continue;
                };
                let p = path_text(path);
                let stamp = mtime(path);
                let found: Option<(i64, i64)> = find
                    .query_row([&p], |r| Ok((r.get(0)?, r.get(1)?)))
                    .optional()?;
                match found {
                    None => {
                        insert.execute(params![
                            p,
                            t.artist,
                            t.title,
                            t.album,
                            t.genre,
                            t.duration,
                            t.bpm,
                            t.key,
                            t.rating as i64,
                            stamp,
                            "scan",
                            now()
                        ])?;
                        extras.execute(tag_extras(tx.last_insert_rowid(), t))?;
                        set_details(&tx, tx.last_insert_rowid(), &t.info, true)?;
                        report.added += 1;
                    }
                    Some((id, old)) if old != stamp => {
                        if retag.execute(params![
                            id, t.artist, t.title, t.album, t.genre, t.duration, stamp
                        ])? > 0
                        {
                            extras.execute(tag_extras(id, t))?;
                            set_details(&tx, id, &t.info, true)?;
                            report.updated += 1;
                        }
                    }
                    Some(_) => {}
                }
            }
        }
        tx.commit()?;
        Ok(report)
    }

    /// Brings in another app's library (`source` names the app, `title` is
    /// what the DJ calls it), unless `stamp` is the same as last time: the
    /// importer makes it from its files' modification times and the DJ's
    /// choices, so it changes whenever there is something new. Its tags,
    /// ratings and colours win, as the other app is where they were set; its
    /// grids and cues fill in what the DJ hasn't edited here. Its lists are
    /// made again in a folder named `title`, replacing the ones brought in
    /// last time. `read` is only called when something has changed. Returns
    /// what changed, or `None` when it was left alone.
    pub fn import(
        &mut self,
        source: &str,
        title: &str,
        stamp: &str,
        read: impl FnOnce() -> Result<ImportedLibrary, String>,
    ) -> Result<Option<SyncReport>, String> {
        let key = format!("import_stamp:{source}");
        if self.meta(&key).as_deref() == Some(stamp) {
            return Ok(None);
        }
        let imported = read()?;
        let mut report = SyncReport::default();
        let tx = self.db.transaction().map_err(|e| e.to_string())?;
        import_tracks(&tx, source, &imported.tracks, &mut report).map_err(|e| e.to_string())?;
        import_lists(&tx, source, title, &imported.lists, &mut report)
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        self.set_meta(&key, stamp).map_err(|e| e.to_string())?;
        Ok(Some(report))
    }

    /// Adds one file (dropped in from outside the collection), returning the
    /// library's copy of it.
    pub fn add_file(&mut self, track: &Track) -> Result<Track, Error> {
        Ok(self.add_files(std::slice::from_ref(track))?.remove(0))
    }

    /// Adds files in one go (a folder from the Files source), returning the
    /// library's copies in the same order. Files it has already keep their
    /// rows.
    pub fn add_files(&mut self, tracks: &[Track]) -> Result<Vec<Track>, Error> {
        let tx = self.db.transaction()?;
        let mut out = Vec::with_capacity(tracks.len());
        {
            let mut insert = tx.prepare(
                "INSERT INTO tracks (path, artist, title, album, genre, duration, bpm, key,
                                     rating, file_mtime, source, added, key_source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'file', ?11,
                         CASE WHEN ?8 != '' THEN 'tags' END)
                 ON CONFLICT(path) DO NOTHING",
            )?;
            let mut find = tx.prepare("SELECT id FROM tracks WHERE path = ?1")?;
            let mut extras = tx.prepare(TAG_EXTRAS)?;
            for track in tracks {
                let Source::File(path) = &track.source else {
                    out.push(track.clone());
                    continue;
                };
                let p = path_text(path);
                let added = insert.execute(params![
                    p,
                    track.artist,
                    track.title,
                    track.album,
                    track.genre,
                    track.duration,
                    track.bpm,
                    track.key,
                    track.rating as i64,
                    mtime(path),
                    now()
                ])?;
                let id: i64 = find.query_row([&p], |r| r.get(0))?;
                if added > 0 {
                    extras.execute(tag_extras(id, track))?;
                    set_details(&tx, id, &track.info, true)?;
                }
                out.push(Track {
                    id: track_id(id),
                    ..track.clone()
                });
            }
        }
        tx.commit()?;
        Ok(out)
    }

    /// Stores what analysing a track found, unless its grid came from
    /// elsewhere (an import, or the DJ's own edits). The key is marked as
    /// found by analysis when [`LibraryInfo::key_detected`] says so.
    pub fn save_analysis(&self, id: usize, track: &Track) -> Result<(), Error> {
        let Some(row) = row_id(id) else { return Ok(()) };
        let grid = track.beat_grid();
        let key_source = track
            .info
            .key_detected
            .map(|d| if d { "analysis" } else { "tags" });
        self.db.execute(
            "UPDATE tracks SET key = ?3, duration = ?4,
                    key_source = CASE ?6 WHEN 'analysis' THEN 'analysis'
                                         WHEN 'tags' THEN COALESCE(key_source, 'tags')
                                         ELSE key_source END,
                    bpm = CASE WHEN grid_source IS NULL OR grid_source = 'analysis'
                               THEN ?2 ELSE bpm END,
                    grid = CASE WHEN grid_source IS NULL OR grid_source = 'analysis'
                                THEN ?5 ELSE grid END,
                    grid_source = COALESCE(grid_source, 'analysis')
             WHERE id = ?1",
            params![
                row,
                track.bpm,
                track.key,
                track.duration,
                (!grid.is_empty()).then(|| grid.to_text()),
                key_source
            ],
        )?;
        Ok(())
    }

    /// Stores a beat grid the DJ edited.
    pub fn save_grid(&self, id: usize, grid: &BeatGrid, bpm: f64) -> Result<(), Error> {
        let Some(row) = row_id(id) else { return Ok(()) };
        self.db.execute(
            "UPDATE tracks SET grid = ?2, bpm = ?3, grid_source = 'user' WHERE id = ?1",
            params![row, grid.to_text(), bpm],
        )?;
        Ok(())
    }

    /// Replaces a track's hot cues and saved loops with the DJ's.
    pub fn save_cues(&mut self, id: usize, cues: &[Hotcue]) -> Result<(), Error> {
        let Some(row) = row_id(id) else { return Ok(()) };
        let tx = self.db.transaction()?;
        tx.execute("DELETE FROM cues WHERE track_id = ?1", [row])?;
        for c in cues {
            tx.execute(
                "INSERT INTO cues (track_id, number, seconds, name, end_seconds, color, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'user')",
                params![
                    row,
                    c.number as i64,
                    c.seconds,
                    c.name,
                    c.end,
                    c.color.map(|c| c as i64)
                ],
            )?;
        }
        tx.commit()
    }

    pub fn set_rating(&self, id: usize, rating: u8) -> Result<(), Error> {
        self.update(id, "rating", rating.min(5) as i64)
    }

    /// Sets a track's colour (0xRRGGBB), or clears it.
    pub fn set_color(&self, id: usize, color: Option<u32>) -> Result<(), Error> {
        self.update(id, "color", color.map(|c| c as i64))
    }

    pub fn set_comment(&self, id: usize, comment: &str) -> Result<(), Error> {
        self.update(id, "comment", comment)
    }

    /// Saves what the DJ edited in the track info panel: the tags, rating,
    /// the User 1 and 2 tags and the gain. A key typed in counts as the
    /// DJ's own, so analysis leaves it alone.
    pub fn save_info(&self, id: usize, t: &Track) -> Result<(), Error> {
        let Some(row) = row_id(id) else { return Ok(()) };
        let labels: Vec<&str> = t
            .info
            .user1
            .split_whitespace()
            .chain(t.info.user2.split_whitespace())
            .collect();
        self.db.execute(
            "UPDATE tracks SET title = ?2, artist = ?3, remix = ?4, album = ?5, year = ?6,
                    genre = ?7, track_number = ?8, grouping = ?9, key = ?10,
                    key_source = CASE WHEN key = ?10 THEN key_source ELSE 'user' END,
                    rating = ?11, user1 = ?12, user2 = ?13, labels = ?14, comment = ?15,
                    gain = ?16, file_mtime = ?17
             WHERE id = ?1",
            params![
                row,
                t.title,
                t.artist,
                t.info.remix,
                t.album,
                t.info.year,
                t.genre,
                t.info.track_number as i64,
                t.info.grouping,
                t.key,
                t.rating.min(5) as i64,
                t.info.user1,
                t.info.user2,
                labels.join(" "),
                t.info.comment,
                t.info.gain_db.map(f64::from),
                match &t.source {
                    // The tags were just written, so a scan needn't read them again.
                    Source::File(path) => mtime(path),
                    Source::Synth { .. } => 0,
                },
            ],
        )?;
        Ok(())
    }

    fn update(&self, id: usize, column: &str, value: impl rusqlite::ToSql) -> Result<(), Error> {
        let Some(row) = row_id(id) else { return Ok(()) };
        self.db.execute(
            &format!("UPDATE tracks SET {column} = ?2 WHERE id = ?1"),
            params![row, value],
        )?;
        Ok(())
    }

    /// The tree of lists, folders and history sessions, parents before
    /// children, siblings in order.
    pub fn playlists(&self) -> Result<Vec<Playlist>, Error> {
        let mut q = self.db.prepare(
            "SELECT id, parent, name, kind, query, created, source FROM playlists
                 ORDER BY position, id",
        )?;
        let all: Vec<Playlist> = q
            .query_map([], |r| {
                Ok(Playlist {
                    id: r.get(0)?,
                    parent: r.get(1)?,
                    name: r.get(2)?,
                    kind: PlaylistKind::from_i64(r.get(3)?),
                    query: r.get(4)?,
                    created: r.get(5)?,
                    source: r.get(6)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        // Depth-first, so each folder is followed by what is in it.
        fn walk(all: &[Playlist], parent: Option<i64>, out: &mut Vec<Playlist>) {
            for p in all.iter().filter(|p| p.parent == parent) {
                out.push(p.clone());
                walk(all, Some(p.id), out);
            }
        }
        let mut out = Vec::with_capacity(all.len());
        walk(&all, None, &mut out);
        Ok(out)
    }

    /// Makes a list or folder at the end of `parent`.
    pub fn create_playlist(
        &self,
        name: &str,
        parent: Option<i64>,
        kind: PlaylistKind,
    ) -> Result<i64, Error> {
        let position: i64 = self.db.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM playlists WHERE parent IS ?1",
            [parent],
            |r| r.get(0),
        )?;
        self.db.execute(
            "INSERT INTO playlists (parent, name, kind, position, created)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![parent, name, kind.to_i64(), position, now()],
        )?;
        Ok(self.db.last_insert_rowid())
    }

    /// Makes a smart list at the end of `parent`.
    pub fn create_smart_list(
        &self,
        name: &str,
        parent: Option<i64>,
        query: &str,
    ) -> Result<i64, Error> {
        let id = self.create_playlist(name, parent, PlaylistKind::Smart)?;
        self.set_query(id, query)?;
        Ok(id)
    }

    /// Changes a smart list's search.
    pub fn set_query(&self, id: i64, query: &str) -> Result<(), Error> {
        self.db.execute(
            "UPDATE playlists SET query = ?2 WHERE id = ?1",
            params![id, query.trim()],
        )?;
        Ok(())
    }

    pub fn rename_playlist(&self, id: i64, name: &str) -> Result<(), Error> {
        self.db.execute(
            "UPDATE playlists SET name = ?2 WHERE id = ?1",
            params![id, name],
        )?;
        Ok(())
    }

    /// Deletes a list, or a folder with everything in it.
    pub fn delete_playlist(&self, id: i64) -> Result<(), Error> {
        self.db
            .execute("DELETE FROM playlists WHERE id = ?1", [id])?;
        Ok(())
    }

    /// The tracks in a list, in order (as track ids).
    pub fn playlist_tracks(&self, id: i64) -> Result<Vec<usize>, Error> {
        let mut q = self.db.prepare(
            "SELECT track_id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position",
        )?;
        let ids = q
            .query_map([id], |r| r.get::<_, i64>(0))?
            .map(|r| r.map(track_id))
            .collect();
        ids
    }

    /// Appends tracks to a list. Tracks not in the library are skipped.
    pub fn add_to_playlist(&mut self, id: i64, tracks: &[usize]) -> Result<(), Error> {
        let tx = self.db.transaction()?;
        let start: i64 = tx.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_tracks WHERE playlist_id = ?1",
            [id],
            |r| r.get(0),
        )?;
        for (position, row) in (start..).zip(tracks.iter().filter_map(|&t| row_id(t))) {
            tx.execute(
                "INSERT INTO playlist_tracks (playlist_id, position, track_id, played_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![id, position, row, now()],
            )?;
        }
        tx.commit()
    }

    /// Removes the track at `index` (0-based, in list order) from a list.
    pub fn remove_from_playlist(&mut self, id: i64, index: usize) -> Result<(), Error> {
        let tx = self.db.transaction()?;
        let position: Option<i64> = tx
            .query_row(
                "SELECT position FROM playlist_tracks WHERE playlist_id = ?1
                 ORDER BY position LIMIT 1 OFFSET ?2",
                params![id, index as i64],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(p) = position {
            tx.execute(
                "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND position = ?2",
                params![id, p],
            )?;
        }
        tx.commit()
    }

    /// Starts a history session named `name`; tracks played are added to it
    /// with [`Library::record_play`].
    pub fn start_session(&self, name: &str) -> Result<i64, Error> {
        let folder = match self
            .db
            .query_row(
                "SELECT id FROM playlists WHERE parent IS NULL AND kind = 2 AND name = 'History'",
                [],
                |r| r.get(0),
            )
            .optional()?
        {
            Some(id) => id,
            None => {
                // The history folder lives at the top and holds the sessions.
                self.db.execute(
                    "INSERT INTO playlists (parent, name, kind, position, created)
                     VALUES (NULL, 'History', 2, -1, ?1)",
                    [now()],
                )?;
                self.db.last_insert_rowid()
            }
        };
        self.create_playlist(name, Some(folder), PlaylistKind::History)
    }

    /// Records that a track was played: one more play, when, and a line in
    /// the session's history.
    pub fn record_play(&mut self, session: Option<i64>, id: usize) -> Result<(), Error> {
        let Some(row) = row_id(id) else { return Ok(()) };
        self.db.execute(
            "UPDATE tracks SET play_count = play_count + 1, last_played = ?2 WHERE id = ?1",
            params![row, now()],
        )?;
        if let Some(s) = session {
            self.add_to_playlist(s, &[id])?;
        }
        Ok(())
    }

    /// Links `linked` to `track`, as one that goes well with it.
    pub fn link(&self, track: usize, linked: usize) -> Result<(), Error> {
        let (Some(a), Some(b)) = (row_id(track), row_id(linked)) else {
            return Ok(());
        };
        if a != b {
            self.db.execute(
                "INSERT OR IGNORE INTO track_links (track, linked, added) VALUES (?1, ?2, ?3)",
                params![a, b, now()],
            )?;
        }
        Ok(())
    }

    /// Removes a link between two tracks, whichever way it was made.
    pub fn unlink(&self, track: usize, linked: usize) -> Result<(), Error> {
        let (Some(a), Some(b)) = (row_id(track), row_id(linked)) else {
            return Ok(());
        };
        self.db.execute(
            "DELETE FROM track_links WHERE (track = ?1 AND linked = ?2) OR (track = ?2 AND linked = ?1)",
            params![a, b],
        )?;
        Ok(())
    }

    /// The tracks linked to `track`, and those that link to it, newest
    /// link first.
    pub fn linked(&self, track: usize) -> Result<Vec<usize>, Error> {
        let Some(a) = row_id(track) else {
            return Ok(Vec::new());
        };
        let mut q = self.db.prepare(
            "SELECT other FROM (
                 SELECT linked AS other, added FROM track_links WHERE track = ?1
                 UNION ALL
                 SELECT track AS other, added FROM track_links WHERE linked = ?1
             ) GROUP BY other ORDER BY MAX(added) DESC, other",
        )?;
        let ids = q
            .query_map([a], |r| r.get::<_, i64>(0))?
            .map(|r| r.map(track_id))
            .collect();
        ids
    }

    /// Tracks played right before or after `track` in the history, most
    /// often first, then most recently.
    pub fn played_with(&self, track: usize) -> Result<Vec<usize>, Error> {
        let Some(a) = row_id(track) else {
            return Ok(Vec::new());
        };
        let mut q = self.db.prepare(
            "SELECT n.track_id FROM playlist_tracks t
             JOIN playlists p ON p.id = t.playlist_id AND p.kind = 2
             JOIN playlist_tracks n ON n.playlist_id = t.playlist_id
                 AND (n.position = t.position - 1 OR n.position = t.position + 1)
             WHERE t.track_id = ?1 AND n.track_id != ?1
             GROUP BY n.track_id
             ORDER BY COUNT(*) DESC, MAX(COALESCE(n.played_at, 0)) DESC, n.track_id",
        )?;
        let ids = q
            .query_map([a], |r| r.get::<_, i64>(0))?
            .map(|r| r.map(track_id))
            .collect();
        ids
    }

    /// Deletes history sessions in which nothing was played.
    pub fn drop_empty_sessions(&self) -> Result<(), Error> {
        self.db.execute(
            "DELETE FROM playlists WHERE kind = 2 AND parent IS NOT NULL
             AND id NOT IN (SELECT playlist_id FROM playlist_tracks)",
            [],
        )?;
        Ok(())
    }

    /// A track already in the library with this artist, title and (within
    /// two seconds) length, so the inbox doesn't add it again.
    pub fn find_same(
        &self,
        artist: &str,
        title: &str,
        duration: f64,
    ) -> Result<Option<usize>, Error> {
        let mut q = self
            .db
            .prepare("SELECT id, artist, title, duration FROM tracks")?;
        let (artist, title) = (simplify(artist), simplify(title));
        if title.is_empty() {
            return Ok(None);
        }
        let rows = q.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, f64>(3)?,
            ))
        })?;
        for row in rows {
            let (id, a, t, d) = row?;
            let close = duration <= 0.0 || d <= 0.0 || (d - duration).abs() < 2.0;
            if simplify(&a) == artist && simplify(&t) == title && close {
                return Ok(Some(track_id(id)));
            }
        }
        Ok(None)
    }

    /// Adds a file that arrived in the purchase inbox, from `store` (empty
    /// when unknown), and ties it to the matching wanted track if there is
    /// one. Returns the library's copy of it.
    pub fn add_purchase(&mut self, track: &Track, store: &str) -> Result<Track, Error> {
        let Source::File(path) = &track.source else {
            return Ok(track.clone());
        };
        let p = path_text(path);
        self.db.execute(
            "INSERT INTO tracks (path, artist, title, album, genre, duration, bpm, key, rating,
                                 file_mtime, source, store, added, key_source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, 'purchase', ?10, ?11,
                     CASE WHEN ?8 != '' THEN 'tags' END)
             ON CONFLICT(path) DO NOTHING",
            params![
                p,
                track.artist,
                track.title,
                track.album,
                track.genre,
                track.duration,
                track.bpm,
                track.key,
                mtime(path),
                store,
                now()
            ],
        )?;
        let row: i64 = self
            .db
            .query_row("SELECT id FROM tracks WHERE path = ?1", [&p], |r| r.get(0))?;
        self.db.execute(TAG_EXTRAS, tag_extras(row, track))?;
        let (artist, title) = (simplify(&track.artist), simplify(&track.title));
        let wanted: Vec<(i64, String, String, String)> = self
            .db
            .prepare("SELECT id, artist, title, page_url FROM wanted WHERE track_id IS NULL")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<Result<_, _>>()?;
        if let Some((id, _, _, url)) = wanted
            .iter()
            .find(|(_, a, t, _)| simplify(t) == title && (a.is_empty() || simplify(a) == artist))
        {
            self.db.execute(
                "UPDATE wanted SET track_id = ?2 WHERE id = ?1",
                params![id, row],
            )?;
            self.db.execute(
                "UPDATE tracks SET store_url = ?2 WHERE id = ?1",
                params![row, url],
            )?;
        }
        Ok(Track {
            id: track_id(row),
            ..track.clone()
        })
    }

    /// The list new purchases go into for the DJ to look through, made the
    /// first time it's needed (and again if the DJ deleted it).
    pub fn inbox_list(&self) -> Result<i64, Error> {
        if let Some(id) = self.meta("inbox_list").and_then(|v| v.parse::<i64>().ok()) {
            let exists = self
                .db
                .prepare("SELECT 1 FROM playlists WHERE id = ?1 AND kind = 0")?
                .exists([id])?;
            if exists {
                return Ok(id);
            }
        }
        let id = self.create_playlist("Inbox", None, PlaylistKind::List)?;
        self.set_meta("inbox_list", &id.to_string())?;
        Ok(id)
    }

    /// Remembers that the DJ went to buy a track. The same page twice is
    /// kept once.
    pub fn add_wanted(
        &self,
        store: &str,
        artist: &str,
        title: &str,
        page_url: &str,
    ) -> Result<(), Error> {
        self.db.execute(
            "INSERT INTO wanted (store, artist, title, page_url, added) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(page_url) DO NOTHING",
            params![store, artist, title, page_url, now()],
        )?;
        Ok(())
    }

    /// Tracks the DJ went to buy that haven't arrived yet, newest first.
    pub fn wanted(&self) -> Result<Vec<Wanted>, Error> {
        let mut q = self.db.prepare(
            "SELECT id, store, artist, title, page_url FROM wanted
             WHERE track_id IS NULL ORDER BY added DESC, id DESC",
        )?;
        let rows = q.query_map([], |r| {
            Ok(Wanted {
                id: r.get(0)?,
                store: r.get(1)?,
                artist: r.get(2)?,
                title: r.get(3)?,
                page_url: r.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn remove_wanted(&self, id: i64) -> Result<(), Error> {
        self.db.execute("DELETE FROM wanted WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Whether the inbox has already dealt with this file.
    pub fn inbox_seen(&self, path: &Path) -> Result<bool, Error> {
        self.db
            .prepare("SELECT 1 FROM inbox_log WHERE path = ?1")?
            .exists([path_text(path)])
    }

    /// Records what the inbox did with a file.
    pub fn log_inbox(&self, path: &Path, outcome: &str, track: Option<usize>) -> Result<(), Error> {
        self.db.execute(
            "INSERT OR REPLACE INTO inbox_log (path, outcome, track_id, at) VALUES (?1, ?2, ?3, ?4)",
            params![path_text(path), outcome, track.and_then(row_id), now()],
        )?;
        Ok(())
    }
}

/// Brings in another app's tracks (`source` names the app): their tags,
/// rating, colour and play count, and their grid and cues where the DJ
/// hasn't edited those here.
fn import_tracks(
    tx: &rusqlite::Transaction,
    source: &str,
    tracks: &[Track],
    report: &mut SyncReport,
) -> Result<(), Error> {
    let mut find = tx.prepare("SELECT id, grid_source FROM tracks WHERE path = ?1")?;
    let mut insert =
        tx.prepare("INSERT INTO tracks (path, file_mtime, source, added) VALUES (?1, ?2, ?4, ?3)")?;
    let mut set = tx.prepare(
        "UPDATE tracks SET artist = ?2, title = ?3, album = ?4, genre = ?5, rating = ?6,
                color = ?7, comment = ?8, grouping = ?9, labels = ?10,
                play_count = MAX(play_count, ?11), source = ?14,
                duration = CASE WHEN ?12 > 0 THEN ?12 ELSE duration END,
                key = CASE WHEN ?13 != '' THEN ?13 ELSE key END,
                key_source = CASE WHEN ?13 != '' THEN ?14 ELSE key_source END,
                remix = CASE WHEN ?15 != '' THEN ?15 ELSE remix END,
                user1 = CASE WHEN ?16 != '' THEN ?16 ELSE user1 END,
                user2 = CASE WHEN ?17 != '' THEN ?17 ELSE user2 END
         WHERE id = ?1",
    )?;
    let mut set_grid = tx.prepare(
        "UPDATE tracks SET grid = ?2, bpm = ?3, grid_source = ?4
         WHERE id = ?1 AND (grid_source IS NULL OR grid_source != 'user')",
    )?;
    let mut user_cues =
        tx.prepare("SELECT COUNT(*) FROM cues WHERE track_id = ?1 AND source = 'user'")?;
    let mut set_dates = tx.prepare(
        "UPDATE tracks SET added = COALESCE(?2, added),
                last_played = NULLIF(MAX(COALESCE(last_played, 0), COALESCE(?3, 0)), 0),
                gain = COALESCE(?4, gain)
         WHERE id = ?1",
    )?;
    let mut clear_markers =
        tx.prepare("DELETE FROM markers WHERE track_id = ?1 AND source = ?2")?;
    let mut add_marker = tx.prepare(
        "INSERT INTO markers (track_id, kind, seconds, end_seconds, name, color, source)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    let mut clear_cues = tx.prepare("DELETE FROM cues WHERE track_id = ?1")?;
    let mut add_cue = tx.prepare(
        "INSERT OR REPLACE INTO cues (track_id, number, seconds, name, color, end_seconds, source)
         VALUES (?1, ?2, ?3, ?4, ?7, ?5, ?6)",
    )?;
    for t in tracks {
        let Source::File(path) = &t.source else {
            continue;
        };
        let p = path_text(path);
        let found: Option<(i64, Option<String>)> = find
            .query_row([&p], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        let id = match found {
            Some((id, _)) => {
                report.updated += 1;
                id
            }
            None => {
                insert.execute(params![p, mtime(path), now(), source])?;
                report.added += 1;
                tx.last_insert_rowid()
            }
        };
        set.execute(params![
            id,
            t.artist,
            t.title,
            t.album,
            t.genre,
            t.rating as i64,
            t.info.color.map(|c| c as i64),
            t.info.comment,
            t.info.grouping,
            t.info.labels.join(" "),
            t.info.play_count as i64,
            t.duration,
            t.key,
            source,
            t.info.remix,
            t.info.user1,
            t.info.user2
        ])?;
        let grid = t.beat_grid();
        if t.info.fixed_grid && !grid.is_empty() {
            set_grid.execute(params![id, grid.to_text(), t.bpm, source])?;
        }
        let mine: i64 = user_cues.query_row([id], |r| r.get(0))?;
        if mine == 0 {
            clear_cues.execute([id])?;
            for c in &t.info.hotcues {
                add_cue.execute(params![
                    id,
                    c.number as i64,
                    c.seconds,
                    c.name,
                    c.end,
                    source,
                    c.color.map(|c| c as i64)
                ])?;
            }
        }
        set_details(tx, id, &t.info, false)?;
        // When it joined the other app's library, and when it was last
        // played there.
        set_dates.execute(params![
            id,
            (t.info.added > 0).then_some(t.info.added),
            t.info.last_played,
            t.info.gain_db.map(f64::from)
        ])?;
        clear_markers.execute(params![id, source])?;
        for m in &t.info.markers {
            add_marker.execute(params![
                id,
                m.kind.name(),
                m.seconds,
                m.end,
                m.name,
                m.color.map(|c| c as i64),
                source
            ])?;
        }
    }
    Ok(())
}

/// Stores the details beyond artist, title, album and genre. With
/// `overwrite` off, empty values leave what is there.
fn set_details(tx: &Connection, id: i64, info: &LibraryInfo, overwrite: bool) -> Result<(), Error> {
    let sql = if overwrite {
        "UPDATE tracks SET disc_number = ?4, label = ?5, composer = ?6, remixer = ?7, isrc = ?8
         WHERE id = ?1"
    } else {
        "UPDATE tracks SET
                year = CASE WHEN ?2 != '' THEN ?2 ELSE year END,
                track_number = CASE WHEN ?3 > 0 THEN ?3 ELSE track_number END,
                disc_number = CASE WHEN ?4 > 0 THEN ?4 ELSE disc_number END,
                label = CASE WHEN ?5 != '' THEN ?5 ELSE label END,
                composer = CASE WHEN ?6 != '' THEN ?6 ELSE composer END,
                remixer = CASE WHEN ?7 != '' THEN ?7 ELSE remixer END,
                isrc = CASE WHEN ?8 != '' THEN ?8 ELSE isrc END
         WHERE id = ?1"
    };
    tx.prepare_cached(sql)?.execute(params![
        id,
        info.year,
        info.track_number as i64,
        info.disc_number as i64,
        info.label,
        info.composer,
        info.remixer,
        info.isrc
    ])?;
    Ok(())
}

/// Makes another app's lists again, in a folder named `title` at the top,
/// in place of those brought in last time. History sessions go to the
/// history tab, as the DJ's own do.
fn import_lists(
    tx: &rusqlite::Transaction,
    source: &str,
    title: &str,
    lists: &[ImportedList],
    report: &mut SyncReport,
) -> Result<(), Error> {
    // Deleting the folder takes everything in it along.
    tx.execute("DELETE FROM playlists WHERE source = ?1", [source])?;
    if lists.is_empty() {
        return Ok(());
    }
    let mut insert = tx.prepare(
        "INSERT INTO playlists (parent, name, kind, position, created, query, source)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    let mut make = |parent: Option<i64>,
                    name: &str,
                    kind: PlaylistKind,
                    position: i64,
                    created: i64,
                    query: Option<&str>|
     -> Result<i64, Error> {
        insert.execute(params![
            parent,
            name,
            kind.to_i64(),
            position,
            created,
            query,
            source
        ])?;
        Ok(tx.last_insert_rowid())
    };
    // After the DJ's own lists.
    let last: i64 = tx.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM playlists WHERE parent IS NULL",
        [],
        |r| r.get(0),
    )?;
    let top = make(None, title, PlaylistKind::Folder, last, now(), None)?;
    report.lists += 1;
    let mut folders: HashMap<Vec<String>, i64> = HashMap::new();
    let mut find = tx.prepare("SELECT id FROM tracks WHERE path = ?1")?;
    let mut add = tx.prepare(
        "INSERT INTO playlist_tracks (playlist_id, position, track_id, played_at)
         VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (i, list) in lists.iter().enumerate() {
        let mut parent = top;
        for depth in 1..=list.folders.len() {
            let path = list.folders[..depth].to_vec();
            parent = match folders.get(&path) {
                Some(&id) => id,
                None => {
                    let id = make(
                        Some(parent),
                        &path[depth - 1],
                        PlaylistKind::Folder,
                        i as i64,
                        now(),
                        None,
                    )?;
                    report.lists += 1;
                    folders.insert(path, id);
                    id
                }
            };
        }
        let (kind, created, query) = match &list.kind {
            ImportedListKind::List => (PlaylistKind::List, now(), None),
            ImportedListKind::History { started } => (PlaylistKind::History, *started, None),
            ImportedListKind::Smart { query } => (PlaylistKind::Smart, now(), Some(query.as_str())),
        };
        let id = make(Some(parent), &list.name, kind, i as i64, created, query)?;
        report.lists += 1;
        let mut position = 0;
        for e in &list.entries {
            let row: Option<i64> = find
                .query_row([path_text(&e.path)], |r| r.get(0))
                .optional()?;
            let Some(row) = row else {
                report.unmatched += 1;
                continue;
            };
            add.execute(params![id, position, row, e.played_at.unwrap_or(created)])?;
            position += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mixxx_engine::beats::GridMarker;

    fn file_track(path: &str, title: &str) -> Track {
        Track {
            id: 0,
            artist: "Artist".into(),
            title: title.into(),
            album: String::new(),
            genre: "House".into(),
            rating: 0,
            bpm: 0.0,
            key: String::new(),
            duration: 200.0,
            first_beat: 0.0,
            grid: BeatGrid::default(),
            source: Source::File(PathBuf::from(path)),
            waveform: Default::default(),
            audio: None,
            cover: None,
            info: Default::default(),
        }
    }

    #[test]
    fn scans_add_new_files_once() {
        let mut lib = Library::in_memory().unwrap();
        let scanned = [file_track("/m/a.mp3", "A"), file_track("/m/b.mp3", "B")];
        let report = lib.sync(&scanned).unwrap();
        assert_eq!(report.added, 2);
        // Scanning again changes nothing.
        assert_eq!(lib.sync(&scanned).unwrap(), SyncReport::default());
        let tracks = lib.tracks(|_| true).unwrap();
        assert_eq!(tracks.len(), 2);
        assert!(tracks.iter().all(|t| t.id >= ID_BASE));
        // Files that are gone are left out.
        assert_eq!(lib.tracks(|p| p.ends_with("a.mp3")).unwrap().len(), 1);
    }

    #[test]
    fn keeps_the_info_panels_fields() {
        let mut lib = Library::in_memory().unwrap();
        let mut scanned = file_track("/m/a.mp3", "A");
        scanned.info.remix = "Club Mix".into();
        scanned.info.year = "1998".into();
        scanned.info.track_number = 4;
        scanned.info.grouping = "3 JNY".into();
        scanned.rating = 3;
        lib.sync(std::slice::from_ref(&scanned)).unwrap();
        let mut t = lib.tracks(|_| true).unwrap().remove(0);
        assert_eq!(t.info.remix, "Club Mix");
        assert_eq!(t.info.year, "1998");
        assert_eq!(t.info.track_number, 4);
        assert_eq!(t.info.grouping, "3 JNY");
        assert_eq!(t.rating, 3);
        assert_eq!(t.info.gain_db, None);

        t.title = "Edited".into();
        t.key = "8A".into();
        t.info.user1 = "#uplifting".into();
        t.info.user2 = "#synth #vox".into();
        t.info.gain_db = Some(-2.5);
        lib.save_info(t.id, &t).unwrap();
        let back = lib.tracks(|_| true).unwrap().remove(0);
        assert_eq!(back.title, "Edited");
        assert_eq!(back.info.user1, "#uplifting");
        assert_eq!(back.info.user2, "#synth #vox");
        assert_eq!(back.info.labels, ["#uplifting", "#synth", "#vox"]);
        assert_eq!(back.info.gain_db, Some(-2.5));
        // A key the DJ typed in is theirs, not analysis's.
        assert_eq!(back.info.key_detected, Some(false));
    }

    fn imported_track(path: &str) -> Track {
        let mut t = file_track(path, "Imported");
        t.rating = 4;
        t.bpm = 128.0;
        t.first_beat = 0.25;
        t.info = LibraryInfo {
            comment: "nice".into(),
            grouping: "11 YOGA".into(),
            labels: vec!["#synth".into(), "#vox".into()],
            color: Some(0xff8800),
            play_count: 3,
            hotcues: vec![Hotcue {
                number: 0,
                seconds: 32.0,
                name: "Drop".into(),
                end: None,
                color: None,
            }],
            fixed_grid: true,
            ..Default::default()
        };
        t
    }

    #[test]
    fn imports_and_keeps_the_djs_edits() {
        let mut lib = Library::in_memory().unwrap();
        let scanned = [file_track("/m/a.mp3", "A"), file_track("/m/b.mp3", "B")];
        lib.sync(&scanned).unwrap();
        let other = vec![imported_track("/m/b.mp3"), imported_track("/m/c.mp3")];
        let read = || {
            Ok(ImportedLibrary {
                tracks: other.clone(),
                lists: Vec::new(),
            })
        };
        let report = lib.import("other", "Other", "1", read).unwrap().unwrap();
        assert_eq!((report.added, report.updated), (1, 1));
        let tracks = lib.tracks(|_| true).unwrap();
        assert_eq!(tracks.len(), 3);
        let b = tracks.iter().find(|t| t.title == "Imported").unwrap();
        assert!(b.id >= ID_BASE);
        assert_eq!(b.rating, 4);
        assert_eq!(b.info.color, Some(0xff8800));
        assert_eq!(b.info.labels, ["#synth", "#vox"]);
        assert_eq!(b.info.grouping, "11 YOGA");
        assert_eq!(b.info.hotcues.len(), 1);
        assert!(b.info.fixed_grid);
        assert!((b.bpm - 128.0).abs() < 1e-9);
        assert!((b.first_beat - 0.25).abs() < 1e-9);

        // The DJ edits the grid and the cues here; a later import keeps them.
        let grid = BeatGrid::new(vec![
            GridMarker {
                seconds: 0.5,
                bpm: 120.0,
            },
            GridMarker {
                seconds: 64.5,
                bpm: 124.0,
            },
        ]);
        lib.save_grid(b.id, &grid, 124.0).unwrap();
        lib.save_cues(
            b.id,
            &[
                Hotcue {
                    number: 2,
                    seconds: 10.0,
                    name: String::new(),
                    end: None,
                    color: None,
                },
                Hotcue {
                    number: 3,
                    seconds: 20.0,
                    name: "Break loop".into(),
                    end: Some(24.0),
                    color: None,
                },
            ],
        )
        .unwrap();
        lib.db.execute("DELETE FROM meta", []).unwrap();
        assert!(lib.import("other", "Other", "1", read).unwrap().is_some());
        let b = lib
            .tracks(|_| true)
            .unwrap()
            .into_iter()
            .find(|t| t.id == b.id)
            .unwrap();
        assert_eq!(b.grid, grid);
        assert_eq!(b.info.hotcues[0].number, 2);
        assert_eq!(b.info.hotcues[0].end, None);
        // A saved loop keeps its end.
        assert_eq!(b.info.hotcues[1].end, Some(24.0));
        // An unchanged file isn't read again.
        assert!(lib
            .import("other", "Other", "1", || panic!("read again"))
            .unwrap()
            .is_none());
    }

    #[test]
    fn imports_details_markers_and_lists() {
        use mixxx_engine::import::ImportedEntry;
        let mut lib = Library::in_memory().unwrap();
        lib.sync(&[file_track("/m/a.mp3", "A")]).unwrap();
        let mine = lib
            .create_playlist("Mine", None, PlaylistKind::List)
            .unwrap();
        let mut b = imported_track("/m/b.mp3");
        b.info.year = "1999".into();
        b.info.label = "Warp".into();
        b.info.added = 1_600_000_000;
        b.info.last_played = Some(1_700_000_000);
        b.info.hotcues[0].color = Some(0x00ff00);
        b.info.hotcues.push(Hotcue {
            number: mixxx_engine::MEMORY_CUE,
            seconds: 5.0,
            ..Default::default()
        });
        b.info.markers = vec![Marker {
            kind: MarkerKind::Section,
            seconds: 100.0,
            end: None,
            name: "Break 1".into(),
            color: None,
        }];
        let entry = |p: &str, at| ImportedEntry {
            path: PathBuf::from(p),
            played_at: at,
        };
        let lists = vec![
            ImportedList {
                folders: vec!["Sets".into(), "2023".into()],
                name: "Friday".into(),
                kind: ImportedListKind::List,
                entries: vec![
                    entry("/m/b.mp3", None),
                    entry("netsearch://td1", None),
                    entry("/m/a.mp3", None),
                ],
            },
            ImportedList {
                folders: Vec::new(),
                name: "2023-11-13".into(),
                kind: ImportedListKind::History {
                    started: 1_699_900_000,
                },
                entries: vec![entry("/m/b.mp3", Some(1_699_928_555))],
            },
            ImportedList {
                folders: vec!["Filters".into()],
                name: "Old ones".into(),
                kind: ImportedListKind::Smart {
                    query: "year:<2000".into(),
                },
                entries: Vec::new(),
            },
        ];
        let read = || {
            Ok(ImportedLibrary {
                tracks: vec![b.clone()],
                lists: lists.clone(),
            })
        };
        let report = lib.import("vdj", "VirtualDJ", "1", read).unwrap().unwrap();
        // VirtualDJ, Sets, 2023, Friday, the session, Filters, Old ones.
        assert_eq!((report.lists, report.unmatched), (7, 1));
        let t = lib
            .tracks(|_| true)
            .unwrap()
            .into_iter()
            .find(|t| t.title == "Imported")
            .unwrap();
        assert_eq!(
            (t.info.year.as_str(), t.info.label.as_str()),
            ("1999", "Warp")
        );
        assert_eq!(
            (t.info.added, t.info.last_played),
            (1_600_000_000, Some(1_700_000_000))
        );
        assert_eq!(t.info.hotcues.len(), 2);
        assert_eq!(t.info.hotcues[0].color, Some(0x00ff00));
        assert!(t.info.hotcues[1].is_memory());
        assert_eq!(t.info.markers[0].name, "Break 1");

        let tree = lib.playlists().unwrap();
        let names: Vec<&str> = tree.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names[names.len() - 8..],
            [
                "Mine",
                "VirtualDJ",
                "Sets",
                "2023",
                "Friday",
                "2023-11-13",
                "Filters",
                "Old ones"
            ]
        );
        let friday = tree.iter().find(|p| p.name == "Friday").unwrap();
        assert_eq!(friday.source.as_deref(), Some("vdj"));
        assert_eq!(lib.playlist_tracks(friday.id).unwrap(), [t.id, ID_BASE + 1]);
        let session = tree
            .iter()
            .find(|p| p.kind == PlaylistKind::History)
            .unwrap();
        assert_eq!(session.created, 1_699_900_000);
        let smart = tree
            .iter()
            .find(|p| p.kind == PlaylistKind::Smart && p.name == "Old ones");
        assert_eq!(smart.unwrap().query.as_deref(), Some("year:<2000"));

        // Importing again replaces the lists brought in last time, and
        // leaves the DJ's own alone.
        let fewer = || {
            Ok(ImportedLibrary {
                tracks: vec![b.clone()],
                lists: lists[..1].to_vec(),
            })
        };
        lib.import("vdj", "VirtualDJ", "2", fewer).unwrap().unwrap();
        let tree = lib.playlists().unwrap();
        assert!(tree.iter().any(|p| p.id == mine));
        assert_eq!(tree.iter().filter(|p| p.name == "Friday").count(), 1);
        assert!(!tree.iter().any(|p| p.name == "Old ones"));
    }

    #[test]
    fn analysis_is_stored_once_and_ratings_and_colours_are_kept() {
        let mut lib = Library::in_memory().unwrap();
        lib.sync(&[file_track("/m/a.mp3", "A")]).unwrap();
        let mut a = lib.tracks(|_| true).unwrap().remove(0);
        assert!(!a.info.fixed_grid);
        a.set_grid(BeatGrid::constant(126.0, 0.1));
        a.key = "8A".into();
        a.info.key_detected = Some(true);
        lib.save_analysis(a.id, &a).unwrap();
        lib.set_rating(a.id, 5).unwrap();
        lib.set_color(a.id, Some(0x00ff00)).unwrap();
        lib.set_comment(a.id, "opener").unwrap();
        let a2 = lib.tracks(|_| true).unwrap().remove(0);
        assert!(a2.info.fixed_grid);
        assert_eq!((a2.bpm, a2.key.as_str(), a2.rating), (126.0, "8A", 5));
        assert_eq!(a2.info.key_detected, Some(true));
        assert_eq!(a2.info.color, Some(0x00ff00));
        assert_eq!(a2.info.comment, "opener");
        // A user grid isn't replaced by a later analysis.
        lib.save_grid(a.id, &BeatGrid::constant(125.0, 0.0), 125.0)
            .unwrap();
        lib.save_analysis(a.id, &a).unwrap();
        assert_eq!(lib.tracks(|_| true).unwrap()[0].bpm, 125.0);
        assert_eq!(
            lib.tracks(|_| true).unwrap()[0].grid,
            BeatGrid::constant(125.0, 0.0)
        );
    }

    #[test]
    fn keys_remember_where_they_came_from() {
        let mut lib = Library::in_memory().unwrap();
        let mut tagged = file_track("/m/tagged.mp3", "Tagged");
        tagged.key = "5A".into();
        lib.sync(&[tagged, file_track("/m/bare.mp3", "Bare")])
            .unwrap();
        let found = |lib: &Library| -> Vec<Option<bool>> {
            let tracks = lib.tracks(|_| true).unwrap();
            tracks.iter().map(|t| t.info.key_detected).collect()
        };
        assert_eq!(found(&lib), [Some(false), None]);

        // Analysis keeps the tagged key as the tags' own.
        let tracks = lib.tracks(|_| true).unwrap();
        let mut t = tracks[0].clone();
        t.info.key_detected = Some(false);
        lib.save_analysis(t.id, &t).unwrap();
        let mut b = tracks[1].clone();
        b.key = "9B".into();
        b.info.key_detected = Some(true);
        lib.save_analysis(b.id, &b).unwrap();
        assert_eq!(found(&lib), [Some(false), Some(true)]);

        // Another app's key is that app's.
        let mut imported = imported_track("/m/bare.mp3");
        imported.key = "1A".into();
        let tx = lib.db.transaction().unwrap();
        import_tracks(&tx, "virtualdj", &[imported], &mut SyncReport::default()).unwrap();
        tx.commit().unwrap();
        assert_eq!(found(&lib), [Some(false), Some(false)]);
    }

    #[test]
    fn a_version_4_library_marks_imported_keys() {
        let path = std::env::temp_dir().join(format!("opendj-lib-v4-{}.db", std::process::id()));
        {
            let mut lib = Library::open(&path).unwrap();
            let mut t = file_track("/m/a.mp3", "A");
            t.key = "4B".into();
            lib.sync(&[t, file_track("/m/b.mp3", "B")]).unwrap();
            lib.db
                .execute_batch(
                    "UPDATE tracks SET key_source = NULL;
                     UPDATE tracks SET source = 'virtualdj', key = '7A' WHERE path = '/m/b.mp3';
                     PRAGMA user_version = 4",
                )
                .unwrap();
        }
        let lib = Library::open(&path).unwrap();
        let keys: Vec<Option<bool>> = lib
            .tracks(|_| true)
            .unwrap()
            .iter()
            .map(|t| t.info.key_detected)
            .collect();
        // A scanned key could be from tags or analysis; that's checked on load.
        assert_eq!(keys, [None, Some(false)]);
        drop(lib);
        for end in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{end}", path.display()));
        }
    }

    #[test]
    fn smart_lists_and_missing_files() {
        let mut lib = Library::in_memory().unwrap();
        let starters: Vec<(String, Option<String>)> = lib
            .playlists()
            .unwrap()
            .into_iter()
            .map(|p| (p.name, p.query))
            .collect();
        assert_eq!(starters.len(), STARTER_SMART_LISTS.len());
        assert_eq!(
            starters[0],
            ("Mixes with deck".into(), Some("~deck".into()))
        );
        let id = lib
            .create_smart_list("Peak", None, " bpm:126-130 ")
            .unwrap();
        lib.set_query(id, "bpm:127-131").unwrap();
        let peak = lib
            .playlists()
            .unwrap()
            .into_iter()
            .find(|p| p.id == id)
            .unwrap();
        assert_eq!(peak.kind, PlaylistKind::Smart);
        assert_eq!(peak.query.as_deref(), Some("bpm:127-131"));

        lib.sync(&[file_track("/m/a.mp3", "A"), file_track("/m/b.mp3", "B")])
            .unwrap();
        let all = lib.all_tracks(|p| p.ends_with("a.mp3")).unwrap();
        assert_eq!(all.len(), 2);
        assert!(!all[0].info.missing && all[1].info.missing);
        assert!(all[0].info.added > 0, "the date it was added is kept");
        assert_eq!(lib.tracks(|p| p.ends_with("a.mp3")).unwrap().len(), 1);
    }

    #[test]
    fn a_version_3_library_gains_saved_loops() {
        let path = std::env::temp_dir().join(format!("opendj-v3-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let db = Connection::open(&path).unwrap();
            // The cues table before saved loops; markers came later still.
            let schema = SCHEMA.replacen("    end_seconds REAL,\n", "", 1);
            db.execute_batch(&schema[..schema.find("-- Named sections").unwrap()])
                .unwrap();
            db.execute_batch("PRAGMA user_version = 3").unwrap();
        }
        let mut lib = Library::open(&path).unwrap();
        lib.sync(&[file_track("/m/a.mp3", "A")]).unwrap();
        let id = lib.tracks(|_| true).unwrap()[0].id;
        let saved = Hotcue {
            number: 0,
            seconds: 8.0,
            name: String::new(),
            end: Some(16.0),
            color: None,
        };
        lib.save_cues(id, std::slice::from_ref(&saved)).unwrap();
        assert_eq!(lib.tracks(|_| true).unwrap()[0].info.hotcues, [saved]);
        drop(lib);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_version_1_library_gains_smart_lists() {
        let path = std::env::temp_dir().join(format!("opendj-v1-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let db = Connection::open(&path).unwrap();
            db.execute_batch(SCHEMA).unwrap();
            db.execute_batch("PRAGMA user_version = 1").unwrap();
            db.execute(
                "INSERT INTO playlists (name, kind) VALUES ('Old list', 0)",
                [],
            )
            .unwrap();
        }
        let lib = Library::open(&path).unwrap();
        let lists = lib.playlists().unwrap();
        assert_eq!(lists[0].name, "Old list");
        assert_eq!(lists.len(), 1 + STARTER_SMART_LISTS.len());
        drop(lib);
        // Opening again doesn't add them twice.
        assert_eq!(
            Library::open(&path).unwrap().playlists().unwrap().len(),
            lists.len()
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn links_and_tracks_played_together() {
        let mut lib = Library::in_memory().unwrap();
        let files: Vec<Track> = ["a", "b", "c", "d"]
            .iter()
            .map(|n| file_track(&format!("/m/{n}.mp3"), n))
            .collect();
        lib.sync(&files).unwrap();
        let ids: Vec<usize> = lib.tracks(|_| true).unwrap().iter().map(|t| t.id).collect();
        let (a, b, c, d) = (ids[0], ids[1], ids[2], ids[3]);

        lib.link(a, b).unwrap();
        lib.link(c, a).unwrap();
        lib.link(a, a).unwrap();
        // Forward and backward links both show, each once.
        let mut linked = lib.linked(a).unwrap();
        linked.sort();
        assert_eq!(linked, [b, c]);
        assert_eq!(lib.linked(b).unwrap(), [a]);
        lib.unlink(b, a).unwrap();
        assert_eq!(lib.linked(a).unwrap(), [c]);

        // Two sessions: b follows a twice, d precedes a once.
        let s1 = lib.start_session("one").unwrap();
        for t in [d, a, b] {
            lib.record_play(Some(s1), t).unwrap();
        }
        let s2 = lib.start_session("two").unwrap();
        for t in [a, b, c] {
            lib.record_play(Some(s2), t).unwrap();
        }
        // An ordinary list doesn't count as history.
        let list = lib
            .create_playlist("List", None, PlaylistKind::List)
            .unwrap();
        lib.add_to_playlist(list, &[a, c]).unwrap();
        assert_eq!(lib.played_with(a).unwrap(), [b, d]);
        assert_eq!(lib.played_with(c).unwrap(), [b]);
    }

    #[test]
    fn playlists_folders_and_history() {
        let mut lib = Library::in_memory().unwrap();
        lib.sync(&[
            file_track("/m/a.mp3", "A"),
            file_track("/m/b.mp3", "B"),
            file_track("/m/c.mp3", "C"),
        ])
        .unwrap();
        let ids: Vec<usize> = lib.tracks(|_| true).unwrap().iter().map(|t| t.id).collect();
        let sets = lib
            .create_playlist("Sets", None, PlaylistKind::Folder)
            .unwrap();
        let friday = lib
            .create_playlist("Friday", Some(sets), PlaylistKind::List)
            .unwrap();
        let warmup = lib
            .create_playlist("Warm-up", None, PlaylistKind::List)
            .unwrap();
        lib.add_to_playlist(friday, &[ids[2], ids[0], ids[2]])
            .unwrap();
        assert_eq!(
            lib.playlist_tracks(friday).unwrap(),
            [ids[2], ids[0], ids[2]]
        );
        lib.remove_from_playlist(friday, 1).unwrap();
        assert_eq!(lib.playlist_tracks(friday).unwrap(), [ids[2], ids[2]]);
        lib.rename_playlist(warmup, "Openers").unwrap();
        let names: Vec<String> = lib
            .playlists()
            .unwrap()
            .into_iter()
            .filter(|p| p.kind != PlaylistKind::Smart)
            .map(|p| p.name)
            .collect();
        assert_eq!(names, ["Sets", "Friday", "Openers"]);

        let session = lib.start_session("2026-10-13 21:00").unwrap();
        lib.record_play(Some(session), ids[1]).unwrap();
        lib.record_play(Some(session), ids[1]).unwrap();
        let empty = lib.start_session("2026-10-14 21:00").unwrap();
        lib.drop_empty_sessions().unwrap();
        let tree = lib.playlists().unwrap();
        assert!(tree
            .iter()
            .any(|p| p.name == "History" && p.parent.is_none()));
        assert!(tree.iter().any(|p| p.id == session));
        assert!(!tree.iter().any(|p| p.id == empty));
        assert_eq!(lib.playlist_tracks(session).unwrap(), [ids[1], ids[1]]);
        let b = lib
            .tracks(|_| true)
            .unwrap()
            .into_iter()
            .find(|t| t.id == ids[1])
            .unwrap();
        assert_eq!(b.info.play_count, 2);

        // Deleting a folder deletes what is in it.
        lib.delete_playlist(sets).unwrap();
        assert!(lib.playlist_tracks(friday).unwrap().is_empty());
        assert!(!lib.playlists().unwrap().iter().any(|p| p.id == friday));
    }

    #[test]
    fn purchases_match_wanted_and_dedupe() {
        let mut lib = Library::in_memory().unwrap();
        lib.add_wanted(
            "Bandcamp",
            "Bonobo",
            "Kerala",
            "https://b.bandcamp.com/track/kerala",
        )
        .unwrap();
        lib.add_wanted(
            "Bandcamp",
            "Bonobo",
            "Kerala",
            "https://b.bandcamp.com/track/kerala",
        )
        .unwrap();
        lib.add_wanted(
            "Bandcamp",
            "Bicep",
            "Glue",
            "https://bicep.bandcamp.com/track/glue",
        )
        .unwrap();
        assert_eq!(lib.wanted().unwrap().len(), 2);

        let mut t = file_track(
            "/m/Purchased/Bandcamp/Bonobo/Migration/03 Kerala.flac",
            "Kerala",
        );
        t.artist = "BONOBO".into();
        t.duration = 300.0;
        let added = lib.add_purchase(&t, "Bandcamp").unwrap();
        assert!(added.id >= ID_BASE);
        let left = lib.wanted().unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].title, "Glue");
        let url: Option<String> = lib
            .db
            .query_row(
                "SELECT store_url FROM tracks WHERE id = ?1",
                [row_id(added.id)],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(url.as_deref(), Some("https://b.bandcamp.com/track/kerala"));

        assert_eq!(
            lib.find_same("Bonobo", "kerala", 301.0).unwrap(),
            Some(added.id)
        );
        assert_eq!(lib.find_same("Bonobo", "Kerala", 200.0).unwrap(), None);
        assert_eq!(lib.find_same("Other", "Kerala", 300.0).unwrap(), None);

        let inbox = lib.inbox_list().unwrap();
        assert_eq!(lib.inbox_list().unwrap(), inbox);
        lib.delete_playlist(inbox).unwrap();
        let again = lib.inbox_list().unwrap();
        assert!(lib
            .playlists()
            .unwrap()
            .iter()
            .any(|p| p.id == again && p.name == "Inbox"));

        let p = Path::new("/dl/a.flac");
        assert!(!lib.inbox_seen(p).unwrap());
        lib.log_inbox(p, "imported", Some(added.id)).unwrap();
        assert!(lib.inbox_seen(p).unwrap());
    }
}
