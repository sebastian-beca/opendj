//! Collection Health in the browser (see [`opendj_library::health`]): a node in
//! the collection's tree with one node per check under it, the checks'
//! page, and the fixes each check offers for its tracks.

use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use opendj_engine::{tags, Track};
use opendj_library::dupes::{self, Case, FileFacts, Group};
use opendj_library::health::{self, Area, Check, Facts};
use opendj_library::query::Indexed;
use opendj_library::stages::Stage;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::{file_path, is_browsed, Browser, Contents, Node, Undo, KIND_AREA, KIND_HEALTH};
use crate::cover_scan;
use crate::{settings, HealthArea, HealthCard, HealthChip, HealthLine, HealthStage, HealthUi};

/// The word the "Tag as mix" fix adds to a track's first tag field.
const MIX_TAG: &str = "mix";

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
            Fix::FillFromName => "Fill from file name…",
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
        Check::MissingInfo => vec![Fix::LookUp, Fix::FillFromName],
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

/// What one of an area's chips lists: a check's tracks, or one case of
/// copies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Chip {
    Check(Check),
    Case(Case),
}

impl Chip {
    fn title(self) -> &'static str {
        match self {
            Chip::Check(Check::Duplicate) => "All copies",
            Chip::Check(c) => c.title(),
            Chip::Case(c) => c.title(),
        }
    }

    fn contents(self) -> Contents {
        match self {
            Chip::Check(c) => Contents::Health(Some(c)),
            Chip::Case(c) => Contents::Dupes(c),
        }
    }
}

/// An area's chips: one per check, and for Duplicates all copies, then
/// each case.
pub(super) fn area_chips(area: Area) -> Vec<Chip> {
    let mut chips: Vec<Chip> = area.checks().into_iter().map(Chip::Check).collect();
    if area == Area::Duplicates {
        chips.extend(Case::ALL.map(Chip::Case));
    }
    chips
}

/// How an area stands, for its dot and tile: nothing found, something to
/// look at, something that won't play, or a goal (Prep).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum AreaState {
    Clear = 0,
    Check = 1,
    Problem = 2,
    Goal = 3,
}

fn area_state(area: Area, count: usize) -> AreaState {
    match (area, count) {
        (_, 0) => AreaState::Clear,
        (Area::Files, _) => AreaState::Problem,
        (Area::Prep, _) => AreaState::Goal,
        _ => AreaState::Check,
    }
}

/// The area to start with: the first whose findings are problems, else
/// Prep when it has any.
fn start_area(counts: &[usize; 5]) -> Option<usize> {
    let first = |problem: bool| {
        Area::ALL
            .iter()
            .zip(counts)
            .position(|(a, &n)| a.problem() == problem && n > 0)
    };
    first(true).or_else(|| first(false))
}

/// Collection Health's tree: the overview page, then a node per area.
pub(super) fn health_nodes() -> Vec<Node> {
    let mut nodes = vec![Node::new(
        "Overview",
        0,
        KIND_HEALTH,
        Contents::Health(None),
    )];
    nodes.extend(
        Area::ALL
            .iter()
            .map(|&a| Node::new(a.title(), 0, KIND_AREA, Contents::Area(a))),
    );
    nodes
}

/// What the dashboard counts in each stage.
#[derive(Default)]
struct StageCount {
    tracks: usize,
    clean: usize,
    /// Tracks each check finds, by its index in `Check::ALL`.
    found: [usize; 13],
}

