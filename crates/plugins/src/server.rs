//! What the sources for a music server the DJ runs (Plex, Navidrome and
//! other Subsonic servers) share: a [`Server`] answers searches with albums,
//! playlists and songs, and [`ServerSource`] turns that into a rail source
//! whose downloads fetch the original files, one after another on a thread
//! of their own, with progress, a cancel and a retry.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::inbox::safe_name;
use crate::source::{
    DownloadUpdate, Found, Progress, SearchSource, Snapshot, SourceInfo, SourceQuery, CACHE_SIZE,
    DOWNLOAD_FOLDER,
};
use crate::{PluginError, Setting, SettingKind, USER_AGENT};

/// A song on the server, with what it takes to download it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Song {
    /// Unique on the server, e.g. "song:123".
    pub key: String,
    pub title: String,
    pub artist: String,
    /// Who the album is by, for its folder; the artist when empty.
    pub album_artist: String,
    pub album: String,
    pub year: String,
    pub number: String,
    pub duration: f64,
    /// The file's extension, e.g. "flac".
    pub suffix: String,
    /// kbps.
    pub bitrate: u64,
    pub size: u64,
    /// The original file. Empty when the server has to be asked
    /// ([`Server::resolve`]).
    pub file_url: String,
    /// The album's picture, full size.
    pub cover_url: String,
    /// The album's picture, small, for the list.
    pub thumb_url: String,
}

impl Song {
    pub fn found(&self, from: &str) -> Found {
        Found {
            key: self.key.clone(),
            artist: self.artist.clone(),
            title: self.title.clone(),
            album: self.album.clone(),
            year: self.year.clone(),
            number: self.number.clone(),
            duration: self.duration,
            format: format_name(&self.suffix),
            quality: kbps(self.bitrate),
            size: self.size,
            from: from.to_string(),
            cover_url: self.thumb_url.clone(),
            file: self
                .files(Path::new(""))
                .into_iter()
                .find(|f| !f.cover)
                .map(|f| f.to.to_string_lossy().into_owned())
                .unwrap_or_default(),
            ..Default::default()
        }
    }

    /// The files it takes: the album's cover, then the song, in its album's
    /// folder under `dir`.
    pub fn files(&self, dir: &Path) -> Vec<File> {
        let artist = if self.album_artist.is_empty() {
            &self.artist
        } else {
            &self.album_artist
        };
        let folder = album_dir(dir, artist, &self.album);
        let mut out = Vec::new();
        if !self.cover_url.is_empty() {
            out.push(File {
                url: self.cover_url.clone(),
                to: folder.join("cover.jpg"),
                size: 0,
                cover: true,
            });
        }
        out.push(File {
            url: self.file_url.clone(),
            to: folder.join(track_name(&self.number, &self.title, &self.suffix)),
            size: self.size,
            cover: false,
        });
        out
    }
}

/// What a search found: folders (albums, playlists) and songs.
#[derive(Clone, Debug, Default)]
pub(crate) struct Results {
    pub folders: Vec<Found>,
    pub songs: Vec<Song>,
}

/// A music server the DJ runs. Every call may block on the network.
pub(crate) trait Server: Send {
    fn info(&self) -> SourceInfo;
    fn describe(&self, query: &SourceQuery) -> Vec<String>;
    /// Why it can't be asked yet, e.g. its address is missing.
    fn missing(&self) -> Option<String>;
    fn search(&self, query: &SourceQuery) -> Result<Results, String>;
    /// The songs in a folder from [`Server::search`], by its key.
    fn folder(&self, key: &str) -> Result<Vec<Song>, String>;
    /// A song with its file's address, when the search didn't give it.
    fn resolve(&self, song: &Song) -> Result<Song, String> {
        Ok(song.clone())
    }
}

/// A [`Server`] as a source in the rail.
pub(crate) struct ServerSource<S> {
    pub server: S,
    searches: HashMap<String, Snapshot>,
    /// Every song listed so far, by key, to download it.
    songs: HashMap<String, Song>,
    next: u64,
    downloads: Downloads,
}

impl<S: Server> ServerSource<S> {
    pub fn new(name: &'static str, server: S) -> Self {
        Self {
            server,
            searches: HashMap::new(),
            songs: HashMap::new(),
            next: 0,
            downloads: Downloads::new(name),
        }
    }

