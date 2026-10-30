//! Collection Health in the browser (see [`opendj_library::health`]): a node in
//! the collection's tree with one node per check under it, the checks'
//! page, and the fixes each check offers for its tracks.

use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use opendj_engine::{tags, Track};
use opendj_library::dupes::{self, Case, FileFacts, Group};
use opendj_library::health::{self, Check, Facts};
use opendj_library::query::Indexed;
use opendj_library::stages::Stage;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::info::tag_fields;
use super::{file_path, is_browsed, Browser, Contents, Node, Undo, KIND_CHECK, KIND_HEALTH};
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
    /// Duplicates: merges every group of copies that is safe to merge.
    AllEasy,
    /// Merges each listed group into the copy picked to keep.
    KeepBest,
    /// Copies of different lengths stay, linked to each other.
    LinkVersions,
    /// Opens the review table with what the clean-up rules would change.
    CleanUp,
    /// Looks online for covers and embeds the largest.
    FindCovers,
    /// Looks tracks up online and opens the review table with what was
    /// found.
    LookUp,
    /// Opens the review table with each album's tracks given the album
    /// tag most of them have.
    SameAlbum,
}

impl Fix {
    fn label(self) -> &'static str {
        match self {
            Fix::Remove => "Remove from collection",
            Fix::MarkMusic => "It's music",
            Fix::ToTriage => "Move to In Prep",
            Fix::Analyze => "Analyze",
            Fix::FillFromName => "Fill from file name",
            Fix::Relink => "Find files",
            Fix::TagMix => "Tag as #mix",
            Fix::AllEasy => "Fix all easy",
            Fix::KeepBest => "Keep the best copies",
            Fix::LinkVersions => "Link as versions",
            Fix::CleanUp => "Clean up…",
            Fix::FindCovers => "Find covers",
            Fix::LookUp => "Look up online",
            Fix::SameAlbum => "Use one album tag…",
        }
    }
}

/// The fixes a check offers, the one most often wanted first.
fn fixes(check: Check, stages: bool) -> Vec<Fix> {
    match check {
        Check::NotMusic => vec![Fix::Remove, Fix::MarkMusic],
        Check::Stray if stages => vec![Fix::ToTriage, Fix::Remove],
        Check::Stray => vec![Fix::Remove],
        Check::Outside if stages => vec![Fix::ToTriage, Fix::Remove],
        Check::Outside => vec![Fix::Remove],
        Check::NotAnalyzed => vec![Fix::Analyze],
        Check::MissingInfo => vec![Fix::FillFromName, Fix::LookUp],
        Check::Missing => vec![Fix::Relink, Fix::Remove],
        Check::Long => vec![Fix::TagMix],
        Check::Duplicate => vec![Fix::AllEasy],
        Check::Junk => vec![Fix::CleanUp, Fix::LookUp],
        Check::NoCover => vec![Fix::FindCovers],
        Check::AlbumTags => vec![Fix::SameAlbum],
        Check::NoRating | Check::LowBitrate => Vec::new(),
    }
}

/// "1 track" or "3 tracks".
pub(super) fn tracks(n: usize) -> String {
    format!("{n} track{}", if n == 1 { "" } else { "s" })
}

/// "1 copy" or "3 copies".
fn copies(n: usize) -> String {
    format!("{n} cop{}", if n == 1 { "y" } else { "ies" })
}

