//! `exportLibrary.db` (OneLibrary), read as EngineX's `sqlite.go` reads a
//! plain SQLite copy (see the module notes for the licence): by column
//! names, with the spellings rekordbox and others use, not rbox's models.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::types::ValueRef;
use rusqlite::Connection;

use super::{Library, Playlist, Track};

/// The SQLCipher passphrase every OneLibrary stick shares, as published by
/// pyrekordbox (each byte one higher, so it isn't a plain string here).
fn passphrase() -> String {
    "s9heeos5l958941bs7dr{cll1fm7rzunc4usccy916kn85wf{75j6p9gosrszrmt"
        .bytes()
        .map(|b| (b - 1) as char)
        .collect()
}

fn normalized(s: &str) -> String {
    s.replace(['_', '-', ' '], "").to_lowercase()
}

struct Row(HashMap<String, String>);

impl Row {
    fn get(&self, names: &[&str]) -> String {
        names
            .iter()
            .find_map(|n| self.0.get(&normalized(n)).filter(|v| !v.is_empty()))
            .cloned()
            .unwrap_or_default()
    }

    fn num(&self, names: &[&str]) -> u32 {
        self.get(names).parse().unwrap_or(0)
    }

    fn decimal(&self, names: &[&str]) -> f64 {
        self.get(names).parse().unwrap_or(0.0)
    }
}

fn rows(db: &Connection, table: &str) -> Result<Vec<Row>, String> {
    let e = |e: rusqlite::Error| format!("{table}: {e}");
    let mut st = db
        .prepare(&format!("SELECT * FROM \"{}\"", table.replace('"', "\"\"")))
        .map_err(e)?;
    let cols: Vec<String> = st.column_names().iter().map(|c| normalized(c)).collect();
    let mut q = st.query([]).map_err(e)?;
    let mut out = Vec::new();
    while let Some(r) = q.next().map_err(e)? {
        let mut row = HashMap::new();
        for (i, c) in cols.iter().enumerate() {
            let v = match r.get_ref(i).map_err(e)? {
                ValueRef::Null => continue,
                ValueRef::Integer(n) => n.to_string(),
                ValueRef::Real(x) => x.to_string(),
                ValueRef::Text(t) | ValueRef::Blob(t) => String::from_utf8_lossy(t).into_owned(),
            };
            row.insert(c.clone(), v);
        }
        out.push(Row(row));
    }
    Ok(out)
}

/// Opens the encrypted database read-only and reads it.
pub fn read(path: &Path) -> Result<Library, String> {
    let db = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    db.pragma_update(None, "key", passphrase())
        .map_err(|e| e.to_string())?;
    read_db(&db)
}