    fn keep(&mut self, songs: &[Song]) -> Vec<Found> {
        let name = self.server.info().name;
        songs
            .iter()
            .map(|s| {
                self.songs.insert(s.key.clone(), s.clone());
                s.found(name)
            })
            .collect()
    }

    /// The files of one song.
    fn song_files(&self, song: &Song, dir: &Path) -> Result<Vec<File>, PluginError> {
        let song = if song.file_url.is_empty() {
            self.server.resolve(song).map_err(PluginError::Network)?
        } else {
            song.clone()
        };
        Ok(song.files(dir))
    }
}

impl<S: Server> SearchSource for ServerSource<S> {
    fn info(&self) -> SourceInfo {
        self.server.info()
    }

    fn plays(&self) -> bool {
        true
    }

    fn describe(&self, query: &SourceQuery) -> Vec<String> {
        if let Some(why) = self.server.missing() {
            return vec![why];
        }
        self.server.describe(query)
    }

    fn start(&mut self, query: &SourceQuery) -> Result<String, PluginError> {
        if let Some(why) = self.server.missing() {
            return Err(PluginError::Network(why));
        }
        let results = self.server.search(query).map_err(PluginError::Network)?;
        let mut items = results.folders;
        items.extend(self.keep(&results.songs));
        self.next += 1;
        let id = self.next.to_string();
        self.searches.insert(
            id.clone(),
            Snapshot {
                items,
                done: true,
                note: String::new(),
                title: String::new(),
            },
        );
        Ok(id)
    }

    fn results(&mut self, id: &str) -> Result<Snapshot, PluginError> {
        self.searches
            .get(id)
            .cloned()
            .ok_or_else(|| PluginError::Format("no such search".into()))
    }

    fn stop(&mut self, id: &str) {
        self.searches.remove(id);
    }

    fn open_folder(&mut self, id: &str, folder: &Found) -> Result<Vec<Found>, PluginError> {
        let songs = self
            .server
            .folder(&folder.key)
            .map_err(PluginError::Network)?;
        let tracks = self.keep(&songs);
        if let Some(s) = self.searches.get_mut(id) {
            if let Some(f) = s.items.iter_mut().find(|f| f.key == folder.key) {
                f.tracks = tracks.clone();
                f.track_count = tracks.len();
                f.size = tracks.iter().map(|t| t.size).sum();
            }
        }
        Ok(tracks)
    }

    fn download(&mut self, _id: &str, items: &[Found], dir: &Path) -> Result<(), PluginError> {
        for f in items {
            let songs = if f.folder {
                let songs = self.server.folder(&f.key).map_err(PluginError::Network)?;
                self.keep(&songs);
                songs
            } else {
                match self.songs.get(&f.key) {
                    Some(s) => vec![s.clone()],
                    None => return Err(PluginError::Format("no such song".into())),
                }
            };
            let mut files: Vec<File> = Vec::new();
            for song in &songs {
                for file in self.song_files(song, dir)? {
                    // An album's cover once.
                    if !files.iter().any(|f| f.to == file.to) {
                        files.push(file);
                    }
                }
            }
            self.downloads.start(&f.key, files)?;
        }
        Ok(())
    }

    fn progress(&mut self) -> Vec<DownloadUpdate> {
        self.downloads.progress()
    }

    fn retry(&mut self, key: &str) -> Result<(), PluginError> {
        self.downloads.retry(key)
    }

    fn cancel(&mut self, key: &str) -> Result<(), PluginError> {
        self.downloads.cancel(key)
    }
}

/// The settings every server source ends with: where Add to Collection
/// puts files, and how big the cache of played tracks may grow.
pub(crate) fn storage_settings(name: &str) -> Vec<Setting> {
    vec![
        Setting {
            key: DOWNLOAD_FOLDER,
            kind: SettingKind::Folder,
            title: "Collection folder".into(),
            detail: format!(
                "Where Add to Collection puts files. Left empty, Downloaded/{name} in your \
                 music folder."
            ),
        },
        Setting {
            key: CACHE_SIZE,
            kind: SettingKind::Text,
            title: "Cache size (GB)".into(),
            detail: format!(
                "Tracks you play or Download from {name} are kept in a cache, outside your \
                 collection, until it is this full; the oldest go first. Left empty, 5 GB."
            ),
        },
    ]
}

