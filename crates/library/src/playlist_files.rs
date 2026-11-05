//! Playlist files: lists out as extended M3U (`.m3u8`), and M3U, M3U8, PLS
//! and XSPF files in.
//!
//! Only the text is handled here; the app decides which tracks go out and
//! matches what comes in to its collection (see [`candidates`]).

use std::path::{Component, Path, PathBuf};

/// One line of a list going out.
#[derive(Debug, Clone, PartialEq)]
pub enum OutEntry {
    /// A file, with what `#EXTINF` says about it.
    File {
        path: PathBuf,
        artist: String,
        title: String,
        /// Seconds; 0 or less when not known.
        duration: f64,
    },
    /// A track with no file here (a streamed one): written as a comment, so
    /// other apps skip it and a person reading the file still sees it.
    Missing {
        artist: String,
        title: String,
        service: String,
    },
}

/// An extended M3U file in UTF-8. With `relative_to`, paths of files in
/// that folder or below it are written relative to it (`/` between
/// folders, as every app reads); the rest stay absolute.
pub fn write_m3u8(entries: &[OutEntry], relative_to: Option<&Path>) -> String {
    let mut out = String::from("#EXTM3U\n");
    for e in entries {
        match e {
            OutEntry::File {
                path,
                artist,
                title,
                duration,
            } => {
                let secs = if *duration > 0.0 {
                    duration.round() as i64
                } else {
                    -1
                };
                out.push_str(&format!(
                    "#EXTINF:{secs},{}\n",
                    one_line(&name(artist, title))
                ));
                let shown = relative_to
                    .and_then(|base| path.strip_prefix(base).ok())
                    .map(|rel| {
                        rel.components()
                            .map(|c| c.as_os_str().to_string_lossy().into_owned())
                            .collect::<Vec<_>>()
                            .join("/")
                    })
                    .unwrap_or_else(|| path.to_string_lossy().into_owned());
                out.push_str(&one_line(&shown));
                out.push('\n');
            }
            OutEntry::Missing {
                artist,
                title,
                service,
            } => {
                let on = if service.is_empty() {
                    String::new()
                } else {
                    format!(" (on {service}, no file)")
                };
                out.push_str(&format!("# {}{on}\n", one_line(&name(artist, title))));
            }
        }
    }
    out
}

fn name(artist: &str, title: &str) -> String {
    match (artist.trim(), title.trim()) {
        ("", t) => t.to_string(),
        (a, "") => a.to_string(),
        (a, t) => format!("{a} - {t}"),
    }
}

fn one_line(s: &str) -> String {
    s.replace(['\r', '\n'], " ")
}

/// A file name from a list's name: characters no file system takes are
/// swapped for `-`, and an empty name is "Playlist".
pub fn file_name(list: &str) -> String {
    let cleaned: String = list
        .trim()
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if cleaned.is_empty() {
        "Playlist".into()
    } else {
        cleaned.to_string()
    }
}

/// A playlist file read in.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReadList {
    /// The XSPF title, else the file's name without its extension.
    pub name: String,
    pub entries: Vec<InEntry>,
}

/// One track a playlist file names.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InEntry {
    /// The location as written: a path (absolute or relative to the file)
    /// or a `file://` address, already decoded.
    pub location: String,
    /// What the file says the track is called, when it says.
    pub title: String,
}

/// The extensions read in, lower case.
pub const EXTENSIONS: [&str; 4] = ["m3u", "m3u8", "pls", "xspf"];

/// Whether `path` looks like a playlist file this module reads.
pub fn is_playlist_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Reads the playlist file at `path`.
pub fn read(path: &Path) -> Result<ReadList, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    // `.m3u` may be in the system's old code page; anything not UTF-8 is
    // read as Latin-1, which keeps every byte.
    let text = match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(e) => e.into_bytes().iter().map(|&b| b as char).collect(),
    };
    let text = text.trim_start_matches('\u{feff}');
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut list = match ext.as_str() {
        "pls" => parse_pls(text),
        "xspf" => parse_xspf(text)?,
        _ => parse_m3u(text),
    };
    if list.name.trim().is_empty() {
        list.name = stem;
    }
    Ok(list)
}

