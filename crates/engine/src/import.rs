//! What an add-on brings in from another DJ app's library: its tracks, and
//! the playlists, folders and history it keeps.

use std::path::PathBuf;

use crate::track::Track;

/// Another app's library, as read by an importer.
#[derive(Debug, Default)]
pub struct ImportedLibrary {
    /// Local files only, with what the app knows about them.
    pub tracks: Vec<Track>,
    /// Lists in the groups the DJ chose, in the app's order.
    pub lists: Vec<ImportedList>,
}

/// A playlist, history session or smart list from another app.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedList {
    /// The folders it sits in, outermost first ("GY Pro", "GY Sets").
    pub folders: Vec<String>,
    pub name: String,
    pub kind: ImportedListKind,
    /// Its tracks in order. Files the library doesn't have (streamed
    /// tracks, files that have gone) are left out when it is saved.
    pub entries: Vec<ImportedEntry>,
}

impl ImportedList {
    /// Names the list among its app's, for the DJ's choice of which come
    /// in: its kind, folders and name ("list:Sets/2023/Friday").
    pub fn key(&self) -> String {
        let kind = match self.kind {
            ImportedListKind::List => "list",
            ImportedListKind::History { .. } => "history",
            ImportedListKind::Smart { .. } => "smart",
        };
        let mut path = self.folders.clone();
        path.push(self.name.clone());
        format!("{kind}:{}", path.join("/"))
    }

    /// Whether it comes in when the DJ hasn't said: all but smart lists,
    /// whose searches may not mean the same here.
    pub fn wanted_by_default(&self) -> bool {
        !matches!(self.kind, ImportedListKind::Smart { .. })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ImportedListKind {
    List,
    /// A session of played tracks; `started` is a Unix time.
    History {
        started: i64,
    },
    /// A saved search in openDJ's search syntax.
    Smart {
        query: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportedEntry {
    pub path: PathBuf,
    /// When it was played, for a history session (Unix time).
    pub played_at: Option<i64>,
}

/// A kind of list an importer can bring in, which the DJ can turn on or
/// off in Settings > Library.
#[derive(Clone, Debug, PartialEq)]
pub struct ListGroup {
    /// Stable, saved in the settings ("playlists", "history").
    pub id: &'static str,
    pub title: &'static str,
    pub detail: &'static str,
    pub on_by_default: bool,
}