/// Whether `name` has `text` in it, ignoring case; an empty text or "*"
/// matches everything.
pub(crate) fn matches(name: &str, text: &str) -> bool {
    let text = text.trim();
    text.is_empty() || text == "*" || name.to_lowercase().contains(&text.to_lowercase())
}

/// One file of a download.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct File {
    pub url: String,
    pub to: PathBuf,
    /// Bytes, when the server says; for the progress bar.
    pub size: u64,
    /// The album's picture: fetched when missing, never fails the download
    /// and isn't one of the files it reports.
    pub cover: bool,
}

/// An agent for a server on the DJ's own network or behind their own
/// address: a long timeout, since a file can be large.
pub(crate) fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_recv_body(Some(Duration::from_secs(600)))
        .user_agent(USER_AGENT)
        .build()
        .into()
}

/// A server address as typed: no trailing slash, `http://` added when
/// there is no scheme. Empty when nothing was typed.
pub(crate) fn server(typed: &str) -> String {
    let s = typed.trim().trim_end_matches('/');
    if s.is_empty() || s.starts_with("http://") || s.starts_with("https://") {
        s.to_string()
    } else {
        format!("http://{s}")
    }
}

/// Where an album's files go: `<dir>/<Artist>/<Album>/`.
pub(crate) fn album_dir(dir: &Path, artist: &str, album: &str) -> PathBuf {
    dir.join(safe_name(artist, "Unknown Artist"))
        .join(safe_name(album, "Unknown Album"))
}

/// A track's file name: "03 Title.flac", or "Title.flac" without a number.
pub(crate) fn track_name(number: &str, title: &str, ext: &str) -> String {
    let ext = ext.trim_start_matches('.').to_lowercase();
    let ext = if ext.is_empty() { "mp3".into() } else { ext };
    let base = match number.trim().parse::<u32>() {
        Ok(n) if n > 0 => format!("{n:02} {title}"),
        _ => title.to_string(),
    };
    format!("{}.{ext}", safe_name(&base, "track"))
}

/// The downloads a source has started.
pub(crate) struct Downloads {
    name: &'static str,
    agent: ureq::Agent,
    updates: Arc<Mutex<Vec<DownloadUpdate>>>,
    cancels: HashMap<String, Arc<AtomicBool>>,
    /// What each download was, so it can be tried again.
    jobs: HashMap<String, Vec<File>>,
}

impl Downloads {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            agent: agent(),
            updates: Arc::default(),
            cancels: HashMap::new(),
            jobs: HashMap::new(),
        }
    }

    /// Starts fetching `files` as the download `key`.
    pub fn start(&mut self, key: &str, files: Vec<File>) -> Result<(), PluginError> {
        if files.iter().all(|f| f.cover) {
            push(
                &self.updates,
                key,
                Progress::Failed("Nothing to download.".into()),
            );
            return Ok(());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancels.insert(key.to_string(), cancel.clone());
        self.jobs.insert(key.to_string(), files.clone());
        push(&self.updates, key, Progress::Queued);
        let job = Job {
            agent: self.agent.clone(),
            key: key.to_string(),
            files,
            updates: self.updates.clone(),
            cancel,
        };
        std::thread::Builder::new()
            .name(self.name.into())
            .spawn(move || job.run())
            .map(|_| ())
            .map_err(|e| PluginError::Network(e.to_string()))
    }

    pub fn progress(&mut self) -> Vec<DownloadUpdate> {
        let mut all = self
            .updates
            .lock()
            .map(|mut u| std::mem::take(&mut *u))
            .unwrap_or_default();
        // Only the latest news of each download matters.
        let mut seen = HashSet::new();
        all.reverse();
        all.retain(|u| seen.insert(u.key.clone()));
        all.reverse();
        for u in &all {
            if matches!(u.progress, Progress::Done(_) | Progress::Failed(_)) {
                self.cancels.remove(&u.key);
            }
        }
        all
    }

    pub fn retry(&mut self, key: &str) -> Result<(), PluginError> {
        let files = self
            .jobs
            .get(key)
            .cloned()
            .ok_or(PluginError::NotSupported)?;
        self.start(key, files)
    }

    pub fn cancel(&mut self, key: &str) -> Result<(), PluginError> {
        if let Some(c) = self.cancels.get(key) {
            c.store(true, Ordering::Relaxed);
        }
        Ok(())
    }
}

