//! Brings a VirtualDJ library into the collection at start, when the DJ
//! turns it on in Settings > Library: the tracks in `database.xml` (see
//! [`database`]), and the playlists, folders and history kept next to it
//! (see [`lists`]). It only reads VirtualDJ's files, never writes them.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::{ImportedLibrary, LibraryImport, ListGroup};

pub mod database;
pub mod lists;

/// VirtualDJ's `database.xml` and its lists, read whenever they have
/// changed since last time.
pub struct VirtualDj;

impl LibraryImport for VirtualDj {
    fn id(&self) -> &str {
        "vdj"
    }

    fn title(&self) -> &str {
        "VirtualDJ"
    }

    fn database(&self) -> Option<PathBuf> {
        database::default_database()
    }

    fn list_groups(&self) -> Vec<ListGroup> {
        lists::groups()
    }

    /// The newest change to the database or to anything in the list folders.
    fn stamp(&self, database: &Path) -> String {
        let home = database.parent().unwrap_or(Path::new("."));
        let mut newest = modified(database);
        let mut stack: Vec<PathBuf> = ["Playlists", "Folders", "History"]
            .iter()
            .map(|d| home.join(d))
            .collect();
        while let Some(dir) = stack.pop() {
            newest = newest.max(modified(&dir));
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    newest = newest.max(modified(&p));
                }
            }
        }
        format!("{}:{newest}", database.display())
    }

    fn read(&self, database: &Path, groups: &[&str]) -> Result<ImportedLibrary, String> {
        let import = database::import(database, 0, Path::exists, database::read_file_tags)?;
        eprintln!(
            "VirtualDJ library: {} tracks; skipped {} streaming, {} cached, {} missing",
            import.tracks.len(),
            import.streaming,
            import.cached,
            import.missing
        );
        let home = database.parent().unwrap_or(Path::new("."));
        let mut skipped = lists::Skipped::default();
        let lists = lists::read(home, groups, &mut skipped);
        if !skipped.filters.is_empty() {
            eprintln!(
                "VirtualDJ filter folders openDJ's search can't say yet: {}",
                skipped.filters.join("; ")
            );
        }
        Ok(ImportedLibrary {
            tracks: import.tracks.iter().map(|t| (**t).clone()).collect(),
            lists,
        })
    }
}

fn modified(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}
