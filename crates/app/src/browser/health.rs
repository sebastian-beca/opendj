//! Library Health in the browser (see [`mixxx_library::health`]): a node in
//! the collection's tree with one node per check under it, the checks'
//! page, and the fixes each check offers for its tracks.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mixxx_engine::{tags, Track};
use mixxx_library::health::{self, Check, Facts};
use mixxx_library::query::Indexed;
use mixxx_library::stages::Stage;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::info::tag_fields;
use super::{file_path, is_browsed, Browser, Contents, Node, KIND_CHECK, KIND_HEALTH};
use crate::cover_scan;
use crate::{settings, HealthCard, HealthUi};

/// The word the "Tag as mix" fix adds to a track's first tag field.
const MIX_TAG: &str = "#mix";

/// A fix a check offers.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Fix {
    /// Out of the library; the file stays.
    Remove,
    /// The check leaves it alone: for "Not music", it is music after all.
    MarkMusic,
    ToTriage,
    Analyze,
    FillFromName,
    Relink,
    TagMix,
}

impl Fix {
    fn label(self) -> &'static str {
        match self {
            Fix::Remove => "Remove from library",
            Fix::MarkMusic => "It's music",
            Fix::ToTriage => "Move to Triage",
            Fix::Analyze => "Analyze",
            Fix::FillFromName => "Fill from file name",
            Fix::Relink => "Find files",
            Fix::TagMix => "Tag as #mix",
        }
    }
}

/// The fixes a check offers, the one most often wanted first.
fn fixes(check: Check, stages: bool) -> Vec<Fix> {
    match check {
        Check::NotMusic => vec![Fix::Remove, Fix::MarkMusic],
        Check::Stray if stages => vec![Fix::ToTriage, Fix::Remove],
        Check::Stray => vec![Fix::Remove],
        Check::NotAnalyzed => vec![Fix::Analyze],
        Check::MissingInfo => vec![Fix::FillFromName],
        Check::Missing => vec![Fix::Relink, Fix::Remove],
        Check::Long => vec![Fix::TagMix],
        Check::NoCover | Check::NoRating | Check::Duplicate => Vec::new(),
    }
}

/// "1 track" or "3 tracks".
fn tracks(n: usize) -> String {
    format!("{n} track{}", if n == 1 { "" } else { "s" })
}

/// The tree's Library Health node, closed, with a node per check under it.
pub(super) fn health_nodes() -> Vec<Node> {
    let mut top = Node::new("Library Health", 0, KIND_HEALTH, Contents::Health(None));
    top.expanded = false;
    let mut nodes = vec![top];
    for check in Check::ALL {
        nodes.push(Node::new(
            check.title(),
            1,
            KIND_CHECK,
            Contents::Health(Some(check)),
        ));
    }
    nodes
}

/// Every audio file under `roots`, by its file name in lower case.
fn files_by_name(roots: &[PathBuf]) -> HashMap<String, Vec<PathBuf>> {
    let mut out: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut stack: Vec<PathBuf> = roots.to_vec();
    let mut seen = HashSet::new();
    while let Some(dir) = stack.pop() {
        if !seen.insert(dir.clone()) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            let hidden = e.file_name().to_string_lossy().starts_with('.');
            match e.file_type() {
                Ok(t) if t.is_dir() && !hidden => stack.push(path),
                Ok(t) if t.is_file() && mixxx_engine::scan::is_audio(&path) => {
                    let name = e.file_name().to_string_lossy().to_lowercase();
                    out.entry(name).or_default().push(path);
                }
                _ => {}
            }
        }
    }
    out
}

impl Browser {
    pub(super) fn install_health(
        ui: &HealthUi,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        ui.on_review(move |i| {
            w(&|b| {
                b.review(
                    usize::try_from(i)
                        .ok()
                        .and_then(|i| Check::ALL.get(i).copied()),
                )
            })
        });
        let w = with.clone();
        ui.on_fix(move |action, first, last| {
            w(&|b| {
                let Some(Contents::Health(Some(check))) =
                    b.nodes.get(b.current_node).map(|n| n.contents.clone())
                else {
                    return;
                };
                let Some(&fix) = fixes(check, b.folders.is_set()).get(action.max(0) as usize)
                else {
                    return;
                };
                let indices = b.picked_rows(first, last);
                b.fix(fix, &indices);
            })
        });
        let w = with.clone();
        ui.on_ignore(move |first, last| {
            w(&|b| {
                let Some(Contents::Health(Some(check))) =
                    b.nodes.get(b.current_node).map(|n| n.contents.clone())
                else {
                    return;
                };
                let indices = b.picked_rows(first, last);
                b.ignore(check, &indices);
            })
        });
        let w = with.clone();
        ui.on_restore(move || w(&|b| b.restore_removed()));
    }