fn push(updates: &Mutex<Vec<DownloadUpdate>>, key: &str, progress: Progress) {
    if let Ok(mut u) = updates.lock() {
        u.push(DownloadUpdate {
            key: key.to_string(),
            progress,
        });
    }
}

struct Job {
    agent: ureq::Agent,
    key: String,
    files: Vec<File>,
    updates: Arc<Mutex<Vec<DownloadUpdate>>>,
    cancel: Arc<AtomicBool>,
}

impl Job {
    fn run(self) {
        let progress = match self.fetch() {
            Ok(paths) => Progress::Done(paths),
            Err(e) => Progress::Failed(e),
        };
        push(&self.updates, &self.key, progress);
    }

    fn fetch(&self) -> Result<Vec<PathBuf>, String> {
        let total: u64 = self
            .files
            .iter()
            .filter(|f| !f.cover)
            .map(|f| f.size)
            .sum::<u64>()
            .max(1);
        let mut done = 0u64;
        let mut paths = Vec::new();
        for f in &self.files {
            if self.cancel.load(Ordering::Relaxed) {
                return Err("Cancelled.".into());
            }
            if let Some(parent) = f.to.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            if f.cover {
                if !f.to.exists() {
                    let _ = self.save(&f.url, &f.to, &mut |_| {});
                }
                continue;
            }
            // Already here from an earlier download.
            if !f.to.exists() {
                let start = done;
                let updates = self.updates.clone();
                let key = self.key.clone();
                self.save(&f.url, &f.to, &mut |n| {
                    let p = (start + n) as f32 / total as f32;
                    push(&updates, &key, Progress::Running(p.min(1.0)));
                })?;
            }
            done += f.size;
            paths.push(f.to.clone());
        }
        Ok(paths)
    }

    /// Writes `url` to `to`, through a partial file so a stopped download
    /// leaves nothing that looks finished.
    fn save(&self, url: &str, to: &Path, progress: &mut dyn FnMut(u64)) -> Result<(), String> {
        let mut response = self.agent.get(url).call().map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        if status >= 400 {
            return Err(format!("The server answered {status}."));
        }
        let kind = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        // A Subsonic server reports a failure as a small JSON or XML body.
        if kind.contains("json") || kind.contains("xml") {
            return Err("The server wouldn't send the file.".into());
        }
        let mut body = response.body_mut().as_reader();
        let part = to.with_extension("part");
        let mut out = std::fs::File::create(&part).map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; 64 * 1024];
        let mut read = 0u64;
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                drop(out);
                let _ = std::fs::remove_file(&part);
                return Err("Cancelled.".into());
            }
            let n = match body.read(&mut buf) {
                Ok(n) => n,
                Err(e) => {
                    drop(out);
                    let _ = std::fs::remove_file(&part);
                    return Err(e.to_string());
                }
            };
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            read += n as u64;
            progress(read);
        }
        drop(out);
        std::fs::rename(&part, to).map_err(|e| e.to_string())
    }
}

/// Percent-encodes everything but unreserved characters, for a URL query
/// value or path segment.
pub(crate) fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// "FLAC" from a suffix or codec name.
pub(crate) fn format_name(suffix: &str) -> String {
    match suffix.to_lowercase().as_str() {
        "" => String::new(),
        "aif" | "aiff" => "AIFF".into(),
        "m4a" | "alac" => "ALAC".into(),
        "wav" | "wave" => "WAV".into(),
        "ogg" | "vorbis" => "OGG".into(),
        other => other.to_uppercase(),
    }
}

/// "320 kbps", or "" when unknown.
pub(crate) fn kbps(rate: u64) -> String {
    if rate == 0 {
        String::new()
    } else {
        format!("{rate} kbps")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn servers() {
        assert_eq!(server(" music.lan:4533/ "), "http://music.lan:4533");
        assert_eq!(server("https://x.example/nd/"), "https://x.example/nd");
        assert_eq!(server(""), "");
    }

    #[test]
    fn names() {
        assert_eq!(track_name("3", "Kerala", "FLAC"), "03 Kerala.flac");
        assert_eq!(track_name("", "Kerala", ""), "Kerala.mp3");
        assert_eq!(
            album_dir(Path::new("/d"), "Bonobo", "Migration"),
            PathBuf::from("/d/Bonobo/Migration")
        );
        assert_eq!(format_name("flac"), "FLAC");
        assert_eq!(kbps(320), "320 kbps");
    }
}