/// M3U and extended M3U: one location per line, `#EXTINF` before it.
pub fn parse_m3u(text: &str) -> ReadList {
    let mut list = ReadList::default();
    let mut title = String::new();
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(info) = line.strip_prefix("#EXTINF:") {
            title = info
                .split_once(',')
                .map(|(_, t)| t.trim().to_string())
                .unwrap_or_default();
            continue;
        }
        if let Some(name) = line.strip_prefix("#PLAYLIST:") {
            list.name = name.trim().to_string();
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        list.entries.push(InEntry {
            location: location(line),
            title: std::mem::take(&mut title),
        });
    }
    list
}

/// PLS: `FileN=` and `TitleN=` under `[playlist]`.
pub fn parse_pls(text: &str) -> ReadList {
    let mut files: std::collections::BTreeMap<u32, InEntry> = Default::default();
    for line in text.lines() {
        let Some((key, value)) = line.trim().split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        if let Some(n) = key.strip_prefix("file").and_then(|n| n.parse().ok()) {
            files.entry(n).or_default().location = location(value);
        } else if let Some(n) = key.strip_prefix("title").and_then(|n| n.parse().ok()) {
            files.entry(n).or_default().title = value.to_string();
        }
    }
    ReadList {
        name: String::new(),
        entries: files
            .into_values()
            .filter(|e| !e.location.is_empty())
            .collect(),
    }
}

/// XSPF: `<track><location>` (a URI) with `<creator>` and `<title>`.
pub fn parse_xspf(text: &str) -> Result<ReadList, String> {
    let doc = roxmltree::Document::parse(text).map_err(|e| e.to_string())?;
    let root = doc.root_element();
    let child_text = |n: roxmltree::Node, name: &str| {
        n.children()
            .find(|c| c.has_tag_name(name))
            .and_then(|c| c.text())
            .map(|t| t.trim().to_string())
            .unwrap_or_default()
    };
    let mut list = ReadList {
        name: child_text(root, "title"),
        entries: Vec::new(),
    };
    for track in root.descendants().filter(|n| n.has_tag_name("track")) {
        let loc = child_text(track, "location");
        if loc.is_empty() {
            continue;
        }
        list.entries.push(InEntry {
            location: location(&loc),
            title: name(&child_text(track, "creator"), &child_text(track, "title")),
        });
    }
    Ok(list)
}