/// "86%" of `part` in `whole`, rounded down so 99.6% isn't "100%".
fn percent(part: usize, whole: usize) -> i32 {
    (part * 100).checked_div(whole).map_or(100, |p| p as i32)
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
        ui.on_open_area(move |i| {
            w(&|b| {
                if let Some(&a) = usize::try_from(i).ok().and_then(|i| Area::ALL.get(i)) {
                    b.open_area(a, None)
                }
            })
        });
        let w = with.clone();
        ui.on_pick_chip(move |i| {
            w(&|b| {
                let Some(Contents::Area(a)) =
                    b.nodes.get(b.current_node).map(|n| n.contents.clone())
                else {
                    return;
                };
                if let Ok(i) = usize::try_from(i) {
                    b.open_area(a, Some(i));
                }
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

    /// What a node lists, with an area taken as the check its chip picks.
    pub(super) fn resolved(&self, contents: &Contents) -> Contents {
        match contents {
            Contents::Area(a) => area_chips(*a)[self.chip_of(*a)].contents(),
            c => c.clone(),
        }
    }

    /// What the selected node lists (see [`Self::resolved`]).
    pub(super) fn current_shown(&self) -> Option<Contents> {
        self.nodes
            .get(self.current_node)
            .map(|n| self.resolved(&n.contents))
    }

    /// The chip shown for an area: the one picked, else the first check
    /// with tracks (copies are never counted for this: that reads files).
    fn chip_of(&self, area: Area) -> usize {
        let chips = area_chips(area);
        if let Some(&i) = self.area_chip.get(&area).filter(|&&i| i < chips.len()) {
            return i;
        }
        chips
            .iter()
            .position(|c| matches!(c, Chip::Check(check) if self.health_count(*check) > 0))
            .unwrap_or(0)
    }

    /// Tracks any of an area's checks finds.
    pub(super) fn area_count(&self, area: Area) -> usize {
        let checks = area.checks();
        self.library
            .iter()
            .filter(|t| {
                self.in_collection(t)
                    && checks
                        .iter()
                        .any(|&c| self.health.finds(c, t, &self.duplicates))
            })
            .count()
    }

    /// How an area stands (see [`AreaState`]).
    pub(super) fn area_state(&self, area: Area) -> AreaState {
        area_state(area, self.area_count(area))
    }

    /// Selects an area's node, with its chip `chip` (or the one it had).
    fn open_area(&mut self, area: Area, chip: Option<usize>) {
        self.show_health(true);
        let Some(n) = self
            .nodes
            .iter()
            .position(|n| matches!(n.contents, Contents::Area(a) if a == area))
        else {
            return;
        };
        if let Some(c) = chip {
            self.area_chip.insert(area, c);
        }
        let now = self.area_count(area);
        let start = self.area_start.entry(area).or_insert(now);
        *start = (*start).max(now);
        self.health_status.clear();
        self.current_node = n;
        self.folders_focused = false;
        if let Some(app) = self.app.upgrade() {
            app.set_current_track(-1);
        }
        self.refresh_tree();
        self.refresh_rows();
    }

    /// The folders missing files are looked for in.
    fn relink_roots(&self) -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = self
            .folders
            .all()
            .into_iter()
            .map(Path::to_path_buf)
            .collect();
        roots.extend(settings::music_folders());
        roots
    }

    /// The one file a missing track's name matches, if exactly one does.
    fn relink_match(&self, t: &Track) -> Result<&Path, usize> {
        let found = self
            .relink_files
            .get_or_init(|| files_by_name(&self.relink_roots()));
        let name = file_path(t)
            .and_then(Path::file_name)
            .map(|n| n.to_string_lossy().to_lowercase());
        match name.and_then(|n| found.get(&n)).map(Vec::as_slice) {
            Some([one]) => Ok(one),
            Some(many) => Err(many.len()),
            None => Err(0),
        }
    }

    /// A Collection Health area's Problem and Fix cells for `t`: what the
    /// listed check found, and what its first fix would do.
    pub(super) fn health_cells(&self, t: &Track) -> (String, String) {
        let Some(check) = self.listed_check() else {
            return Default::default();
        };
        let short = |p: &Path| {
            let parts: Vec<String> = p
                .components()
                .rev()
                .take(2)
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            parts.into_iter().rev().collect::<Vec<_>>().join("/")
        };
        let s = |a: &str, b: &str| (a.to_string(), b.to_string());
        match check {
            Check::Missing => match self.relink_match(t) {
                Ok(p) => ("File not found".into(), format!("Relink to {}", short(p))),
                Err(0) => s(
                    "File not found",
                    "Not found in your folders: find it or remove it",
                ),
                Err(n) => (
                    "File not found".into(),
                    format!("{n} files match: pick one"),
                ),
            },
            Check::LowBitrate => (
                self.health
                    .bitrate(t.id)
                    .map_or_else(|| "Low bitrate".into(), |b| format!("{b} kb/s")),
                "Find a better copy".into(),
            ),
            Check::NotMusic => s("Audiobook or podcast", "Remove from collection"),
            Check::Stray if self.folders.is_set() => {
                s("Outside the stage folders", "Move to In Prep")
            }
            Check::Stray => s("Outside the stage folders", "Remove from collection"),
            Check::Outside => s(
                "Outside your music folders",
                if self.folders.is_set() {
                    "Move to In Prep"
                } else {
                    "Remove from collection"
                },
            ),
            Check::Long => (
                format!("{} min", (t.duration / 60.0).round() as i64),
                format!("Tag as {MIX_TAG}"),
            ),
            Check::Duplicate => {
                let Some(g) = self.dupe_groups().iter().find(|g| g.tracks.contains(&t.id)) else {
                    return s("Copy", "");
                };
                if g.tracks[0] == t.id {
                    (
                        format!("{}: kept", g.case.title()),
                        "Keeps the others' cues and plays".into(),
                    )
                } else {
                    let keep = self
                        .index_of
                        .get(&g.tracks[0])
                        .and_then(|&i| file_path(&self.library[i]))
                        .map(short)
                        .unwrap_or_default();
                    (g.case.title().into(), format!("Merge into {keep}"))
                }
            }
            Check::MissingInfo => {
                let lacking: Vec<&str> = [
                    ("artist", &t.artist),
                    ("title", &t.title),
                    ("album", &t.album),
                    ("year", &t.info.year),
                    ("genre", &t.genre),
                ]
                .into_iter()
                .filter(|(_, v)| health::says_nothing(v))
                .map(|(what, _)| what)
                .collect();
                let problem = format!("No {}", lacking.join(", "));
                let named = file_path(t).and_then(health::from_file_name);
                let fix = match named {
                    Some((artist, title))
                        if health::says_nothing(&t.artist) || health::says_nothing(&t.title) =>
                    {
                        format!("{artist} – {title}, from the file name")
                    }
                    _ => "Look up online".into(),
                };
                (problem, fix)
            }
            Check::Junk => {
                let changes = self.health.junk(t);
                let fix = changes
                    .iter()
                    .map(|c| format!("{} → {}", c.old, c.new))
                    .collect::<Vec<_>>()
                    .join("; ");
                ("Junk in names".into(), fix)
            }
            Check::AlbumTags => (
                if t.album.trim().is_empty() {
                    "No album tag".into()
                } else {
                    format!("Album: {}", t.album)
                },
                self.health
                    .album_tag(t.id)
                    .map(|a| format!("Album: {a}"))
                    .unwrap_or_default(),
            ),
            Check::NoCover => s("No cover", "Look online for one"),
            Check::NotAnalyzed => s("No tempo or key", "Analyze"),
            Check::NoRating => s("No stars", "Rate it"),
        }
    }

    /// How far an area has come since it was first opened: tracks fixed,
    /// of those it found then.
    fn area_progress(&self, area: Area) -> (usize, usize) {
        let now = self.area_count(area);
        let start = self.area_start.get(&area).copied().unwrap_or(now).max(now);
        (start - now, start)
    }

    /// The check whose tracks are listed: a case of copies is Duplicates'.
    fn listed_check(&self) -> Option<Check> {
        match self.current_shown()? {
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
        match self.current_shown()? {
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
            "Linked {} as versions. Each shows the others on its Related tab.",
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

    /// Library tracks (indices into `library`) on rows `first` to `last`
    /// (every picked row when `first` is one of several picked), or on
    /// every row when `first` is negative.
    pub(super) fn picked_rows(&self, first: i32, last: i32) -> Vec<usize> {
        if first < 0 {
            return self.rows.iter().copied().flatten().collect();
        }
        self.chosen_tracks(first as usize, last.max(first) as usize)
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
            self.current_shown(),
            Some(Contents::Health(Some(Check::NoCover | Check::LowBitrate)))
        ) {
            self.refresh_rows();
        }
    }

    /// Selects a check's area with that check's chip, or the Overview.
    fn review(&mut self, check: Option<Check>) {
        let Some(check) = check else {
            self.show_health(true);
            if let Some(n) = self
                .nodes
                .iter()
                .position(|n| matches!(n.contents, Contents::Health(None)))
            {
                self.health_status.clear();
                self.current_node = n;
                self.refresh_tree();
                self.refresh_rows();
            }
            return;
        };
        let area = check.area();
        let chip = area_chips(area)
            .iter()
            .position(|&c| c == Chip::Check(check));
        self.open_area(area, chip);
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
                } else if Some(c) == listed && c == Check::Missing {
                    // Only files found once by name are relinked: the sure ones.
                    let sure = self
                        .rows
                        .iter()
                        .flatten()
                        .filter(|&&i| self.relink_match(&self.library[i]).is_ok())
                        .count();
                    let listed = self.rows.iter().flatten().count();
                    offered
                        .iter()
                        .map(|&f| if f == Fix::Relink { sure } else { listed } as i32)
                        .collect()
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
        ui.set_view(self.health_view && self.tree == super::COLLECTION);
        ui.set_cards(ModelRc::new(VecModel::from(cards)));
        ui.set_status(self.health_status.clone().into());
        ui.set_removed(self.removed as i32);
        ui.set_copies(on && self.dupe_view().is_some());
        ui.set_quarantined(self.quarantined as i32);
        ui.set_lists(self.list_health_text().into());
        ui.set_quarantine_exists(self.quarantine_root().is_some_and(|d| d.is_dir()));
        ui.set_can_quarantine(self.saved() && self.quarantine_root().is_some());
        self.refresh_dashboard(&ui);
        self.refresh_chips(&ui);
        self.refresh_cleanup_ui();
    }

    /// The Overview's numbers: how much is clean, by stage, and each area.
    fn refresh_dashboard(&self, ui: &HealthUi) {
        let stages = [Stage::Inbox, Stage::Triage, Stage::Collection];
        let mut by_stage: [StageCount; 3] = Default::default();
        let (mut total, mut clean) = (0, 0);
        let mut areas = [0usize; 5];
        for t in self.library.iter().filter(|t| self.in_collection(t)) {
            if t.info.stage == Stage::Dismissed.name() {
                continue;
            }
            let found: Vec<bool> = Check::ALL
                .iter()
                .map(|&c| self.health.finds(c, t, &self.duplicates))
                .collect();
            let problem = Check::ALL
                .iter()
                .zip(&found)
                .any(|(c, &f)| f && c.area().problem());
            for (i, a) in Area::ALL.iter().enumerate() {
                if Check::ALL
                    .iter()
                    .zip(&found)
                    .any(|(c, &f)| f && c.area() == *a)
                {
                    areas[i] += 1;
                }
            }
            total += 1;
            clean += usize::from(!problem);
            if let Some(s) = stages.iter().position(|s| t.info.stage == s.name()) {
                let count = &mut by_stage[s];
                count.tracks += 1;
                count.clean += usize::from(!problem);
                for (n, &f) in count.found.iter_mut().zip(&found) {
                    *n += usize::from(f);
                }
            }
        }
        ui.set_total(total as i32);
        ui.set_clean(clean as i32);
        ui.set_clean_percent(percent(clean, total));
        let stage_rows: Vec<HealthStage> = if self.folders.is_set() {
            stages
                .iter()
                .zip(&by_stage)
                .map(|(stage, count)| {
                    // The check finding most of the stage's problems.
                    let top = Check::ALL
                        .iter()
                        .zip(count.found)
                        .filter(|(c, n)| c.area().problem() && *n > 0)
                        .max_by_key(|&(_, n)| n);
                    HealthStage {
                        title: stage.title().into(),
                        count: count.tracks as i32,
                        percent: percent(count.clean, count.tracks),
                        note: top
                            .map(|(c, n)| format!("{n} {}", c.title().to_lowercase()))
                            .unwrap_or_default()
                            .into(),
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        ui.set_stages(ModelRc::new(VecModel::from(stage_rows)));
        let start = start_area(&areas);
        let tiles: Vec<HealthArea> = Area::ALL
            .iter()
            .enumerate()
            .map(|(i, &a)| {
                let lines: Vec<HealthLine> = a
                    .checks()
                    .into_iter()
                    .map(|c| HealthLine {
                        title: c.title().into(),
                        count: self.health_count(c) as i32,
                        check: Check::ALL.iter().position(|&x| x == c).unwrap_or(0) as i32,
                    })
                    .collect();
                HealthArea {
                    title: a.title().into(),
                    question: a.question().into(),
                    count: areas[i] as i32,
                    state: area_state(a, areas[i]) as i32,
                    start: start == Some(i),
                    lines: ModelRc::new(VecModel::from(lines)),
                }
            })
            .collect();
        ui.set_areas(ModelRc::new(VecModel::from(tiles)));
    }

    /// The chips over an area's tracks, when one is selected.
    fn refresh_chips(&self, ui: &HealthUi) {
        let area = match self.nodes.get(self.current_node).map(|n| &n.contents) {
            Some(Contents::Area(a)) if self.health_view => Some(*a),
            _ => None,
        };
        let chips: Vec<HealthChip> = area
            .map(|a| {
                area_chips(a)
                    .into_iter()
                    .map(|c| HealthChip {
                        title: c.title().into(),
                        count: match c {
                            Chip::Check(check) => self.health_count(check),
                            Chip::Case(case) => self.dupe_tracks(Some(case)).len(),
                        } as i32,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let (fixed, of) = area.map_or((0, 0), |a| self.area_progress(a));
        ui.set_fixed(fixed as i32);
        ui.set_fixed_of(of as i32);
        ui.set_chip(area.map_or(-1, |a| self.chip_of(a) as i32));
        ui.set_chips(ModelRc::new(VecModel::from(chips)));
    }

    /// A line on what a check found beyond its count.
    fn health_detail(&self, check: Check) -> String {
        match check {
            Check::MissingInfo => {
                // What the flagged tracks lack, the most often lacked first.
                let found = self.health_tracks(check);
                let lacking = |f: &dyn Fn(&Track) -> &str| {
                    found
                        .iter()
                        .filter(|&&i| health::says_nothing(f(&self.library[i])))
                        .count()
                };
                let mut counts = [
                    (
                        "artist or title",
                        lacking(&|t| {
                            if health::says_nothing(&t.artist) {
                                ""
                            } else {
                                &t.title
                            }
                        }),
                    ),
                    ("album", lacking(&|t| &t.album)),
                    ("year", lacking(&|t| &t.info.year)),
                    ("genre", lacking(&|t| &t.genre)),
                ];
                counts.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
                let parts: Vec<String> = counts
                    .iter()
                    .filter(|&&(_, n)| n > 0)
                    .map(|(what, n)| format!("{n} no {what}"))
                    .collect();
                if parts.is_empty() {
                    String::new()
                } else {
                    format!("{}.", parts.join(", "))
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
            Fix::FillFromName => self.review_file_names(indices),
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

    /// Looks for missing files by name in the stage folders and the music
    /// folders, and points each track with exactly one match at it.
    fn relink(&mut self, indices: &[usize]) -> String {
        let found = files_by_name(&self.relink_roots());
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
        self.relink_files = OnceCell::new();
        let Some(db) = &mut self.db else {
            return String::new();
        };
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

    /// Tags long tracks "mix", in the first tag group.
    fn tag_mix(&mut self, indices: &[usize]) -> String {
        let Some(group) = opendj_library::query::group_at(0) else {
            return "Make a tag group first, in Settings › Collection › Tags.".into();
        };
        let mut n = 0;
        for &i in indices {
            let mut t = (*self.library[i]).clone();
            if t.info.has_tag(group, MIX_TAG) {
                continue;
            }
            t.info.add_tag(group, MIX_TAG, false);
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
        format!("Tagged {} as \"{MIX_TAG}\".", tracks(n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_check_has_a_node_and_fixes_fit() {
        let nodes = health_nodes();
        assert_eq!(nodes.len(), Area::ALL.len() + 1);
        assert_eq!(nodes[0].name, "Overview");
        assert!(nodes.iter().all(|n| n.depth == 0));
        let chips: usize = Area::ALL.iter().map(|&a| area_chips(a).len()).sum();
        assert_eq!(chips, Check::ALL.len() + Case::ALL.len());
        assert_eq!(fixes(Check::Stray, false), [Fix::Remove]);
        assert_eq!(fixes(Check::Stray, true)[0], Fix::ToTriage);
    }

    #[test]
    fn starts_with_the_first_problem_then_prep() {
        assert_eq!(start_area(&[0, 28, 0, 5, 9]), Some(1));
        assert_eq!(start_area(&[0, 0, 0, 0, 9]), Some(4));
        assert_eq!(start_area(&[0; 5]), None);
        assert_eq!(area_state(Area::Files, 3), AreaState::Problem);
        assert_eq!(area_state(Area::Prep, 3), AreaState::Goal);
        assert_eq!(area_state(Area::Info, 0), AreaState::Clear);
        assert_eq!(percent(862, 1105), 78);
        assert_eq!(percent(0, 0), 100);
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