/// Collection Health's tree: the overview page, then a node per check, the
/// cases of copies under Duplicates (closed: they read the copies' files).
pub(super) fn health_nodes() -> Vec<Node> {
    let mut nodes = vec![Node::new(
        "Overview",
        0,
        KIND_HEALTH,
        Contents::Health(None),
    )];
    for check in Check::ALL {
        let mut node = Node::new(check.title(), 0, KIND_CHECK, Contents::Health(Some(check)));
        node.expanded = false;
        nodes.push(node);
        if check == Check::Duplicate {
            for case in Case::ALL {
                nodes.push(Node::new(
                    case.title(),
                    1,
                    KIND_CHECK,
                    Contents::Dupes(case),
                ));
            }
        }
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
                Ok(t) if t.is_file() && opendj_engine::scan::is_audio(&path) => {
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
        ui.on_show(move |on| w(&|b| b.show_health(on)));
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
                let Some(check) = b.listed_check() else {
                    return;
                };
                let Some(&fix) = b.offered(check).get(action.max(0) as usize) else {
                    return;
                };
                let indices = b.picked_rows(first, last);
                b.fix(fix, &indices);
            })
        });
        let w = with.clone();
        ui.on_ignore(move |first, last| {
            w(&|b| {
                let Some(check) = b.listed_check() else {
                    return;
                };
                let indices = b.picked_rows(first, last);
                b.ignore(check, &indices);
            })
        });
        let w = with.clone();
        ui.on_restore(move || w(&|b| b.restore_removed()));
        let w = with.clone();
        ui.on_tidy_lists(move || w(&|b| b.tidy_lists()));
        let w = with.clone();
        ui.on_keep_this(move |row| w(&|b| b.keep_this(row)));
        let w = with.clone();
        ui.on_quarantine(move |first, last| {
            w(&|b| {
                let indices = b.picked_rows(first, last);
                b.health_status = b.quarantine(&indices);
                b.refresh_crates();
                b.refresh_tree();
                b.refresh_rows();
            })
        });
        let w = with.clone();
        ui.on_show_quarantine(move || {
            w(&|b| {
                if let Some(dir) = b.quarantine_root().filter(|d| d.is_dir()) {
                    crate::files::reveal(&dir);
                }
            })
        });
    }

    /// The check whose tracks are listed: a case of copies is Duplicates'.
    fn listed_check(&self) -> Option<Check> {
        match self.nodes.get(self.current_node)?.contents {
            Contents::Health(check) => check,
            Contents::Dupes(_) => Some(Check::Duplicate),
            _ => None,
        }
    }

    /// The fixes offered for the tracks listed: each case of copies has
    /// its own.
    fn offered(&self, check: Check) -> Vec<Fix> {
        match self.dupe_view() {
            Some(Some(Case::Versions)) => vec![Fix::LinkVersions, Fix::KeepBest],
            Some(Some(_)) => vec![Fix::KeepBest],
            _ => fixes(check, self.folders.is_set()),
        }
    }

    /// Copies are listed: all of them (`Some(None)`), or one case's.
    pub(super) fn dupe_view(&self) -> Option<Option<Case>> {
        match self.nodes.get(self.current_node)?.contents {
            Contents::Health(Some(Check::Duplicate)) => Some(None),
            Contents::Dupes(case) => Some(Some(case)),
            _ => None,
        }
    }

    /// Works out which tracks are copies of others, without reading their
    /// files, and forgets the groups worked out before.
    pub(super) fn refresh_dupes(&mut self) {
        self.dupe_groups = OnceCell::new();
        self.links = self
            .db
            .as_ref()
            .and_then(|db| db.all_links().ok())
            .unwrap_or_default();
        let everyone: HashSet<usize> = self.library.iter().map(|t| t.id).collect();
        let tracks: Vec<&Track> = self
            .library
            .iter()
            .filter(|t| self.in_collection(t) && self.health.finds(Check::Duplicate, t, &everyone))
            .map(|t| &**t)
            .collect();
        let links = &self.links;
        self.duplicates = dupes::copies(tracks)
            .iter()
            .flat_map(|c| {
                dupes::groups(
                    c,
                    &self.folders,
                    |_| FileFacts::default(),
                    |_, _| false,
                    |a, b| links.contains(&(a, b)),
                )
            })
            .flat_map(|g| g.tracks)
            .collect();
        self.quarantined = self.count_quarantined();
    }

    /// The copies grouped by case, each with the copy to keep first. The
    /// first call after the library changes reads the copies' files.
    fn dupe_groups(&self) -> &[Group] {
        self.dupe_groups.get_or_init(|| {
            let tracks = self
                .library
                .iter()
                .filter(|t| self.in_collection(t) && self.duplicates.contains(&t.id))
                .map(|t| &**t);
            // Files iCloud has taken off this computer aren't read.
            fn here(t: &Track) -> Option<&Path> {
                file_path(t).filter(|p| !cover_scan::offloaded(p))
            }
            let facts = |t: &Track| {
                here(t).map_or_else(FileFacts::default, |p| {
                    let info = tags::file_info(p);
                    FileFacts {
                        size: info.size,
                        lossless: dupes::lossless(&info.format, info.bit_depth),
                        bitrate: info.bitrate.unwrap_or(0),
                    }
                })
            };
            let same = |a: &Track, b: &Track| match (here(a), here(b)) {
                (Some(x), Some(y)) => dupes::same_bytes(x, y),
                _ => false,
            };
            dupes::copies(tracks)
                .iter()
                .flat_map(|c| {
                    dupes::groups(c, &self.folders, facts, same, |a, b| {
                        self.links.contains(&(a, b))
                    })
                })
                .collect()
        })
    }

    /// The copies (indices into `library`) in groups of `case`, or in any
    /// group, group by group.
    pub(super) fn dupe_tracks(&self, case: Option<Case>) -> Vec<usize> {
        let mut seen = HashSet::new();
        self.dupe_groups()
            .iter()
            .filter(|g| case.is_none_or(|c| g.case == c))
            .flat_map(|g| &g.tracks)
            .filter(|&&id| seen.insert(id))
            .filter_map(|id| self.index_of.get(id).copied())
            .collect()
    }

    /// Ids of the copies the listed groups would merge away: in every easy
    /// group, or in `case`'s.
    pub(super) fn dupe_faded(&self, case: Option<Case>) -> HashSet<usize> {
        self.dupe_groups()
            .iter()
            .filter(|g| case.map_or(g.case.easy(), |c| g.case == c))
            .flat_map(|g| g.tracks[1..].iter().copied())
            .collect()
    }

    /// The groups of the cases `wanted` takes that hold any of these
    /// tracks (indices into `library`).
    fn picked_groups(&self, indices: &[usize], wanted: impl Fn(Case) -> bool) -> Vec<Group> {
        let ids: HashSet<usize> = indices.iter().map(|&i| self.library[i].id).collect();
        self.dupe_groups()
            .iter()
            .filter(|g| wanted(g.case) && g.tracks.iter().any(|t| ids.contains(t)))
            .cloned()
            .collect()
    }

    /// How many listed groups a fix for copies works on.
    fn fix_groups(&self, fix: Fix) -> usize {
        let case = self.dupe_view().flatten();
        self.dupe_groups()
            .iter()
            .filter(|g| match fix {
                Fix::AllEasy => g.case.easy(),
                Fix::LinkVersions => g.case == Case::Versions,
                _ => case.map_or(g.case.easy(), |c| g.case == c),
            })
            .count()
    }

    /// What merging `group` into `keep` takes out: the other copies, and
    /// for versions, each other version's own copies too.
    fn merge_plan(&self, group: &Group, keep: usize) -> (usize, Vec<usize>) {
        let mut others: Vec<usize> = group
            .tracks
            .iter()
            .copied()
            .filter(|&t| t != keep)
            .collect();
        if group.case == Case::Versions {
            let groups = self.dupe_groups();
            let copies: Vec<usize> = others
                .iter()
                .flat_map(|&o| {
                    groups
                        .iter()
                        .filter(move |g| g.case.easy() && g.tracks[0] == o)
                        .flat_map(|g| g.tracks[1..].iter().copied())
                })
                .collect();
            others.extend(copies);
        }
        (keep, others)
    }

    /// Keeps the copy at a row, merging the rest of its group into it.
    fn keep_this(&mut self, row: i32) {
        let Some(i) = usize::try_from(row)
            .ok()
            .and_then(|r| self.rows.get(r).copied().flatten())
        else {
            return;
        };
        let id = self.library[i].id;
        let view = self.dupe_view().flatten();
        let holds = |g: &&Group| g.tracks.contains(&id) && view.is_none_or(|c| g.case == c);
        let groups = self.dupe_groups();
        // On the list of all copies, its own recording's group comes first.
        let Some(group) = groups
            .iter()
            .filter(holds)
            .find(|g| g.case.easy())
            .or_else(|| groups.iter().find(holds))
            .cloned()
        else {
            return;
        };
        let plan = self.merge_plan(&group, id);
        self.health_status = self.merge(vec![plan]);
        self.refresh_tree();
        self.refresh_rows();
    }

    /// Merges groups of copies, each into the copy kept (see
    /// [`opendj_library::Library::merge_copies`]), as one change the undo
    /// line takes back.
    fn merge(&mut self, plan: Vec<(usize, Vec<usize>)>) -> String {
        if plan.is_empty() {
            return "Nothing to merge here.".into();
        }
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        let quarantine = dupes::quarantine_dir(&self.folders, &date);
        let moving = settings::move_files();
        let Some(db) = &mut self.db else {
            return String::new();
        };
        let (mut batch, mut groups, mut restore) = (None, 0, Vec::new());
        for (keep, others) in &plan {
            match db.merge_copies(*keep, others, quarantine.as_deref(), batch) {
                Ok(b) => {
                    batch = batch.or(b);
                    groups += 1;
                    restore.extend(others.iter().copied());
                }
                Err(e) => eprintln!("can't merge copies: {e}"),
            }
        }
        let mut failed = 0;
        if batch.is_some() && moving {
            match db.apply_moves() {
                Ok(r) => {
                    for (path, why) in &r.failed {
                        eprintln!("can't move {}: {why}", path.display());
                    }
                    failed = r.failed.len();
                }
                Err(e) => eprintln!("can't move files: {e}"),
            }
        }
        let n = restore.len();
        if n > 0 {
            self.undo = Some(Undo {
                batch,
                text: format!("Undo merging {}", copies(n)),
                restore,
                changes: None,
            });
        }
        self.read_pending_moves();
        self.reload_library();
        let mut s = format!(
            "Kept one copy of {} track{}; {} merged into {}.",
            groups,
            if groups == 1 { "" } else { "s" },
            copies(n),
            if groups == 1 { "it" } else { "them" },
        );
        s += match (quarantine.is_some(), moving) {
            (false, _) => " Their files stay where they are: set your Play Ready folder in Settings › Collection to quarantine them.",
            (true, true) => " Their files are in _Quarantine.",
            (true, false) => " Their files go to _Quarantine once Move files is on in Settings › Collection.",
        };
        if failed > 0 {
            s += &format!(" {failed} couldn't be moved.");
        }
        s
    }

    /// Takes tracks (indices into `library`) out of the library and moves
    /// their files to today's quarantine folder, or queues the moves while
    /// Move files is off, as one change the undo line takes back.
    fn quarantine(&mut self, indices: &[usize]) -> String {
        let ids: Vec<usize> = indices
            .iter()
            .map(|&i| &self.library[i])
            .filter(|t| !is_browsed(t))
            .map(|t| t.id)
            .collect();
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        let Some(quarantine) = dupes::quarantine_dir(&self.folders, &date) else {
            return "Set your Play Ready folder in Settings › Collection to quarantine tracks."
                .into();
        };
        let moving = settings::move_files();
        let Some(db) = &mut self.db else {
            return String::new();
        };
        let batch = match db.quarantine_tracks(&ids, Some(&quarantine)) {
            Ok(Some(b)) => b,
            Ok(None) => return "Nothing to quarantine here.".into(),
            Err(e) => return format!("Couldn't quarantine them: {e}"),
        };
        let mut failed = 0;
        if moving {
            match db.apply_moves() {
                Ok(r) => {
                    for (path, why) in &r.failed {
                        eprintln!("can't move {}: {why}", path.display());
                    }
                    failed = r.failed.len();
                }
                Err(e) => eprintln!("can't move files: {e}"),
            }
        }
        let n = ids.len();
        self.undo = Some(Undo {
            batch: Some(batch),
            text: format!("Undo moving {} to Quarantine", tracks(n)),
            restore: ids,
            changes: None,
        });
        self.read_pending_moves();
        self.reload_library();
        let mut s = format!("{} taken out of your collection.", tracks(n));
        s += if moving {
            " Their files are in _Quarantine."
        } else {
            " Their files go to _Quarantine once Move files is on in Settings › Collection."
        };
        if failed > 0 {
            s += &format!(" {failed} couldn't be moved.");
        }
        s
    }

    /// Links each listed group of versions, so they stay.
    fn link_versions(&mut self, indices: &[usize]) -> String {
        let groups = self.picked_groups(indices, |c| c == Case::Versions);
        let Some(db) = &self.db else {
            return String::new();
        };
        let mut n = 0;
        for g in &groups {
            for (i, &a) in g.tracks.iter().enumerate() {
                for &b in &g.tracks[i + 1..] {
                    if let Err(e) = db.link(a, b) {
                        eprintln!("can't link: {e}");
                    }
                }
            }
            n += g.tracks.len();
        }
        self.refresh_dupes();
        format!(
            "Linked {} as versions. Each shows the others on its Linked tab.",
            tracks(n)
        )
    }

    /// The folder merged copies' files go to, by date.
    fn quarantine_root(&self) -> Option<PathBuf> {
        dupes::quarantine_root(&self.folders)
    }

    /// Files in the quarantine folder, and those waiting to go there.
    fn count_quarantined(&self) -> usize {
        let Some(root) = self.quarantine_root() else {
            return 0;
        };
        let there: usize = files_by_name(std::slice::from_ref(&root))
            .values()
            .map(Vec::len)
            .sum();
        there
            + self
                .pending_moves
                .values()
                .filter(|to| to.starts_with(&root))
                .count()
    }

    /// Library tracks (indices into `library`) on rows `first` to `last`, or
    /// on every row when `first` is negative.
    pub(super) fn picked_rows(&self, first: i32, last: i32) -> Vec<usize> {
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
            .filter(|t| self.in_collection(t))
            .map(|t| &**t)
            .collect();
        let mut covers = HashMap::new();
        let mut bitrates = Vec::new();
        let mut look = Vec::new();
        for t in &tracks {
            let Some(path) = file_path(t) else { continue };
            if t.info.missing {
                continue;
            }
            match saved.get(&t.id) {
                // Looked at before bitrates were: look again.
                Some(found)
                    if found.mtime == cover_scan::mtime(path) && found.bitrate.is_some() =>
                {
                    covers.insert(t.id, found.cover);
                    bitrates.extend(found.bitrate.map(|b| (t.id, b)));
                }
                _ => look.push((t.id, path.to_path_buf(), t.album.clone())),
            }
        }
        let mut folders = settings::music_folders();
        folders.extend(self.folders.all().into_iter().map(Path::to_path_buf));
        folders.extend(self.mixed_sets.clone());
        let mut facts = Facts::new(
            tracks.iter().copied(),
            ignored,
            covers,
            self.folders.is_set(),
        )
        .with_folders(folders)
        .with_rules(settings::cleanup_rules());
        if let Some(albums) = &self.folders.albums {
            facts = facts.with_albums(albums, tracks.iter().copied());
        }
        for (id, kbps) in bitrates {
            facts.set_bitrate(id, kbps);
        }
        self.health = facts;
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
            .filter(|t| self.in_collection(t) && self.health.finds(check, t, &self.duplicates))
            .count()
    }

    /// The tracks (indices into `library`) `check` finds.
    pub(super) fn health_tracks(&self, check: Check) -> Vec<usize> {
        (0..self.library.len())
            .filter(|&i| {
                let t = &self.library[i];
                self.in_collection(t) && self.health.finds(check, t, &self.duplicates)
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
        for (id, look) in found {
            self.health.set_cover(id, look.cover);
            if let Some(kbps) = look.bitrate {
                self.health.set_bitrate(id, kbps);
            }
        }
        self.refresh_tree();
        if matches!(
            self.nodes.get(self.current_node).map(|n| &n.contents),
            Some(Contents::Health(Some(Check::NoCover | Check::LowBitrate)))
        ) {
            self.refresh_rows();
        }
    }

    /// Selects a check's node (or the Collection Health node), opening it.
    fn review(&mut self, check: Option<Check>) {
        self.show_health(true);
        let Some(n) = self.nodes.iter().position(|n| match (&n.contents, check) {
            (Contents::Health(c), _) => *c == check,
            _ => false,
        }) else {
            return;
        };
        // A case of copies sits under Duplicates.
        if let Some(dupes) = self
            .nodes
            .iter()
            .position(|n| matches!(n.contents, Contents::Health(Some(Check::Duplicate))))
            .filter(|&d| d < n && self.nodes[n].depth > 0)
        {
            self.nodes[dupes].expanded = true;
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
        let listed = self.listed_check().filter(|_| on);
        let check = listed.and_then(|c| Check::ALL.iter().position(|&x| x == c));
        ui.set_check(check.map_or(-1, |c| c as i32));
        let case = self.dupe_view().flatten();
        let cards: Vec<HealthCard> = Check::ALL
            .iter()
            .map(|&c| {
                let offered = if Some(c) == listed {
                    self.offered(c)
                } else {
                    fixes(c, self.folders.is_set())
                };
                // The track menu has the stage lines already, and "Keep
                // this copy" instead of fixing every group.
                let menu: Vec<i32> = (0..offered.len() as i32)
                    .filter(|&j| !matches!(offered[j as usize], Fix::ToTriage | Fix::AllEasy))
                    .collect();
                // Fixes for copies count the groups they merge or link.
                let counts: Vec<i32> = if Some(c) == listed && c == Check::Duplicate {
                    offered.iter().map(|&f| self.fix_groups(f) as i32).collect()
                } else {
                    Vec::new()
                };
                let fixes: Vec<SharedString> =
                    offered.into_iter().map(|f| f.label().into()).collect();
                let case = case.filter(|_| c == Check::Duplicate);
                HealthCard {
                    title: c.title().into(),
                    about: case.map_or(c.about(), Case::about).into(),
                    count: case.map_or_else(
                        || self.health_count(c),
                        |case| self.dupe_tracks(Some(case)).len(),
                    ) as i32,
                    detail: self.health_detail(c).into(),
                    fixes: ModelRc::new(VecModel::from(fixes)),
                    counts: ModelRc::new(VecModel::from(counts)),
                    menu_fixes: ModelRc::new(VecModel::from(menu)),
                }
            })
            .collect();
        // The rail's badge: the checks that find something to fix.
        if self.saved() {
            ui.set_issues(cards.iter().filter(|c| c.count > 0).count() as i32);
        }
        ui.set_view(self.health_view && self.tree == super::COLLECTION);
        ui.set_cards(ModelRc::new(VecModel::from(cards)));
        ui.set_status(self.health_status.clone().into());
        ui.set_removed(self.removed as i32);
        ui.set_copies(on && self.dupe_view().is_some());
        ui.set_quarantined(self.quarantined as i32);
        ui.set_lists(self.list_health_text().into());
        ui.set_quarantine_exists(self.quarantine_root().is_some_and(|d| d.is_dir()));
        ui.set_can_quarantine(self.saved() && self.quarantine_root().is_some());
        self.refresh_cleanup_ui();
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
                        self.in_collection(t)
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
            Check::Duplicate => {
                // Grouping reads the files, so only once it's looked at.
                let on_page = matches!(
                    self.nodes.get(self.current_node).map(|n| &n.contents),
                    Some(Contents::Health(None))
                ) || self.dupe_view().is_some();
                let groups = if on_page {
                    Some(self.dupe_groups())
                } else {
                    self.dupe_groups.get().map(Vec::as_slice)
                };
                let Some(groups) = groups.filter(|g| !g.is_empty()) else {
                    return String::new();
                };
                let easy = groups.iter().filter(|g| g.case.easy()).count();
                let versions = groups.len() - easy;
                let mut s = format!("{easy} easy to fix at once");
                if versions > 0 {
                    s += &format!(", {versions} of different lengths to look at");
                }
                s + "."
            }
            Check::Stray if !self.folders.is_set() => {
                "Set your stage folders in Settings › Collection first.".into()
            }
            Check::Outside
                if settings::music_folders().is_empty() && self.folders.all().is_empty() =>
            {
                "Add a music folder in Settings › Collection first.".into()
            }
            _ => String::new(),
        }
    }

    /// Looks at the albums again for "Album tags differ", after album tags
    /// changed.
    pub(super) fn refresh_album_tags(&mut self) {
        let Some(albums) = self.folders.albums.clone() else {
            return;
        };
        let tracks: Vec<Arc<Track>> = self
            .library
            .iter()
            .filter(|t| self.in_collection(t))
            .cloned()
            .collect();
        self.health.set_albums(&albums, tracks.iter().map(|t| &**t));
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
                format!("{} moved to In Prep.", tracks(indices.len()))
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
            Fix::AllEasy => {
                let plan = self
                    .picked_groups(indices, Case::easy)
                    .iter()
                    .map(|g| self.merge_plan(g, g.tracks[0]))
                    .collect();
                self.merge(plan)
            }
            Fix::KeepBest => {
                let case = self.dupe_view().flatten();
                let plan = self
                    .picked_groups(indices, |c| case.map_or(c.easy(), |x| x == c))
                    .iter()
                    .map(|g| self.merge_plan(g, g.tracks[0]))
                    .collect();
                self.merge(plan)
            }
            Fix::LinkVersions => self.link_versions(indices),
            Fix::CleanUp => self.open_cleanup(indices),
            Fix::FindCovers => self.batch_covers(indices),
            Fix::LookUp => self.batch_lookup(indices),
            Fix::SameAlbum => self.review_album_tags(indices),
        };
        // Removed tracks leave dead entries in lists.
        self.refresh_crates();
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
        self.refresh_dupes();
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
                    "{} removed from your collection. The files are still there.",
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
                format!("{} back in your collection.", tracks(n))
            }
            Err(e) => format!("Couldn't bring them back: {e}"),
        };
        self.refresh_tree();
        self.refresh_rows();
    }

    /// Reads the library again (after tracks left it or came back),
    /// staying on the node shown.
    pub(super) fn reload_library(&mut self) {
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
                    self.nodes[top].expanded |= matches!(
                        self.nodes[n].contents,
                        Contents::Health(_) | Contents::Dupes(_)
                    );
                }
                if matches!(self.nodes[n].contents, Contents::Dupes(_)) {
                    if let Some(d) = self.nodes.iter().position(|n| {
                        matches!(n.contents, Contents::Health(Some(Check::Duplicate)))
                    }) {
                        self.nodes[d].expanded = true;
                    }
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
    /// folders, and points each track with exactly one match at it.
    fn relink(&mut self, indices: &[usize]) -> String {
        let mut roots: Vec<PathBuf> = self
            .folders
            .all()
            .into_iter()
            .map(Path::to_path_buf)
            .collect();
        roots.extend(settings::music_folders());
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
            s += &format!(" {taken} matched a track your collection has already: see Duplicates.");
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
        assert_eq!(nodes.len(), Check::ALL.len() + Case::ALL.len() + 1);
        assert_eq!(nodes[0].name, "Overview");
        assert!(nodes.iter().all(|n| n.depth <= 1));
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