    /// Library tracks (indices into `library`) on rows `first` to `last`, or
    /// on every row when `first` is negative.
    fn picked_rows(&self, first: i32, last: i32) -> Vec<usize> {
        let rows = &self.rows;
        let range = if first < 0 {
            0..rows.len()
        } else {
            let last = (last.max(first) as usize).min(rows.len().saturating_sub(1));
            first as usize..last + 1
        };
        range
            .filter_map(|r| rows.get(r).copied().flatten())
            .collect()
    }

    /// Works out the checks' facts from the library as it is now, and looks
    /// for covers in files not looked at since they last changed.
    pub(super) fn refresh_health(&mut self) {
        let Some(db) = &self.db else {
            self.health = Facts::default();
            return;
        };
        let ignored = db.health_ignored().unwrap_or_default();
        let saved = db.covers().unwrap_or_default();
        let tracks: Vec<&Track> = self
            .library
            .iter()
            .filter(|t| !is_browsed(t))
            .map(|t| &**t)
            .collect();
        let mut covers = HashMap::new();
        let mut look = Vec::new();
        for t in &tracks {
            let Some(path) = file_path(t) else { continue };
            if t.info.missing {
                continue;
            }
            match saved.get(&t.id) {
                Some(&(mtime, has)) if mtime == cover_scan::mtime(path) => {
                    covers.insert(t.id, has);
                }
                _ => look.push((t.id, path.to_path_buf(), t.album.clone())),
            }
        }
        self.health = Facts::new(
            tracks.iter().copied(),
            ignored,
            covers,
            self.folders.is_set(),
        );
        self.removed = db.removed_count().unwrap_or(0);
        for (id, path, album) in look {
            let music = self.index_of.get(&id).is_some_and(|&i| {
                !self
                    .health
                    .finds(Check::NotMusic, &self.library[i], &HashSet::new())
            });
            if music && !cover_scan::offloaded(&path) {
                self.cover_scan.add(id, &path, &album);
            }
        }
    }

    /// How many tracks `check` finds.
    pub(super) fn health_count(&self, check: Check) -> usize {
        self.library
            .iter()
            .filter(|t| !is_browsed(t) && self.health.finds(check, t, &self.duplicates))
            .count()
    }

    /// The tracks (indices into `library`) `check` finds.
    pub(super) fn health_tracks(&self, check: Check) -> Vec<usize> {
        (0..self.library.len())
            .filter(|&i| {
                let t = &self.library[i];
                !is_browsed(t) && self.health.finds(check, t, &self.duplicates)
            })
            .collect()
    }

    /// Takes in what the cover scan found since last time.
    pub fn poll_covers(&mut self) {
        let found = self.cover_scan.take();
        if found.is_empty() {
            return;
        }
        if let Some(db) = &mut self.db {
            if let Err(e) = db.save_covers(&found) {
                eprintln!("can't save covers: {e}");
            }
        }
        for (id, _, has) in found {
            self.health.set_cover(id, has);
        }
        self.refresh_tree();
        if matches!(
            self.nodes.get(self.current_node).map(|n| &n.contents),
            Some(Contents::Health(Some(Check::NoCover)))
        ) {
            self.refresh_rows();
        }
    }

    /// Selects a check's node (or the Library Health node), opening it.
    fn review(&mut self, check: Option<Check>) {
        let Some(n) = self.nodes.iter().position(|n| match (&n.contents, check) {
            (Contents::Health(c), _) => *c == check,
            _ => false,
        }) else {
            return;
        };
        if let Some(top) = self
            .nodes
            .iter()
            .position(|n| matches!(n.contents, Contents::Health(None)))
        {
            self.nodes[top].expanded = true;
        }
        self.health_status.clear();
        self.current_node = n;
        self.folders_focused = false;
        if let Some(app) = self.app.upgrade() {
            app.set_current_track(-1);
        }
        self.refresh_tree();
        self.refresh_rows();
    }