pub fn read_db(db: &Connection) -> Result<Library, String> {
    let mut tables = HashMap::new();
    {
        let mut st = db
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .map_err(|e| format!("not a OneLibrary database: {e}"))?;
        let names = st
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        for n in names {
            let n = n.map_err(|e| e.to_string())?;
            tables.insert(normalized(&n), n);
        }
    }
    let load = |names: &[&str]| -> Result<Option<Vec<Row>>, String> {
        for n in names {
            if let Some(t) = tables.get(&normalized(n)) {
                return rows(db, t).map(Some);
            }
        }
        Ok(None)
    };
    let mut lookups: HashMap<&str, HashMap<u32, String>> = HashMap::new();
    for kind in [
        "artist", "album", "genre", "label", "key", "artwork", "image",
    ] {
        let mut m = HashMap::new();
        for r in load(&[kind, &format!("djmd{kind}")])?.unwrap_or_default() {
            m.insert(
                r.num(&["id", &format!("{kind}_id")]),
                r.get(&["name", "title", "path", "file_path"]),
            );
        }
        lookups.insert(kind, m);
    }
    let mut lib = Library {
        keys: lookups["key"].clone(),
        artwork: if lookups["artwork"].is_empty() {
            lookups["image"].clone()
        } else {
            lookups["artwork"].clone()
        },
        ..Library::default()
    };
    let name = |kind: &str, id: u32| lookups[kind].get(&id).cloned().unwrap_or_default();
    let contents = load(&["content", "djmdContent", "track", "tracks"])?
        .ok_or("unsupported schema: missing content table")?;
    for r in contents {
        let mut path = r.get(&["file_path", "path", "location"]);
        let file_name = r.get(&["file_name", "filename"]);
        if path.is_empty() && !file_name.is_empty() {
            path = format!(
                "{}/{file_name}",
                r.get(&["folder_path"]).trim_end_matches('/')
            );
        }
        let mut bpm = r.decimal(&["bpmx100", "bpm", "tempo"]);
        if !r.get(&["bpmx100"]).is_empty() || bpm > 1000.0 {
            bpm /= 100.0;
        }
        let mut key = r.get(&["key_name"]);
        if key.is_empty() {
            key = name("key", r.num(&["key_id"]));
            if key.is_empty() {
                key = r.get(&["key"]);
            }
        }
        let mut t = Track {
            id: r.num(&["id", "content_id", "track_id"]),
            path,
            analysis_path: r.get(&[
                "analysisDataFilePath",
                "analysis_data_path",
                "analyze_path",
                "analysis_path",
            ]),
            title: r.get(&["title", "name"]),
            artist: name("artist", r.num(&["artist_id_artist", "artist_id"])),
            composer: name("artist", r.num(&["artist_id_composer", "composer_id"])),
            remixer: name("artist", r.num(&["artist_id_remixer", "remixer_id"])),
            album: name("album", r.num(&["album_id"])),
            genre: name("genre", r.num(&["genre_id"])),
            label: name("label", r.num(&["label_id"])),
            key,
            bpm,
            duration: r.decimal(&["length", "duration"]),
            sample_rate: r.num(&["samplingRate", "sample_rate"]),
            bit_depth: r.num(&["bit_depth"]),
            bitrate: r.num(&["bit_rate", "bitrate"]),
            file_size: r.decimal(&["file_size"]) as u64,
            year: r.num(&["release_year", "year"]),
            rating: r.num(&["rating"]),
            comment: r.get(&["djComment", "commnt", "comment"]),
            artwork_path: String::new(),
        };
        t.artwork_path = lib
            .artwork
            .get(&r.num(&["image_id", "artwork_id"]))
            .cloned()
            .unwrap_or_default();
        if t.artist.is_empty() {
            t.artist = r.get(&["artist"]);
        }
        if t.album.is_empty() {
            t.album = r.get(&["album"]);
        }
        lib.tracks.push(t);
    }
    for r in load(&["playlist", "djmdPlaylist"])?.unwrap_or_default() {
        let attr = r.num(&["attribute"]);
        lib.playlists.push(Playlist {
            id: r.num(&["id", "playlist_id"]),
            parent_id: r.num(&["playlist_id_parent", "parent_id"]),
            sort_order: r.num(&["sequenceNo", "seq", "sort_order"]),
            name: r.get(&["name", "title"]),
            folder: r.num(&["is_folder"]) != 0 || attr == 1,
            tracks: Vec::new(),
        });
    }
    let entries = load(&[
        "playlist_content",
        "djmdSongPlaylist",
        "playlist_entry",
        "playlist_entries",
    ])?
    .unwrap_or_default();
    let mut grouped: HashMap<u32, Vec<(u32, u32)>> = HashMap::new();
    for r in entries {
        grouped.entry(r.num(&["playlist_id"])).or_default().push((
            r.num(&["sequenceNo", "track_no", "seq", "entry_index", "position"]),
            r.num(&["content_id", "track_id"]),
        ));
    }
    for p in &mut lib.playlists {
        let mut xs = grouped.remove(&p.id).unwrap_or_default();
        xs.sort_by_key(|x| x.0);
        p.tracks = xs.into_iter().map(|x| x.1).collect();
    }
    lib.validate()?;
    Ok(lib)
}
