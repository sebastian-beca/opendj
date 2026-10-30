//! The Albums and Mixed Sets sources in the rail: the albums folder (an
//! album a folder, or Artist/Album) and the mixed sets folder (recordings
//! of whole sets, openDJ's own among them), each as a tree of its folders
//! over the collection's tracks. Mixed sets stay out of the collection;
//! albums are in it too.
//!
//! Play Album puts an album on the Automix list and plays it through
//! gaplessly.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use opendj_engine::Track;
use slint::{ComponentHandle, ModelRc, VecModel};

use super::{
    file_path, is_browsed, Browser, Contents, Node, ALBUMS, KIND_ALBUM, KIND_FOLDER, KIND_MIX,
};
use crate::{settings, AlbumTile, AlbumsUi, LibraryEdit};

/// Longest side of a cover in the covers view: twice its width, for HiDPI
/// screens.
pub(super) const COVER_SIZE: u32 = 360;

/// Whether the Albums source shows covers rather than a list.
const ALBUMS_COVERS: &str = "albums_covers";

/// What most of `values` are, ignoring empty ones and case; `None` when
/// none is shared by more than half of them.
fn most(values: &[&str]) -> Option<String> {
    let mut counts: HashMap<String, (usize, &str)> = HashMap::new();
    for v in values.iter().map(|v| v.trim()).filter(|v| !v.is_empty()) {
        counts.entry(v.to_lowercase()).or_insert((0, v)).0 += 1;
    }
    counts
        .into_values()
        .max_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(a.1)))
        .filter(|(n, _)| 2 * n > values.len())
        .map(|(_, v)| v.to_string())
}

/// An album's name and artist, from its tracks' album and artist tags:
/// the album most have (else the folder's name), and the artist most have
/// (else "Various Artists").
fn album_title(dir: &Path, albums: &[&str], artists: &[&str]) -> (String, String) {
    let name = most(albums).unwrap_or_else(|| {
        dir.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    let artist = most(artists).unwrap_or_else(|| "Various Artists".into());
    (name, artist)
}

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

    pub(super) fn install_album_covers(
        ui: &AlbumsUi,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        ui.set_covers(settings::flag(ALBUMS_COVERS, true));
        let w = with.clone();
        ui.on_set_covers(move |on| {
            w(&|b| {
                settings::set(ALBUMS_COVERS, serde_json::json!(on));
                b.album_back = None;
                b.refresh_album_ui();
            })
        });
        let w = with.clone();
        ui.on_open(move |i| w(&|b| b.open_album(i as usize)));
        let w = with.clone();
        ui.on_back(move || {
            w(&|b| {
                let back = b.album_back.take().or_else(|| {
                    // The folder above the album: its artist, or All Albums.
                    let depth = b.nodes.get(b.current_node)?.depth;
                    (0..b.current_node)
                        .rev()
                        .find(|&n| b.nodes[n].depth < depth)
                });
                if let Some(n) = back {
                    b.show_node(n);
                }
            })
        });
        let w = with.clone();
        ui.on_play(move |i, add| {
            w(&|b| {
                if let Some(dir) = b.album_tiles.get(i as usize).cloned() {
                    b.play_album(&dir, add);
                }
            })
        });
    }

    /// Whether the Albums source shows covers (when the folder selected
    /// isn't an album itself).
    fn showing_covers(&self) -> bool {
        self.tree == ALBUMS && settings::flag(ALBUMS_COVERS, true)
    }

    /// Whether the node selected is an album.
    fn on_album(&self) -> bool {
        self.nodes
            .get(self.current_node)
            .is_some_and(|n| n.kind == KIND_ALBUM)
    }

    /// Shows the covers view (or not) for what the list now holds: one
    /// tile per album folder among the listed tracks, in their order.
    pub(super) fn refresh_album_ui(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<AlbumsUi>();
        let covers = self.showing_covers();
        let grid = covers && !self.on_album();
        ui.set_offered(self.tree == ALBUMS);
        ui.set_covers(covers || (self.tree != ALBUMS && settings::flag(ALBUMS_COVERS, true)));
        ui.set_grid(grid);
        ui.set_opened(covers && self.on_album());
        if !grid {
            if !self.album_tiles.is_empty() {
                self.album_tiles.clear();
                ui.set_tiles(ModelRc::default());
            }
            return;
        }
        let mut order: Vec<PathBuf> = Vec::new();
        let mut tracks: HashMap<PathBuf, Vec<usize>> = HashMap::new();
        for &i in self.rows.iter().flatten() {
            let Some(dir) = file_path(&self.library[i]).and_then(Path::parent) else {
                continue;
            };
            let list = tracks.entry(dir.to_path_buf()).or_default();
            if list.is_empty() {
                order.push(dir.to_path_buf());
            }
            list.push(i);
        }
        let tiles: Vec<AlbumTile> = order
            .iter()
            .map(|dir| {
                let ids = &tracks[dir];
                let albums: Vec<&str> = ids
                    .iter()
                    .map(|&i| self.library[i].album.as_str())
                    .collect();
                let artists: Vec<&str> = ids
                    .iter()
                    .map(|&i| self.library[i].artist.as_str())
                    .collect();
                let (name, artist) = album_title(dir, &albums, &artists);
                let cover = self.album_art.get(&self.library[ids[0]]);
                AlbumTile {
                    name: name.into(),
                    artist: artist.into(),
                    has_cover: cover.is_some(),
                    cover: cover.unwrap_or_default(),
                }
            })
            .collect();
        self.album_tiles = order;
        ui.set_tiles(ModelRc::new(VecModel::from(tiles)));
    }

    /// Selects tree node `n` and lists it, as clicking it does.
    fn show_node(&mut self, n: usize) {
        self.current_node = n;
        // The folders above it open, so it shows in the tree.
        let mut depth = self.nodes[n].depth;
        for i in (0..n).rev() {
            if self.nodes[i].depth < depth {
                self.nodes[i].expanded = true;
                depth = self.nodes[i].depth;
            }
        }
        if let Some(app) = self.app.upgrade() {
            app.set_current_track(-1);
        }
        self.refresh_tree();
        self.refresh_rows();
    }

    /// Opens the album at `index` of the covers: its tracks in the list,
    /// with "‹ Albums" back to the covers.
    fn open_album(&mut self, index: usize) {
        let Some(dir) = self.album_tiles.get(index) else {
            return;
        };
        let Some(n) = self
            .nodes
            .iter()
            .position(|n| matches!(&n.contents, Contents::Under(d) if d == dir))
        else {
            return;
        };
        self.album_back = Some(self.current_node);
        self.show_node(n);
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
    fn an_album_is_named_by_most_of_its_tags() {
        let dir = Path::new("/m/Albums/Air/Moon Safari");
        assert_eq!(
            album_title(
                dir,
                &["Moon Safari", "moon safari", ""],
                &["Air", "Air", "Air"]
            ),
            ("Moon Safari".into(), "Air".into())
        );
        // No album tags: the folder's name; artists all differ: various.
        assert_eq!(
            album_title(dir, &["", ""], &["Air", "Beck"]),
            ("Moon Safari".into(), "Various Artists".into())
        );
    }

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