    /// Shows the page and bar as things are now.
    pub(super) fn refresh_health_ui(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<HealthUi>();
        let current = self.nodes.get(self.current_node).map(|n| &n.contents);
        let on = self.saved() && self.tree == super::COLLECTION && !self.history_tab;
        ui.set_overview(on && matches!(current, Some(Contents::Health(None))));
        let check = match current {
            Some(Contents::Health(Some(c))) if on => Check::ALL.iter().position(|x| x == c),
            _ => None,
        };
        ui.set_check(check.map_or(-1, |c| c as i32));
        let cards: Vec<HealthCard> = Check::ALL
            .iter()
            .map(|&c| {
                let offered = fixes(c, self.folders.is_set());
                // The track menu has the stage lines already.
                let menu: Vec<i32> = (0..offered.len() as i32)
                    .filter(|&j| offered[j as usize] != Fix::ToTriage)
                    .collect();
                let fixes: Vec<SharedString> =
                    offered.into_iter().map(|f| f.label().into()).collect();
                HealthCard {
                    title: c.title().into(),
                    about: c.about().into(),
                    count: self.health_count(c) as i32,
                    detail: self.health_detail(c).into(),
                    fixes: ModelRc::new(VecModel::from(fixes)),
                    menu_fixes: ModelRc::new(VecModel::from(menu)),
                }
            })
            .collect();
        ui.set_cards(ModelRc::new(VecModel::from(cards)));
        ui.set_status(self.health_status.clone().into());
        ui.set_removed(self.removed as i32);
    }

    /// A line on what a check found beyond its count.
    fn health_detail(&self, check: Check) -> String {
        match check {
            Check::MissingInfo => {
                let found = self.health_tracks(check);
                let no_name = found
                    .iter()
                    .filter(|&&i| {
                        let t = &self.library[i];
                        t.artist.trim().is_empty() || t.title.trim().is_empty()
                    })
                    .count();
                if no_name > 0 {
                    format!("{} without artist or title.", no_name)
                } else {
                    String::new()
                }
            }
            Check::NoCover => {
                let unknown = self
                    .library
                    .iter()
                    .filter(|t| {
                        !is_browsed(t)
                            && !t.info.missing
                            && file_path(t).is_some()
                            && !self.health.cover_known(t.id)
                    })
                    .count();
                if unknown > 0 {
                    format!("Still looking in {}.", tracks(unknown))
                } else {
                    String::new()
                }
            }
            Check::Stray if !self.folders.is_set() => {
                "Set your stage folders in Settings › Library first.".into()
            }
            _ => String::new(),
        }
    }

    /// Runs `fix` on library tracks (indices into `library`).
    fn fix(&mut self, fix: Fix, indices: &[usize]) {
        if indices.is_empty() || !self.saved() {
            return;
        }
        self.health_status = match fix {
            Fix::Remove => self.remove_tracks(indices),
            Fix::MarkMusic => {
                self.ignore(Check::NotMusic, indices);
                format!("{} kept as music.", tracks(indices.len()))
            }
            Fix::ToTriage => {
                self.change_stage(indices, Stage::Triage, None);
                format!("{} moved to Triage.", tracks(indices.len()))
            }
            Fix::Analyze => {
                let picked: Vec<Arc<Track>> =
                    indices.iter().map(|&i| self.library[i].clone()).collect();
                self.analysis.add(&picked);
                format!("Analyzing {} in the background.", tracks(picked.len()))
            }
            Fix::FillFromName => self.fill_from_names(indices),
            Fix::Relink => self.relink(indices),
            Fix::TagMix => self.tag_mix(indices),
        };
        self.refresh_tree();
        self.refresh_rows();
    }

    /// `check` leaves these tracks (indices into `library`) alone from now on.
    fn ignore(&mut self, check: Check, indices: &[usize]) {
        let ids: Vec<usize> = indices.iter().map(|&i| self.library[i].id).collect();
        if let Some(db) = &mut self.db {
            if let Err(e) = db.set_health_ignored(check, &ids, true) {
                eprintln!("can't save: {e}");
                return;
            }
        }
        self.health.set_ignored(check, &ids, true);
        self.health_status = format!("{} left out of {}.", tracks(ids.len()), check.title());
        self.refresh_tree();
        self.refresh_rows();
    }

