//! Brings a djay Pro library into the collection at start, when the DJ turns
//! it on in Settings › Collection › Import: each track's tags, BPM, key,
//! hot cues 1 to 8 with their names and colors, saved loops, Automix
//! points, play count, color and grid anchor, and djay's playlists. It
//! opens djay's database read-only and never writes it.
//!
//! djay keeps its library in `MediaLibrary.db`, a YapDatabase on SQLite:
//! table `database2` holds one row per track in each `collection`, keyed by
//! the track's uuid, and each row's `data` is a TSAF object ([`tsaf`]):
//!
//! - `mediaItemTitleIDs`: `ADCMediaItemTitleID` with `title`, `artist`,
//!   `album` and `duration`.
//! - `localMediaItemLocations`: `ADCMediaItemLocation` with `sourceURIs`,
//!   file URLs or a macOS bookmark ([`bookmark`]).
//! - `mediaItemAnalyzedData`: `bpm` and `keySignatureIndex`.
//! - `mediaItemUserData`: `cuePoints` (`ADCCuePoint`: `time`, `endTime`
//!   (-1 unless it is a loop), `number` 0 to 7, `comment`, `colorIndex`),
//!   `automixStartPoint` and `automixEndPoint` (cue points too),
//!   `playCount`, `colorIndex` and `beatGridEdits`.
//!
//! Playlists are a YapDatabase view: `view_mediaItemPlaylistView_page` has
//! a `group` (the playlist) per page of rows, `_map` which page each row is
//! on.
//!
//! Prior work: EngineX-OS's `pkg/djay` (MIT, Copyright (c) 2026 EngineX OS
//! Contributors) set out what to bring across; the record layout follows
//! djay-pro-autohotcue (MIT, Copyright (c) 2026 payne0420), tsaf-parser
//! (MIT, Copyright (c) 2026 Arlo O'Keeffe) and whats-now-playing's djay Pro input (MIT,
//! Copyright (c) 2021-2026 Allen Wittenauer), which found the key order and
//! the playlist view.

pub mod bookmark;
pub mod tsaf;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use opendj_engine::beats::{BeatGrid, GridMarker};
use opendj_engine::import::{ImportedEntry, ImportedList, ImportedListKind};
use opendj_engine::{Hotcue, LibraryInfo, Marker, MarkerKind, Source, Track, MEMORY_CUE};
use rusqlite::{Connection, OpenFlags};

use crate::{ImportedLibrary, LibraryImport, ListGroup};
use tsaf::{Obj, Value};

pub const PLAYLISTS: &str = "playlists";

pub struct Djay;

/// djay Pro's library where djay keeps it (or `OPENDJ_DJAY_LIBRARY`).
pub fn default_database() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("OPENDJ_DJAY_LIBRARY") {
        return Some(PathBuf::from(p));
    }
    let home = opendj_engine::paths::home_dir()?;
    [
        "Music/djay/djay Media Library.djayMediaLibrary/MediaLibrary.db",
        "Music/djay/djay Media Library/MediaLibrary.db",
    ]
    .iter()
    .map(|p| home.join(p))
    .find(|p| p.is_file())
}

impl LibraryImport for Djay {
    fn id(&self) -> &str {
        "djay"
    }

    fn title(&self) -> &str {
        "djay Pro"
    }

    fn database(&self) -> Option<PathBuf> {
        default_database()
    }

    fn list_groups(&self) -> Vec<ListGroup> {
        vec![ListGroup {
            id: PLAYLISTS,
            title: "Playlists",
            detail: "Your djay playlists.",
            on_by_default: true,
        }]
    }

    fn stamp(&self, database: &Path) -> String {
        crate::mixxx::sqlite_stamp(database)
    }

    fn preview(&self, database: &Path) -> Result<String, String> {
        let db = open(database)?;
        let tracks: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM database2 WHERE collection = 'localMediaItemLocations'",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let lists = playlist_names(&db).len();
        let n = |count: usize, one: &str, many: &str| {
            format!("{count} {}", if count == 1 { one } else { many })
        };
        Ok(format!(
            "{} and {}.",
            n(tracks as usize, "track", "tracks"),
            n(lists, "playlist", "playlists")
        ))
    }