/// A location as a path: `file://` addresses are decoded, anything else
/// is kept as written.
fn location(raw: &str) -> String {
    let raw = raw.trim();
    let Some(rest) = raw
        .strip_prefix("file://")
        .or_else(|| raw.strip_prefix("FILE://"))
    else {
        return raw.to_string();
    };
    // file:///C:/Music → C:/Music; file://localhost/x → /x.
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let decoded = percent_decode(rest);
    let bytes = decoded.as_bytes();
    if bytes.len() > 2 && bytes[0] == b'/' && bytes[2] == b':' && bytes[1].is_ascii_alphabetic() {
        decoded[1..].to_string()
    } else {
        decoded
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = |c: u8| (c as char).to_digit(16);
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Where an entry's file may be, most likely first: the location as an
/// absolute path, then relative to the playlist file's folder. Windows
/// back slashes are read as folder breaks on every system.
pub fn candidates(entry: &InEntry, list_dir: &Path) -> Vec<PathBuf> {
    let loc = entry.location.replace('\\', "/");
    if loc.is_empty() || loc.contains("://") {
        return Vec::new();
    }
    let path = PathBuf::from(&loc);
    let mut out = Vec::new();
    let windows_absolute = loc.len() > 2 && loc.as_bytes()[1] == b':';
    if path.is_absolute() || windows_absolute {
        out.push(path);
    } else {
        out.push(normalize(&list_dir.join(path)));
    }
    out
}

/// `a/./b/../c` → `a/c`, without touching the disk.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

/// The file name an entry points at, for matching by name and size when
/// its folder isn't here.
pub fn entry_file_name(entry: &InEntry) -> Option<String> {
    let loc = entry.location.replace('\\', "/");
    let name = loc.rsplit('/').next()?.trim();
    (!name.is_empty() && !loc.contains("://")).then(|| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_extended_m3u() {
        let entries = vec![
            OutEntry::File {
                path: PathBuf::from("/Volumes/USB/Music/a.mp3"),
                artist: "Artist".into(),
                title: "Song".into(),
                duration: 215.6,
            },
            OutEntry::File {
                path: PathBuf::from("/Users/dj/b.flac"),
                artist: String::new(),
                title: "Untitled".into(),
                duration: 0.0,
            },
            OutEntry::Missing {
                artist: "Someone".into(),
                title: "Streamed".into(),
                service: "Spotify".into(),
            },
        ];
        let text = write_m3u8(&entries, Some(Path::new("/Volumes/USB")));
        assert_eq!(
            text,
            "#EXTM3U\n#EXTINF:216,Artist - Song\nMusic/a.mp3\n\
             #EXTINF:-1,Untitled\n/Users/dj/b.flac\n\
             # Someone - Streamed (on Spotify, no file)\n"
        );
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_name("Fri 10/09: warm-up?"), "Fri 10-09- warm-up-");
        assert_eq!(file_name("  ..  "), "Playlist");
    }

    #[test]
    fn reads_m3u() {
        let l = parse_m3u(
            "\u{feff}#EXTM3U\n#EXTINF:200,A - One\nmusic/one.mp3\n\n# comment\nC:\\Music\\two.mp3\n",
        );
        assert_eq!(l.entries.len(), 2);
        assert_eq!(l.entries[0].title, "A - One");
        assert_eq!(l.entries[1].title, "");
        let c = candidates(&l.entries[0], Path::new("/lists"));
        assert_eq!(c, vec![PathBuf::from("/lists/music/one.mp3")]);
        assert_eq!(entry_file_name(&l.entries[1]).as_deref(), Some("two.mp3"));
    }

    #[test]
    fn reads_pls() {
        let l = parse_pls(
            "[playlist]\nFile2=/b.mp3\nTitle1=First\nFile1=file:///a%20b.mp3\nNumberOfEntries=2\n",
        );
        let locs: Vec<&str> = l.entries.iter().map(|e| e.location.as_str()).collect();
        assert_eq!(locs, ["/a b.mp3", "/b.mp3"]);
        assert_eq!(l.entries[0].title, "First");
    }

    #[test]
    fn reads_xspf() {
        let l = parse_xspf(
            r#"<?xml version="1.0"?>
            <playlist version="1" xmlns="http://xspf.org/ns/0/">
              <title>Friday</title>
              <trackList>
                <track><location>file:///C:/Music/x%C3%A9.flac</location>
                  <creator>Band</creator><title>Tune</title></track>
                <track><title>No location</title></track>
              </trackList>
            </playlist>"#,
        )
        .unwrap();
        assert_eq!(l.name, "Friday");
        assert_eq!(l.entries.len(), 1);
        assert_eq!(l.entries[0].location, "C:/Music/xé.flac");
        assert_eq!(l.entries[0].title, "Band - Tune");
    }

    #[test]
    fn web_addresses_have_no_file() {
        let e = InEntry {
            location: "https://example.org/stream".into(),
            title: String::new(),
        };
        assert!(candidates(&e, Path::new("/")).is_empty());
        assert!(entry_file_name(&e).is_none());
    }

    #[test]
    fn round_trip() {
        let entries = vec![OutEntry::File {
            path: PathBuf::from("/m/Set/a.mp3"),
            artist: "A".into(),
            title: "B".into(),
            duration: 60.0,
        }];
        let text = write_m3u8(&entries, Some(Path::new("/m")));
        let back = parse_m3u(&text);
        assert_eq!(
            candidates(&back.entries[0], Path::new("/m")),
            vec![PathBuf::from("/m/Set/a.mp3")]
        );
    }
}