    /// Takes tracks out of the library, keeping their files.
    fn remove_tracks(&mut self, indices: &[usize]) -> String {
        let ids: Vec<usize> = indices.iter().map(|&i| self.library[i].id).collect();
        let Some(db) = &mut self.db else {
            return String::new();
        };
        match db.remove_tracks(&ids) {
            Ok(n) => {
                self.reload_library();
                format!(
                    "{} removed from the library. The files are still there.",
                    tracks(n)
                )
            }
            Err(e) => format!("Couldn't remove them: {e}"),
        }
    }

    /// Brings back every track taken out of the library.
    fn restore_removed(&mut self) {
        let Some(db) = &mut self.db else { return };
        self.health_status = match db.restore_tracks(None) {
            Ok(n) => {
                self.reload_library();
                format!("{} back in the library.", tracks(n))
            }
            Err(e) => format!("Couldn't bring them back: {e}"),
        };
        self.refresh_tree();
        self.refresh_rows();
    }

    /// Reads the library again (after tracks left it or came back),
    /// staying on the node shown.
    fn reload_library(&mut self) {
        let Some(db) = &self.db else { return };
        let tracks = match db.all_tracks(Path::exists) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("can't read the library: {e}");
                return;
            }
        };
        let place = self
            .nodes
            .get(self.current_node)
            .map(|n| (n.name.clone(), n.depth, n.history));
        let status = std::mem::take(&mut self.health_status);
        // The side panel's lists (Automix) stay, by track id.
        let side: Vec<Vec<usize>> = self
            .side
            .iter()
            .map(|l| {
                l.iter()
                    .filter_map(|&i| self.library.get(i).map(|t| t.id))
                    .collect()
            })
            .collect();
        self.set_library(tracks.into_iter().map(Arc::new).collect());
        for (list, ids) in self.side.iter_mut().zip(side) {
            *list = ids
                .into_iter()
                .filter_map(|id| self.index_of.get(&id).copied())
                .collect();
        }
        self.refresh_side();
        self.health_status = status;
        if let Some((name, depth, history)) = place {
            if let Some(n) = self
                .nodes
                .iter()
                .position(|n| n.name == name && n.depth == depth && n.history == history)
            {
                if let Some(top) = self
                    .nodes
                    .iter()
                    .position(|n| matches!(n.contents, Contents::Health(None)))
                {
                    self.nodes[top].expanded |=
                        matches!(self.nodes[n].contents, Contents::Health(_));
                }
                self.current_node = n;
            }
        }
    }

    /// Fills in a missing artist or title from the file's name ("Artist -
    /// Title.mp3"), in the library and the file's tags.
    fn fill_from_names(&mut self, indices: &[usize]) -> String {
        let (mut filled, mut failed) = (0, 0);
        for &i in indices {
            let t = &self.library[i];
            let Some(path) = file_path(t) else { continue };
            let Some((artist, title)) = health::from_file_name(path) else {
                continue;
            };
            // A file without tags is listed by its name, as its title.
            let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned());
            let named =
                t.title.trim().is_empty() || Some(t.title.trim()) == stem.as_deref().map(str::trim);
            let mut t = (**t).clone();
            let before = (t.artist.clone(), t.title.clone());
            if t.artist.trim().is_empty() {
                t.artist = artist;
                if named {
                    t.title = title;
                }
            } else if t.title.trim().is_empty() {
                t.title = title;
            }
            if (t.artist.clone(), t.title.clone()) == before {
                continue;
            }
            if !t.info.missing && path.exists() {
                if let Err(e) = tags::write(path, &tag_fields(&t)) {
                    eprintln!("can't write tags to {}: {e}", path.display());
                    failed += 1;
                }
            }
            if let Some(db) = &self.db {
                if let Err(e) = db.save_info(t.id, &t) {
                    eprintln!("can't save: {e}");
                    failed += 1;
                    continue;
                }
            }
            self.indexed[i] = Indexed::new(&t);
            self.library[i] = Arc::new(t);
            filled += 1;
        }
        let skipped = indices.len() - filled;
        let mut s = format!("Filled in {}.", tracks(filled));
        if skipped > 0 {
            s += &format!(
                " {} had no \"Artist - Title\" file name, or nothing missing there.",
                tracks(skipped)
            );
        }
        if failed > 0 {
            s += &format!(" {failed} couldn't be written to the file.");
        }
        s
    }

    /// Looks for missing files by name in the stage folders and the music
    /// folder, and points each track with exactly one match at it.
    fn relink(&mut self, indices: &[usize]) -> String {
        let mut roots: Vec<PathBuf> = self
            .folders
            .all()
            .into_iter()
            .map(Path::to_path_buf)
            .collect();
        if let Some(music) = settings::music_folder() {
            roots.push(music);
        }
        let found = files_by_name(&roots);
        let (mut linked, mut many, mut none, mut taken) = (0, 0, 0, 0);
        let Some(db) = &mut self.db else {
            return String::new();
        };
        for &i in indices {
            let t = &self.library[i];
            let Some(name) = file_path(t).and_then(Path::file_name) else {
                continue;
            };
            let candidates: Vec<&PathBuf> = found
                .get(&name.to_string_lossy().to_lowercase())
                .map(|v| v.iter().collect())
                .unwrap_or_default();
            match candidates.as_slice() {
                [one] => match db.relink(t.id, one) {
                    Ok(()) => linked += 1,
                    // Another track has that file, with lists or cues of its own.
                    Err(_) => taken += 1,
                },
                [] => none += 1,
                _ => many += 1,
            }
        }
        if linked > 0 {
            if let Err(e) = db.assign_stages(&self.folders) {
                eprintln!("can't give the found files their stage: {e}");
            }
            self.reload_library();
        }
        let mut s = format!("Found {}.", tracks(linked));
        if none > 0 {
            s += &format!(" {none} not found.");
        }
        if many > 0 {
            s += &format!(" {many} matched more than one file, so they were left alone.");
        }
        if taken > 0 {
            s += &format!(" {taken} matched a track the library has already: see Duplicates.");
        }
        s
    }

    /// Adds "#mix" to the first tag field of long tracks.
    fn tag_mix(&mut self, indices: &[usize]) -> String {
        let mut n = 0;
        for &i in indices {
            let mut t = (*self.library[i]).clone();
            let first = t.info.user_tag(0).to_string();
            if first
                .split_whitespace()
                .any(|w| w.eq_ignore_ascii_case(MIX_TAG))
            {
                continue;
            }
            let words = if first.trim().is_empty() {
                MIX_TAG.to_string()
            } else {
                format!("{} {MIX_TAG}", first.trim())
            };
            t.info.set_user_tag(0, words);
            t.info.labels = t.info.user_words();
            if let Some(db) = &self.db {
                if let Err(e) = db.save_info(t.id, &t) {
                    eprintln!("can't save: {e}");
                    continue;
                }
            }
            self.health.set_ignored(Check::Long, &[t.id], true);
            if let Some(db) = &mut self.db {
                let _ = db.set_health_ignored(Check::Long, &[t.id], true);
            }
            self.indexed[i] = Indexed::new(&t);
            self.library[i] = Arc::new(t);
            n += 1;
        }
        format!("Tagged {} as {MIX_TAG}.", tracks(n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_check_has_a_node_and_fixes_fit() {
        let nodes = health_nodes();
        assert_eq!(nodes.len(), Check::ALL.len() + 1);
        assert!(!nodes[0].expanded);
        assert_eq!(fixes(Check::Stray, false), [Fix::Remove]);
        assert_eq!(fixes(Check::Stray, true)[0], Fix::ToTriage);
    }

    #[test]
    fn finds_files_by_name_below_folders() {
        let dir = std::env::temp_dir().join(format!("opendj-relink-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("a/b")).unwrap();
        std::fs::write(dir.join("a/b/Song.MP3"), b"x").unwrap();
        std::fs::write(dir.join("a/notes.txt"), b"x").unwrap();
        let found = files_by_name(std::slice::from_ref(&dir));
        assert_eq!(found["song.mp3"], [dir.join("a/b/Song.MP3")]);
        assert!(!found.contains_key("notes.txt"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