    fn read(&self, database: &Path, groups: &[&str]) -> Result<ImportedLibrary, String> {
        let library = read(database, groups, Path::exists)?;
        eprintln!(
            "djay library: {} tracks here, {} lists",
            library.tracks.len(),
            library.lists.len()
        );
        Ok(library)
    }
}

/// Opens djay's database read-only. When SQLite can't read it in place (djay
/// left its write-ahead log without the index SQLite needs to read it
/// read-only), it reads a copy.
fn open(path: &Path) -> Result<Connection, String> {
    let try_open = |p: &Path| -> Result<Connection, String> {
        let db = Connection::open_with_flags(
            p,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| e.to_string())?;
        db.query_row("SELECT COUNT(*) FROM database2 LIMIT 1", [], |r| {
            r.get::<_, i64>(0)
        })
        .map_err(|e| e.to_string())?;
        Ok(db)
    };
    match try_open(path) {
        Ok(db) => Ok(db),
        Err(first) => {
            let dir = std::env::temp_dir().join(format!("opendj-djay-{}", std::process::id()));
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let copy = dir.join("MediaLibrary.db");
            let side = |p: &Path, s: &str| {
                let mut o = p.as_os_str().to_owned();
                o.push(s);
                PathBuf::from(o)
            };
            std::fs::copy(path, &copy)
                .map_err(|_| format!("can't open djay's library: {first}"))?;
            for s in ["-wal", "-shm"] {
                let _ = std::fs::remove_file(side(&copy, s));
                if side(path, s).is_file() {
                    let _ = std::fs::copy(side(path, s), side(&copy, s));
                }
            }
            let db = Connection::open(&copy)
                .and_then(|db| {
                    db.query_row("SELECT COUNT(*) FROM database2", [], |r| r.get::<_, i64>(0))?;
                    Ok(db)
                })
                .map_err(|e| format!("can't open djay's library: {e}"))?;
            Ok(db)
        }
    }
}

/// What djay holds about one track, by collection.
#[derive(Default)]
struct Item {
    title: Option<Obj>,
    location: Option<Obj>,
    analyzed: Option<Obj>,
    user: Option<Obj>,
}

const COLLECTIONS: [&str; 5] = [
    "mediaItemTitleIDs",
    "localMediaItemLocations",
    "globalMediaItemLocations",
    "mediaItemAnalyzedData",
    "mediaItemUserData",
];

