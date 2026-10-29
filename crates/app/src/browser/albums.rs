//! The Albums and Mixed Sets sources in the rail: the albums folder (an
//! album a folder, or Artist/Album) and the mixed sets folder (recordings
//! of whole sets, openDJ's own among them), each as a tree of its folders
//! over the collection's tracks. Mixed sets stay out of the collection;
//! albums are in it too.
//!
//! Play Album puts an album on the Automix list and plays it through
//! gaplessly.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use opendj_engine::Track;

use super::{
    file_path, is_browsed, Browser, Contents, Node, ALBUMS, KIND_ALBUM, KIND_FOLDER, KIND_MIX,
};
use crate::LibraryEdit;

/// The tree of `root`'s folders that hold some of `files`, under a node
/// for all of them. Folders with tracks right in them are `leaf` kind
/// (an album, a folder of sets); those with only folders below are folders.
fn folder_tree<'a>(
    title: &str,
    root: &Path,
    files: impl Iterator<Item = &'a Path>,
    leaf: i32,
) -> Vec<Node> {
    let mut dirs: BTreeSet<PathBuf> = BTreeSet::new();
    let mut with_files: BTreeSet<PathBuf> = BTreeSet::new();
    for path in files {
        let Some(dir) = path.parent().filter(|d| d.starts_with(root) && *d != root) else {
            continue;
        };
        with_files.insert(dir.to_path_buf());
        let mut up = Some(dir);
        while let Some(d) = up.filter(|d| *d != root) {
            dirs.insert(d.to_path_buf());
            up = d.parent();
        }
    }
    let mut nodes = vec![Node::new(title, 0, 0, Contents::Under(root.to_path_buf()))];
    // In name order, ignoring case, each folder followed by what's in it.
    let mut sorted: Vec<PathBuf> = dirs.into_iter().collect();
    let key = |p: &PathBuf| {
        p.strip_prefix(root)
            .unwrap_or(p)
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
            .collect::<Vec<_>>()
    };
    sorted.sort_by_key(key);
    for dir in sorted {
        let depth = dir.strip_prefix(root).map_or(1, |r| r.components().count()) as i32;
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let kind = if with_files.contains(&dir) {
            leaf
        } else {
            KIND_FOLDER
        };
        let mut n = Node::new(name, depth, kind, Contents::Under(dir));
        // Artists start closed, so the albums list stays short.
        n.expanded = false;
        nodes.push(n);
    }
    nodes
}

impl Browser {
    pub(super) fn install_albums(
        edit: &LibraryEdit,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        edit.on_play_album(move |row, add| {
            w(&|b| {
                let dir = b
                    .rows
                    .get(row as usize)
                    .copied()
                    .flatten()
                    .and_then(|i| file_path(&b.library[i])?.parent().map(Path::to_path_buf));
                if let Some(dir) = dir {
                    b.play_album(&dir, add);
                }
            });
        });
        let w = with.clone();
        edit.on_play_album_folder(move |index| {
            w(&|b| {
                let dir =
                    b.visible_nodes
                        .get(index as usize)
                        .and_then(|&n| match &b.nodes[n].contents {
                            Contents::Under(d) => Some(d.clone()),
                            _ => None,
                        });
                if let Some(dir) = dir {
                    b.play_album(&dir, false);
                }
            });
        });
    }

    /// Whether `t` is a mixed set: a file in the mixed sets folder.
    pub(super) fn is_mixed_set(&self, t: &Track) -> bool {
        self.mixed_sets
            .as_deref()
            .is_some_and(|m| file_path(t).is_some_and(|p| p.starts_with(m)))
    }

    /// Whether `t` is in the collection: neither a file only shown in
    /// Files nor a mixed set.
    pub(super) fn in_collection(&self, t: &Track) -> bool {
        !is_browsed(t) && !self.is_mixed_set(t)
    }

    /// The folder the Albums or Mixed Sets source shows.
    fn folder_of(&self, source: i32) -> Option<&Path> {
        if source == ALBUMS {
            self.folders.albums.as_deref()
        } else {
            self.mixed_sets.as_deref()
        }
    }

