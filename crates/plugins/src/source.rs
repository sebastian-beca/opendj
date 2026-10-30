//! A source of its own in the rail, for an add-on that searches somewhere
//! and downloads what the DJ picks: saved searches down the left, results
//! (folders and single files) in the list, and a download for each.
//!
//! The add-on does the searching and downloading; the app keeps the saved
//! searches, shows the results as they arrive, tracks each download and adds
//! finished files to the collection.

use std::path::{Path, PathBuf};

use crate::PluginError;

/// How the source appears in the rail.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceInfo {
    /// Shown in the rail's menu and above its searches, e.g. a sharing network.
    pub name: &'static str,
    /// The rail icon, as SVG.
    pub icon_svg: &'static [u8],
    /// What the search box suggests, e.g. "Artist - Title, an album, or a link".
    pub hint: &'static str,
}

/// What the DJ said they were looking for, when they didn't leave it to
/// the source to work out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum QueryKind {
    #[default]
    Auto,
    Track,
    Album,
    Artist,
    /// A link to a playlist elsewhere.
    Playlist,
}

impl QueryKind {
    pub const ALL: [QueryKind; 5] = [
        QueryKind::Auto,
        QueryKind::Track,
        QueryKind::Album,
        QueryKind::Artist,
        QueryKind::Playlist,
    ];

    /// As saved with a search.
    pub fn name(self) -> &'static str {
        match self {
            QueryKind::Auto => "auto",
            QueryKind::Track => "track",
            QueryKind::Album => "album",
            QueryKind::Artist => "artist",
            QueryKind::Playlist => "playlist",
        }
    }

    pub fn from_name(s: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|k| k.name() == s)
            .unwrap_or_default()
    }
}

/// Which copies to rank first. Nothing is hidden.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Prefer {
    #[default]
    Lossless,
    Mp3,
    Any,
}

impl Prefer {
    pub const ALL: [Prefer; 3] = [Prefer::Lossless, Prefer::Mp3, Prefer::Any];

    pub fn name(self) -> &'static str {
        match self {
            Prefer::Lossless => "lossless",
            Prefer::Mp3 => "mp3",
            Prefer::Any => "any",
        }
    }

    pub fn from_name(s: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|k| k.name() == s)
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SourceQuery {
    pub text: String,
    pub kind: QueryKind,
    pub prefer: Prefer,
    /// A playlist searched again: the entries (each a [`Found::group`])
    /// the DJ already has, so they aren't searched for again.
    pub skip: Vec<String>,
}

/// One result: a file, or a folder of them (an album).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    /// The source's own handle on it, passed back to download it. Unique
    /// within one search.
    pub key: String,
    pub folder: bool,
    pub artist: String,
    pub title: String,
    pub album: String,
    pub year: String,
    /// Its number on the album, when it's a file in a folder.
    pub number: String,
    /// Seconds, when known.
    pub duration: f64,
    /// "FLAC", "MP3".
    pub format: String,
    /// "16/44.1", "320 kbps".
    pub quality: String,
    /// Bytes (a folder: all its files).
    pub size: u64,
    /// Who shares it.
    pub from: String,
    /// A folder's files, as far as they are known.
    pub tracks: Vec<Found>,
    /// How many audio files a folder has, even before they're all listed.
    pub track_count: usize,
    /// For a playlist search: the playlist entry this was found for, e.g.
    /// "3. Bonobo – Kerala". Results with the same group show together.
    pub group: String,
    /// A picture of it (an album's cover), as an address the app fetches.
    pub cover_url: String,
}

/// What a search has found so far.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub items: Vec<Found>,
    /// Nothing more will come.
    pub done: bool,
    /// A line about the search as a whole, e.g. "18 of 22 found".
    pub note: String,
    /// A name to list the search by when its query isn't one, e.g. a
    /// playlist's name for its link. Empty keeps the query.
    pub title: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Progress {
    Queued,
    /// 0..1.
    Running(f32),
    /// Finished, with the files it wrote.
    Done(Vec<PathBuf>),
    Failed(String),
}

/// A change in one download, by the key of what was asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct DownloadUpdate {
    pub key: String,
    pub progress: Progress,
}

/// Searches and downloads for a source in the rail. Runs on a thread of
/// its own; every call may block on the network.
pub trait SearchSource: Send {
    fn info(&self) -> SourceInfo;

    /// How the source reads a query, a line for each search it will run,
    /// e.g. "Album: artist Bonobo, album Migration". Shown while typing, so
    /// it must not touch the network.
    fn describe(&self, query: &SourceQuery) -> Vec<String>;

    /// Starts a search and returns its id; results come from [`Self::results`].
    fn start(&mut self, query: &SourceQuery) -> Result<String, PluginError>;

    /// What search `id` has found so far. Asked about once a second until
    /// it's done.
    fn results(&mut self, id: &str) -> Result<Snapshot, PluginError>;

    /// The DJ removed search `id` or started it again.
    fn stop(&mut self, _id: &str) {}

    /// All the files of a folder from search `id`, for when the DJ opens it.
    fn open_folder(&mut self, _id: &str, folder: &Found) -> Result<Vec<Found>, PluginError> {
        Ok(folder.tracks.clone())
    }

    /// Downloads files and folders from search `id` into `dir`. Each is
    /// then reported by its key from [`Self::progress`].
    fn download(&mut self, id: &str, items: &[Found], dir: &Path) -> Result<(), PluginError>;

    /// What changed in the downloads since last asked.
    fn progress(&mut self) -> Vec<DownloadUpdate>;

    /// Tries a failed download again, from another copy if there is one.
    fn retry(&mut self, _key: &str) -> Result<(), PluginError> {
        Err(PluginError::NotSupported)
    }

    /// Stops a download that's queued or running. Files it already wrote
    /// stay.
    fn cancel(&mut self, _key: &str) -> Result<(), PluginError> {
        Err(PluginError::NotSupported)
    }
}

/// The key of a source plugin's setting for where its downloads go. Left
/// empty, they go to `Downloaded/<source name>` in the music folder. `~`
/// at the start is the home folder.
pub const DOWNLOAD_FOLDER: &str = "download_folder";

/// A size as the list shows it: "40 MB", "1.2 GB".
pub fn megabytes(bytes: u64) -> String {
    match bytes {
        0 => String::new(),
        b if b >= 1 << 30 => format!("{:.1} GB", b as f64 / (1u64 << 30) as f64),
        b => format!("{:.0} MB", (b as f64 / (1u64 << 20) as f64).max(1.0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip() {
        for k in QueryKind::ALL {
            assert_eq!(QueryKind::from_name(k.name()), k);
        }
        assert_eq!(QueryKind::from_name("nonsense"), QueryKind::Auto);
        for p in Prefer::ALL {
            assert_eq!(Prefer::from_name(p.name()), p);
        }
    }

    #[test]
    fn sizes() {
        assert_eq!(megabytes(0), "");
        assert_eq!(megabytes(40 << 20), "40 MB");
        assert_eq!(megabytes(1000), "1 MB");
        assert_eq!(megabytes(3 << 30), "3.0 GB");
    }
}