/// The tracks whose files `exists` says are here, and the playlists in
/// `groups`.
pub fn read(
    path: &Path,
    groups: &[&str],
    exists: impl Fn(&Path) -> bool,
) -> Result<ImportedLibrary, String> {
    let db = open(path)?;
    let mut items: HashMap<String, Item> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    {
        let mut stmt = db
            .prepare(
                "SELECT collection, key, data FROM database2 \
                 WHERE collection IN (?1, ?2, ?3, ?4, ?5) ORDER BY rowid",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(COLLECTIONS, |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<Vec<u8>>>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (collection, key, data) = row.map_err(|e| e.to_string())?;
            // A record djay wrote in a way this reader doesn't know is left
            // out rather than stopping the import.
            let Some(Value::Obj(obj)) = data.and_then(|d| tsaf::parse(&d).ok()) else {
                continue;
            };
            if !items.contains_key(&key) {
                order.push(key.clone());
            }
            let item = items.entry(key).or_default();
            match collection.as_str() {
                "mediaItemTitleIDs" => item.title = Some(obj),
                "localMediaItemLocations" => item.location = Some(obj),
                "globalMediaItemLocations" => {
                    item.location.get_or_insert(obj);
                }
                "mediaItemAnalyzedData" => item.analyzed = Some(obj),
                _ => item.user = Some(obj),
            }
        }
    }
    let mut files: HashMap<String, PathBuf> = HashMap::new();
    let mut tracks = Vec::new();
    for key in &order {
        let item = &items[key];
        let Some(file) = item.location.as_ref().and_then(location) else {
            continue;
        };
        if exists(&file) {
            tracks.push(track(item, file.clone()));
        }
        files.insert(key.clone(), file);
    }
    let lists = if groups.contains(&PLAYLISTS) {
        playlists(&db, &files)
    } else {
        Vec::new()
    };
    Ok(ImportedLibrary { tracks, lists })
}

/// The file a location names: a `file://` URL, or a macOS bookmark.
fn location(obj: &Obj) -> Option<PathBuf> {
    fn find(v: &Value) -> Option<PathBuf> {
        match v {
            Value::Url(s) | Value::Str(s) => file_url(s),
            Value::Data(d) => bookmark::path(d),
            Value::Arr(items) => items.iter().find_map(find),
            _ => None,
        }
    }
    ["sourceURIs", "urlBookmarkData"]
        .iter()
        .filter_map(|n| obj.get(n))
        .chain(obj.fields.iter().map(|(_, v)| v))
        .find_map(find)
}

/// The path in a `file://` URL, its %-escapes decoded.
fn file_url(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    // "file://localhost/Users/…" and "file:///Users/…".
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let Some(b) = bytes
                .get(i + 1..i + 3)
                .and_then(|h| std::str::from_utf8(h).ok())
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    let path = String::from_utf8(out).ok()?;
    // Windows: "/C:/Music/a.mp3".
    let path = match path.as_bytes() {
        [b'/', d, b':', ..] if d.is_ascii_alphabetic() => path[1..].to_string(),
        _ => path,
    };
    (path.len() > 1).then(|| PathBuf::from(path))
}

/// djay's `keySignatureIndex`: even numbers the major keys from D flat up
/// by semitones, each odd one the minor key of the major before it.
fn key_name(index: f64) -> Option<String> {
    const NOTES: [&str; 12] = [
        "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
    ];
    if !(0.0..24.0).contains(&index) || index.fract() != 0.0 {
        return None;
    }
    let i = index as usize;
    let major = (1 + i / 2) % 12;
    Some(if i % 2 == 1 {
        format!("{}m", NOTES[(major + 9) % 12])
    } else {
        NOTES[major].to_string()
    })
}

/// djay's colors by `colorIndex`, for tracks and cues. Not yet checked
/// against a real library: the order follows djay's color menu.
fn color(index: f64) -> Option<u32> {
    match index as i64 {
        1 => Some(0xE5_3935), // red
        2 => Some(0xFB_8C00), // orange
        3 => Some(0xFD_D835), // yellow
        4 => Some(0x43_A047), // green
        5 => Some(0x00_ACC1), // cyan
        6 => Some(0x1E_88E5), // blue
        7 => Some(0x8E_24AA), // purple
        8 => Some(0xD8_1B60), // pink
        _ => None,
    }
}

/// A cue point as a hot cue numbered by its pad (0 to 7, or `None`): its
/// time, where its loop ends (if it is one), name and color.
fn cue(c: &Obj) -> Option<(Option<usize>, Hotcue)> {
    let time = c.number("time").filter(|t| t.is_finite() && *t >= 0.0)?;
    let end = c.number("endTime").filter(|e| e.is_finite() && *e > time);
    let pad = c
        .number("number")
        .filter(|n| (0.0..8.0).contains(n))
        .map(|n| n as usize);
    let name = c.text("comment");
    let hotcue = Hotcue {
        number: 0,
        seconds: time,
        name,
        end,
        color: c.number("colorIndex").and_then(color),
    };
    Some((pad, hotcue))
}

fn track(item: &Item, file: PathBuf) -> Track {
    // The title ID is its own record, and is copied into the others.
    let title = item.title.as_ref().cloned().or_else(|| {
        [&item.location, &item.analyzed, &item.user]
            .into_iter()
            .flatten()
            .find_map(|o| {
                o.get("titleIDs")?
                    .items()
                    .iter()
                    .find_map(|v| v.as_obj().cloned())
            })
    });
    let title = title.unwrap_or_default();
    let analyzed = item.analyzed.clone().unwrap_or_default();
    let user = item.user.clone().unwrap_or_default();

    let mut hotcues: Vec<Hotcue> = Vec::new();
    let mut memory = MEMORY_CUE;
    let cue_points = user.get("cuePoints").map(Value::items).unwrap_or_default();
    // Saved loops, if djay keeps them apart from the cue points.
    let loops = user
        .fields
        .iter()
        .filter(|(n, _)| {
            n.as_deref()
                .is_some_and(|n| n.to_lowercase().contains("loop"))
        })
        .flat_map(|(_, v)| v.items());
    for (from_loops, c) in cue_points
        .iter()
        .map(|c| (false, c))
        .chain(loops.map(|c| (true, c)))
    {
        let Some((pad, mut hotcue)) = c.as_obj().and_then(cue) else {
            continue;
        };
        hotcue.number = match pad {
            Some(p) if !from_loops && !hotcues.iter().any(|h| h.number == p) => p,
            _ => {
                memory += 1;
                memory - 1
            }
        };
        hotcues.push(hotcue);
    }
    hotcues.sort_by_key(|h| h.number);

    let mut markers = Vec::new();
    for (field, name) in [
        ("automixStartPoint", "realStart"),
        ("automixEndPoint", "fadeStart"),
    ] {
        let seconds = match user.get(field) {
            Some(Value::Obj(o)) => o.number("time"),
            Some(v) => v.as_f64(),
            None => None,
        };
        if let Some(seconds) = seconds.filter(|s| s.is_finite() && *s >= 0.0) {
            markers.push(Marker {
                kind: MarkerKind::Automix,
                seconds,
                end: None,
                name: name.into(),
                color: None,
                detected: false,
            });
        }
    }

    let bpm = analyzed
        .number("bpm")
        .filter(|b| b.is_finite() && *b > 0.0)
        .unwrap_or(0.0);
    let anchor = user
        .get("beatGridEdits")
        .and_then(Value::as_obj)
        .and_then(|g| g.number("firstDownbeatPosition"))
        .filter(|s| s.is_finite() && *s >= 0.0);
    let mut t = Track {
        id: 0,
        artist: title.text("artist"),
        title: title.text("title"),
        album: title.text("album"),
        genre: String::new(),
        rating: 0,
        bpm,
        key: analyzed
            .number("keySignatureIndex")
            .and_then(key_name)
            .unwrap_or_default(),
        duration: title
            .number("duration")
            .filter(|d| d.is_finite())
            .unwrap_or(0.0)
            .max(0.0),
        first_beat: anchor.unwrap_or(0.0),
        grid: Default::default(),
        source: Source::File(file),
        waveform: Arc::default(),
        audio: None,
        cover: None,
        info: LibraryInfo {
            color: user.number("colorIndex").and_then(color),
            play_count: user.number("playCount").unwrap_or(0.0).max(0.0) as u32,
            hotcues,
            isrc: title.text("isrc"),
            fixed_grid: anchor.is_some() && bpm > 0.0,
            markers,
            ..Default::default()
        },
    };
    if let (Some(seconds), true) = (anchor, bpm > 0.0) {
        t.set_grid(BeatGrid::new(vec![GridMarker { seconds, bpm }]));
    }
    t
}

const PAGES: &str = "view_mediaItemPlaylistView_page";
const MAP: &str = "view_mediaItemPlaylistView_map";

/// Each playlist's name as the DJ sees it, by the view's `group`. A group
/// may be a playlist's uuid; then its name is looked up in djay's playlist
/// records.
fn playlist_names(db: &Connection) -> Vec<(String, String)> {
    let groups: Vec<String> = db
        .prepare(&format!(
            "SELECT DISTINCT \"group\" FROM {PAGES} WHERE \"group\" IS NOT NULL AND \"group\" != ''"
        ))
        .and_then(|mut s| {
            s.query_map([], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or_default();
    if groups.is_empty() {
        return Vec::new();
    }
    let mut by_uuid: HashMap<String, String> = HashMap::new();
    if groups.iter().any(|g| is_uuid(g)) {
        let rows: Vec<(String, Option<Vec<u8>>)> = db
            .prepare("SELECT key, data FROM database2 WHERE collection LIKE '%laylist%'")
            .and_then(|mut s| {
                s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                    .collect::<Result<Vec<_>, _>>()
            })
            .unwrap_or_default();
        for (key, data) in rows {
            if let Some(Value::Obj(o)) = data.and_then(|d| tsaf::parse(&d).ok()) {
                let name = Some(o.text("name"))
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| o.text("title"));
                if !name.is_empty() {
                    let uuid = Some(o.text("uuid"))
                        .filter(|u| !u.is_empty())
                        .unwrap_or(key);
                    by_uuid.insert(uuid, name);
                }
            }
        }
    }
    let mut out: Vec<(String, String)> = groups
        .into_iter()
        .map(|g| {
            let name = by_uuid.get(&g).cloned().unwrap_or_else(|| g.clone());
            (g, name)
        })
        .collect();
    out.sort_by_key(|(_, name)| name.to_lowercase());
    out
}

fn is_uuid(s: &str) -> bool {
    let hex: String = s.chars().filter(|c| *c != '-').collect();
    hex.len() == 32 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

/// A page of the playlist view: a run of one playlist's rows.
struct Page {
    key: String,
    group: String,
    prev: Option<String>,
    data: Option<Vec<u8>>,
}

/// The rows of a page, in order: YapDatabase writes them as 64-bit row ids.
fn page_rows(data: &[u8]) -> Vec<i64> {
    if data.starts_with(b"bplist") || !data.len().is_multiple_of(8) {
        return Vec::new();
    }
    data.chunks_exact(8)
        .map(|c| i64::from_le_bytes(c.try_into().expect("8 bytes")))
        .collect()
}

/// djay's playlists, each track in the order djay shows it. Tracks that
/// aren't files (streamed ones) stay out.
fn playlists(db: &Connection, files: &HashMap<String, PathBuf>) -> Vec<ImportedList> {
    // Every row in a playlist: its page's group and the track's key.
    let members: Vec<(i64, String, String)> = db
        .prepare(&format!(
            "SELECT m.rowid, p.\"group\", d.key FROM {MAP} m \
             JOIN {PAGES} p ON p.pageKey = m.pageKey \
             JOIN database2 d ON d.rowid = m.rowid ORDER BY m.rowid"
        ))
        .and_then(|mut s| {
            s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or_default();
    let key_of: HashMap<i64, &str> = members.iter().map(|(r, _, k)| (*r, k.as_str())).collect();
    // Pages in order within each group, following prevPageKey.
    let pages: Vec<Page> = db
        .prepare(&format!(
            "SELECT pageKey, \"group\", prevPageKey, data FROM {PAGES}"
        ))
        .and_then(|mut s| {
            s.query_map([], |r| {
                Ok(Page {
                    key: r.get(0)?,
                    group: r.get(1)?,
                    prev: r.get(2)?,
                    data: r.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or_default();

    let mut out = Vec::new();
    for (group, name) in playlist_names(db) {
        let mut rows: Vec<i64> = Vec::new();
        let in_group: Vec<_> = pages.iter().filter(|p| p.group == group).collect();
        let mut prev: Option<&str> = None;
        for _ in 0..in_group.len() {
            let Some(page) = in_group
                .iter()
                .find(|p| p.prev.as_deref().filter(|k| !k.is_empty()) == prev)
            else {
                break;
            };
            rows.extend(
                page_rows(page.data.as_deref().unwrap_or_default())
                    .into_iter()
                    .filter(|r| key_of.contains_key(r)),
            );
            prev = Some(page.key.as_str());
        }
        rows.dedup();
        // Rows the pages didn't give in order come after, by row id.
        let seen: HashSet<i64> = rows.iter().copied().collect();
        for (r, g, _) in &members {
            if *g == group && !seen.contains(r) {
                rows.push(*r);
            }
        }
        let entries: Vec<ImportedEntry> = rows
            .iter()
            .filter_map(|r| files.get(key_of[r]))
            .map(|f| ImportedEntry::new(f.clone(), None))
            .collect();
        out.push(ImportedList {
            folders: Vec::new(),
            name,
            kind: ImportedListKind::List,
            entries,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::tsaf::write::{obj, Writer};
    use super::*;

    fn title_id(uuid: &str, title: &str, artist: &str, duration: f32) -> Value {
        obj(
            "ADCMediaItemTitleID",
            vec![
                ("uuid", Value::Str(uuid.into())),
                ("title", Value::Str(title.into())),
                ("artist", Value::Str(artist.into())),
                ("duration", Value::F32(duration)),
            ],
        )
    }

    fn cue(time: f32, end: f32, number: i64, comment: &str) -> Value {
        let mut f = vec![("time", Value::F32(time)), ("endTime", Value::F32(end))];
        if !comment.is_empty() {
            f.push(("comment", Value::Str(comment.into())));
        }
        f.push(("number", Value::Int(number)));
        obj("ADCCuePoint", f)
    }

    /// A small djay library: two local tracks (one by URL, one by
    /// bookmark), one streamed, a playlist with all three.
    fn library(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("opendj-djay-{name}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let db = Connection::open(&path).unwrap();
        db.execute_batch(
            "CREATE TABLE database2 (rowid INTEGER PRIMARY KEY, collection CHAR NOT NULL, \
               key CHAR NOT NULL, data BLOB, metadata BLOB);
             CREATE TABLE view_mediaItemPlaylistView_map (rowid INTEGER PRIMARY KEY, pageKey CHAR NOT NULL);
             CREATE TABLE view_mediaItemPlaylistView_page (pageKey CHAR NOT NULL PRIMARY KEY, \
               \"group\" CHAR NOT NULL, prevPageKey CHAR, count INTEGER, data BLOB);",
        )
        .unwrap();
        let mut rowid = 0;
        let mut put = |collection: &str, key: &str, v: Value| -> i64 {
            rowid += 1;
            db.execute(
                "INSERT INTO database2 (rowid, collection, key, data) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![rowid, collection, key, Writer::blob(&v)],
            )
            .unwrap();
            rowid
        };
        let a = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let b = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let s = "cccccccccccccccccccccccccccccccc";
        let ra = put("mediaItemTitleIDs", a, title_id(a, "Glue", "Bicep", 269.0));
        let rb = put("mediaItemTitleIDs", b, title_id(b, "Just", "Bicep", 372.5));
        let rs = put("mediaItemTitleIDs", s, title_id(s, "Streamed", "X", 200.0));
        put(
            "localMediaItemLocations",
            a,
            obj(
                "ADCMediaItemLocation",
                vec![
                    ("uuid", Value::Str(a.into())),
                    (
                        "sourceURIs",
                        Value::Arr(vec![Value::Url(
                            "file:///Users/dj/Music/Bicep%20-%20Glue.mp3".into(),
                        )]),
                    ),
                ],
            ),
        );
        put(
            "localMediaItemLocations",
            b,
            obj(
                "ADCMediaItemLocation",
                vec![
                    ("uuid", Value::Str(b.into())),
                    (
                        "titleIDs",
                        Value::Arr(vec![title_id(b, "Just", "Bicep", 372.5)]),
                    ),
                    (
                        "sourceURIs",
                        Value::Data(bookmark::make(&["Volumes", "USB", "Just.mp3"])),
                    ),
                ],
            ),
        );
        put(
            "mediaItemAnalyzedData",
            a,
            obj(
                "ADCMediaItemAnalyzedData",
                vec![
                    ("uuid", Value::Str(a.into())),
                    ("bpm", Value::F32(120.0)),
                    ("keySignatureIndex", Value::Int(15)),
                    ("isStraightGrid", Value::Bool(true)),
                ],
            ),
        );
        let automix = |t: f32| cue(t, -1.0, 1, "");
        put(
            "mediaItemUserData",
            a,
            obj(
                "ADCMediaItemUserData",
                vec![
                    ("uuid", Value::Str(a.into())),
                    (
                        "userChangedCloudKeys",
                        Value::Arr(vec![Value::Str("cuePoints".into())]),
                    ),
                    (
                        "titleIDs",
                        Value::Arr(vec![title_id(a, "Glue", "Bicep", 269.0)]),
                    ),
                    (
                        "cuePoints",
                        Value::Arr(vec![
                            cue(0.5, -1.0, 0, "First Beat"),
                            cue(64.0, 72.0, 3, ""),
                            cue(96.0, -1.0, 1, "Drop"),
                        ]),
                    ),
                    (
                        "beatGridEdits",
                        obj(
                            "ADCBeatGridEdits",
                            vec![("firstDownbeatPosition", Value::F32(0.25))],
                        ),
                    ),
                    ("automixStartPoint", automix(16.0)),
                    ("automixEndPoint", automix(240.0)),
                    ("playCount", Value::Int(7)),
                    ("colorIndex", Value::Int(4)),
                ],
            ),
        );
        // A record this reader can't follow is skipped.
        db.execute(
            "INSERT INTO database2 (collection, key, data) VALUES ('mediaItemUserData', ?1, X'545341460000')",
            [b],
        )
        .unwrap();
        // One playlist over two pages, in an order other than row ids.
        for (r, page) in [(rb, "p1"), (rs, "p1"), (ra, "p2")] {
            db.execute(
                "INSERT INTO view_mediaItemPlaylistView_map (rowid, pageKey) VALUES (?1, ?2)",
                rusqlite::params![r, page],
            )
            .unwrap();
        }
        let rows = |r: &[i64]| r.iter().flat_map(|r| r.to_le_bytes()).collect::<Vec<u8>>();
        db.execute(
            "INSERT INTO view_mediaItemPlaylistView_page VALUES ('p2', 'Friday', 'p1', 1, ?1)",
            [rows(&[ra])],
        )
        .unwrap();
        db.execute(
            "INSERT INTO view_mediaItemPlaylistView_page VALUES ('p1', 'Friday', NULL, 2, ?1)",
            [rows(&[rb, rs])],
        )
        .unwrap();
        path
    }

    #[test]
    fn reads_a_djay_library() {
        let path = library("read");
        let lib = read(&path, &[PLAYLISTS], |_| true).unwrap();
        assert_eq!(preview(&path), "2 tracks and 1 playlist.");
        std::fs::remove_file(&path).ok();

        assert_eq!(lib.tracks.len(), 2);
        let t = &lib.tracks[0];
        assert!(
            matches!(&t.source, Source::File(p) if p == Path::new("/Users/dj/Music/Bicep - Glue.mp3"))
        );
        assert_eq!((t.artist.as_str(), t.title.as_str()), ("Bicep", "Glue"));
        assert_eq!((t.bpm, t.key.as_str(), t.duration), (120.0, "Fm", 269.0));
        let i = &t.info;
        assert_eq!((i.play_count, i.color), (7, Some(0x43_A047)));
        assert!(i.fixed_grid);
        assert_eq!(t.grid.markers().len(), 1);
        assert_eq!(t.first_beat, 0.25);
        let pads: Vec<(usize, &str, Option<f64>)> = i
            .hotcues
            .iter()
            .map(|h| (h.number, h.name.as_str(), h.end))
            .collect();
        assert_eq!(
            pads,
            [
                (0, "First Beat", None),
                (1, "Drop", None),
                (3, "", Some(72.0))
            ]
        );
        let automix: Vec<(&str, f64)> = i
            .markers
            .iter()
            .map(|m| (m.name.as_str(), m.seconds))
            .collect();
        assert_eq!(automix, [("realStart", 16.0), ("fadeStart", 240.0)]);

        // Its title from the location's copy; its file from a bookmark.
        let t = &lib.tracks[1];
        assert_eq!(t.title, "Just");
        assert!(matches!(&t.source, Source::File(p) if p == Path::new("/Volumes/USB/Just.mp3")));
        assert!(t.info.hotcues.is_empty() && !t.info.fixed_grid);

        assert_eq!(lib.lists.len(), 1);
        assert_eq!(lib.lists[0].name, "Friday");
        let files: Vec<&Path> = lib.lists[0]
            .entries
            .iter()
            .map(|e| e.path.as_path())
            .collect();
        assert_eq!(
            files,
            [
                Path::new("/Volumes/USB/Just.mp3"),
                Path::new("/Users/dj/Music/Bicep - Glue.mp3")
            ]
        );
    }

    fn preview(path: &Path) -> String {
        Djay.preview(path).unwrap()
    }

    #[test]
    fn leaves_out_missing_files_and_lists() {
        let path = library("missing");
        let lib = read(&path, &[], |p| p.starts_with("/Volumes")).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(lib.tracks.len(), 1);
        assert!(lib.lists.is_empty());
    }

    #[test]
    fn keys_and_urls() {
        assert_eq!(key_name(0.0).as_deref(), Some("Db"));
        assert_eq!(key_name(1.0).as_deref(), Some("Bbm"));
        assert_eq!(key_name(22.0).as_deref(), Some("C"));
        assert_eq!(key_name(23.0).as_deref(), Some("Am"));
        assert_eq!(key_name(11.0).as_deref(), Some("Ebm"));
        assert_eq!(key_name(24.0), None);
        assert_eq!(
            file_url("file:///C:/Music/a%C3%A9.mp3"),
            Some(PathBuf::from("C:/Music/aé.mp3"))
        );
        assert_eq!(
            file_url("file://localhost/Users/x.mp3"),
            Some(PathBuf::from("/Users/x.mp3"))
        );
        assert_eq!(file_url("https://tidal.com/x"), None);
    }
}