    /// Builds the Albums or Mixed Sets tree again from the library,
    /// keeping the folder selected and those opened.
    pub(super) fn rebuild_folder_tree(&mut self, source: i32) {
        let selected = match self.nodes.get(self.current_node).map(|n| &n.contents) {
            Some(Contents::Under(p)) => Some(p.clone()),
            _ => None,
        };
        let open: Vec<PathBuf> = self
            .nodes
            .iter()
            .filter(|n| n.expanded && n.depth > 0)
            .filter_map(|n| match &n.contents {
                Contents::Under(p) => Some(p.clone()),
                _ => None,
            })
            .collect();
        let Some(root) = self.folder_of(source).map(Path::to_path_buf) else {
            self.nodes = Vec::new();
            self.current_node = 0;
            return;
        };
        let (title, leaf) = if source == ALBUMS {
            ("All Albums", KIND_ALBUM)
        } else {
            ("All Mixed Sets", KIND_MIX)
        };
        let files = self
            .library
            .iter()
            .filter(|t| !is_browsed(t) && !t.info.missing)
            .filter_map(|t| file_path(t));
        let mut nodes = folder_tree(title, &root, files, leaf);
        for n in &mut nodes {
            if let Contents::Under(p) = &n.contents {
                if open.contains(p) {
                    n.expanded = true;
                }
            }
        }
        let at = selected
            .and_then(|p| {
                nodes
                    .iter()
                    .position(|n| matches!(&n.contents, Contents::Under(d) if *d == p))
            })
            .unwrap_or(0);
        // The folders above the selection are opened.
        let mut depth = nodes[at].depth;
        for i in (0..at).rev() {
            if nodes[i].depth < depth {
                nodes[i].expanded = true;
                depth = nodes[i].depth;
            }
        }
        self.nodes = nodes;
        self.current_node = at;
    }

    /// The collection's tracks in `dir` and below, as an album plays: by
    /// folder, then track number, then file name.
    pub(super) fn tracks_under(&self, dir: &Path) -> Vec<usize> {
        let mut found: Vec<(usize, &Path)> = (0..self.library.len())
            .filter(|&i| !is_browsed(&self.library[i]) && !self.library[i].info.missing)
            .filter_map(|i| Some((i, file_path(&self.library[i])?)))
            .filter(|(_, p)| p.starts_with(dir))
            .collect();
        found.sort_by_cached_key(|&(i, p)| {
            let number = match self.library[i].info.track_number {
                0 => u32::MAX,
                n => n,
            };
            let lower = |p: Option<&Path>| p.map(|p| p.to_string_lossy().to_lowercase());
            (
                lower(p.parent()),
                number,
                lower(p.file_name().map(Path::new)),
            )
        });
        found.into_iter().map(|(i, _)| i).collect()
    }

    /// Puts the album in `dir` on the Automix list (after what is there
    /// with `add`, else instead of it) and, unless adding, has Automix play
    /// it gaplessly.
    fn play_album(&mut self, dir: &Path, add: bool) {
        // Only the album's own tracks, not those in folders below it.
        let album: Vec<usize> = self
            .tracks_under(dir)
            .into_iter()
            .filter(|&t| file_path(&self.library[t]).and_then(Path::parent) == Some(dir))
            .collect();
        if album.is_empty() {
            return;
        }
        self.adopt(&album);
        if add {
            self.side[0].extend(album);
        } else {
            self.side[0] = album;
            self.album_request = true;
        }
        self.refresh_side();
    }

    /// Whether Play Album was picked since this was last asked: Automix
    /// then plays the Automix list gaplessly.
    pub fn take_album_request(&mut self) -> bool {
        std::mem::take(&mut self.album_request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn albums_nest_under_their_artist() {
        let files = [
            "/m/Albums/Air/Moon Safari/01.flac",
            "/m/Albums/Air/Talkie Walkie/01.flac",
            "/m/Albums/bonobo - migration/01.mp3",
            "/m/Albums/loose.mp3",
            "/m/Other/x.mp3",
        ];
        let nodes = folder_tree(
            "All Albums",
            Path::new("/m/Albums"),
            files.iter().map(Path::new),
            KIND_ALBUM,
        );
        let shown: Vec<(&str, i32, i32)> = nodes
            .iter()
            .map(|n| (n.name.as_str(), n.depth, n.kind))
            .collect();
        assert_eq!(
            shown,
            [
                ("All Albums", 0, 0),
                ("Air", 1, KIND_FOLDER),
                ("Moon Safari", 2, KIND_ALBUM),
                ("Talkie Walkie", 2, KIND_ALBUM),
                ("bonobo - migration", 1, KIND_ALBUM),
            ]
        );
    }
}
