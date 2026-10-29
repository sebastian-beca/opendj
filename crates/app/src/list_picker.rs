//! The tree of another app's lists in the import wizard, each with a tick
//! box: a folder's ticks or unticks everything in it, and "All lists" at the
//! top does the lot.

use std::collections::HashMap;

use opendj_plugins::{ImportedList, LibraryImport, ListGroup};
use std::path::Path;

/// One row of the tree: a folder (with children) or a list.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub title: String,
    pub depth: usize,
    pub children: Vec<usize>,
    /// A list's [`ImportedList::key`] and the group it is read with.
    pub list: Option<(String, &'static str)>,
    /// For a list: whether it comes in. For a folder: unused.
    pub on: bool,
    /// For a list: whether it comes in when the DJ hasn't said.
    pub default_on: bool,
    /// For a list: what it holds. For a folder: unused.
    pub detail: String,
    /// Whether a folder shows what's in it.
    pub open: bool,
}

/// How much of a folder is ticked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ticked {
    All,
    Some,
    None,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Picker {
    /// The root ("All lists") first; children after their parent.
    pub nodes: Vec<Node>,
    /// The groups that had no lists to show, as they were.
    pub empty_groups: Vec<(&'static str, bool)>,
}

impl Picker {
    /// The lists `importer` finds in `database`, ticked by the DJ's
    /// `choices` and the `groups` turned on.
    pub fn read(
        importer: &dyn LibraryImport,
        database: &Path,
        groups: &[&'static str],
        choices: &HashMap<String, bool>,
    ) -> Result<Self, String> {
        let mut found = Vec::new();
        for g in importer.list_groups() {
            let lists = importer.lists(database, &[g.id])?;
            found.push((g, lists));
        }
        Ok(Self::new(found, groups, choices))
    }

    pub fn new(
        found: Vec<(ListGroup, Vec<ImportedList>)>,
        groups: &[&'static str],
        choices: &HashMap<String, bool>,
    ) -> Self {
        let mut p = Picker {
            nodes: vec![folder("All lists", 0, true)],
            empty_groups: Vec::new(),
        };
        for (g, lists) in found {
            let group_on = groups.contains(&g.id);
            if lists.is_empty() {
                p.empty_groups.push((g.id, group_on));
                continue;
            }
            let top = p.add(0, folder(g.title, 1, false));
            // VirtualDJ keeps every list of a group in one folder of its own
            // ("Playlists", "Folders"): that folder is the group itself. A
            // folder the DJ made stays, even when it holds every list.
            let first = lists[0].folders.first().cloned();
            let shared = first
                .as_deref()
                .is_some_and(|f| f == g.title || f == "Folders")
                && lists.iter().all(|l| l.folders.first() == first.as_ref());
            let mut folders: HashMap<Vec<String>, usize> = HashMap::new();
            for l in &lists {
                let mut path: Vec<String> =
                    l.folders.iter().skip(shared as usize).cloned().collect();
                // History by year, so years of sessions stay short.
                if let (true, opendj_engine::import::ImportedListKind::History { started }) =
                    (path.is_empty(), &l.kind)
                {
                    if let Some(year) = year_of(*started) {
                        path.push(year.to_string());
                    }
                }
                let mut parent = top;
                for depth in 1..=path.len() {
                    let at = path[..depth].to_vec();
                    parent = match folders.get(&at) {
                        Some(&i) => i,
                        None => {
                            let i = p.add(parent, folder(&path[depth - 1], depth + 1, false));
                            folders.insert(at, i);
                            i
                        }
                    };
                }
                let key = l.key();
                // Streamed tracks (netsearch://, a store's link) don't come
                // in, so a list of only those starts unticked.
                let here = l
                    .entries
                    .iter()
                    .filter(|e| !e.path.to_string_lossy().contains("://"))
                    .count();
                let online_only = here == 0 && !l.entries.is_empty();
                let default_on = l.wanted_by_default() && !online_only;
                let on = group_on && choices.get(&key).copied().unwrap_or(default_on);
                let detail = match l.kind {
                    opendj_engine::import::ImportedListKind::Smart { .. } => {
                        "Smart list".to_string()
                    }
                    _ if online_only => "Online only".to_string(),
                    _ if here == 1 => "1 track".to_string(),
                    _ => format!("{here} tracks"),
                };
                let depth = path.len() + 2;
                p.add(
                    parent,
                    Node {
                        title: l.name.clone(),
                        depth,
                        children: Vec::new(),
                        list: Some((key, g.id)),
                        on,
                        default_on,
                        detail,
                        open: false,
                    },
                );
            }
        }
        p
    }

    fn add(&mut self, parent: usize, node: Node) -> usize {
        self.nodes.push(node);
        let i = self.nodes.len() - 1;
        self.nodes[parent].children.push(i);
        i
    }

    /// The lists under `node` (itself, for a list).
    fn lists_under(&self, node: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut stack = vec![node];
        while let Some(n) = stack.pop() {
            if self.nodes[n].list.is_some() {
                out.push(n);
            }
            stack.extend(self.nodes[n].children.iter().rev());
        }
        out
    }

    pub fn ticked(&self, node: usize) -> Ticked {
        let lists = self.lists_under(node);
        let on = lists.iter().filter(|&&i| self.nodes[i].on).count();
        match on {
            0 => Ticked::None,
            n if n == lists.len() => Ticked::All,
            _ => Ticked::Some,
        }
    }

    /// Ticks or unticks `node` and everything in it.
    pub fn set(&mut self, node: usize, on: bool) {
        for i in self.lists_under(node) {
            self.nodes[i].on = on;
        }
    }

    pub fn toggle_open(&mut self, node: usize) {
        if let Some(n) = self.nodes.get_mut(node) {
            n.open = !n.open;
        }
    }

    /// The rows shown, in order: each open folder's children after it.
    pub fn visible(&self) -> Vec<usize> {
        let mut out = Vec::new();
        let mut stack = vec![0];
        while let Some(n) = stack.pop() {
            out.push(n);
            if self.nodes[n].open {
                stack.extend(self.nodes[n].children.iter().rev());
            }
        }
        out
    }

    /// The groups to read: those with a ticked list, and empty ones as they
    /// were.
    pub fn groups(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for n in &self.nodes {
            if let (Some((_, g)), true) = (&n.list, n.on) {
                if !out.contains(g) {
                    out.push(g);
                }
            }
        }
        out.extend(
            self.empty_groups
                .iter()
                .filter(|(_, on)| *on)
                .map(|(g, _)| *g),
        );
        out
    }

    /// The choices to save: the lists ticked differently from what comes in
    /// by default.
    pub fn choices(&self) -> HashMap<String, bool> {
        self.nodes
            .iter()
            .filter_map(|n| {
                let (key, _) = n.list.as_ref()?;
                (n.on != n.default_on).then(|| (key.clone(), n.on))
            })
            .collect()
    }

    /// "12 of 40 lists".
    pub fn summary(&self) -> String {
        let all = self.lists_under(0);
        let on = all.iter().filter(|&&i| self.nodes[i].on).count();
        format!("{on} of {} lists", all.len())
    }
}

fn folder(title: &str, depth: usize, open: bool) -> Node {
    Node {
        title: title.to_string(),
        depth,
        children: Vec::new(),
        list: None,
        on: false,
        default_on: false,
        detail: String::new(),
        open,
    }
}

/// The year a Unix time falls in (UTC), for times after 1970.
fn year_of(t: i64) -> Option<i64> {
    if t <= 0 {
        return None;
    }
    // Days to a civil date (Howard Hinnant's algorithm).
    let z = t.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    Some(yoe + era * 400 + (month <= 2) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendj_engine::import::{ImportedEntry, ImportedListKind};

    fn list(folders: &[&str], name: &str, kind: ImportedListKind) -> ImportedList {
        ImportedList {
            folders: folders.iter().map(|f| f.to_string()).collect(),
            name: name.into(),
            kind,
            entries: vec![ImportedEntry {
                path: "/m/a.mp3".into(),
                played_at: None,
            }],
        }
    }

    fn group(id: &'static str, title: &'static str) -> ListGroup {
        ListGroup {
            id,
            title,
            detail: "",
            on_by_default: true,
        }
    }

    fn picker(choices: &HashMap<String, bool>) -> Picker {
        Picker::new(
            vec![
                (
                    group("playlists", "Playlists"),
                    vec![
                        list(&["Playlists", "Sets"], "Friday", ImportedListKind::List),
                        list(&["Playlists", "Sets"], "Saturday", ImportedListKind::List),
                        list(&["Playlists"], "Warm up", ImportedListKind::List),
                    ],
                ),
                (
                    group("filters", "Filter folders"),
                    vec![list(
                        &["Folders"],
                        "Old ones",
                        ImportedListKind::Smart {
                            query: "year:<2000".into(),
                        },
                    )],
                ),
                (
                    group("history", "History"),
                    vec![list(
                        &[],
                        "2023-11-13",
                        ImportedListKind::History {
                            started: 1_699_900_000,
                        },
                    )],
                ),
                (group("folders", "Virtual folders"), Vec::new()),
            ],
            &["playlists", "filters", "history"],
            choices,
        )
    }

    fn titles(p: &Picker) -> Vec<String> {
        p.visible()
            .into_iter()
            .map(|i| format!("{}{}", "  ".repeat(p.nodes[i].depth), p.nodes[i].title))
            .collect()
    }

    #[test]
    fn groups_folders_and_years_make_the_tree() {
        let mut p = picker(&HashMap::new());
        assert_eq!(
            titles(&p),
            ["All lists", "  Playlists", "  Filter folders", "  History"]
        );
        for i in 0..p.nodes.len() {
            p.nodes[i].open = true;
        }
        assert_eq!(
            titles(&p),
            [
                "All lists",
                "  Playlists",
                "    Sets",
                "      Friday",
                "      Saturday",
                "    Warm up",
                "  Filter folders",
                "    Old ones",
                "  History",
                "    2023",
                "      2023-11-13",
            ]
        );
        // Smart lists start unticked.
        assert_eq!(p.ticked(0), Ticked::Some);
        assert_eq!(p.ticked(1), Ticked::All);
        assert_eq!(p.ticked(p.nodes[0].children[1]), Ticked::None);
    }

    #[test]
    fn a_folder_ticks_its_whole_branch() {
        let mut p = picker(&HashMap::new());
        let sets = p.nodes[1].children[0];
        p.set(sets, false);
        assert_eq!(p.ticked(sets), Ticked::None);
        assert_eq!(p.ticked(1), Ticked::Some);
        p.set(0, true);
        assert_eq!(p.ticked(0), Ticked::All);
        assert_eq!(p.summary(), "5 of 5 lists");
        p.set(0, false);
        // Nothing ticked: no group is read, but an empty one keeps its setting.
        assert!(p.groups().is_empty());
    }

    #[test]
    fn only_changes_from_the_defaults_are_saved() {
        let mut p = picker(&HashMap::new());
        let sets = p.nodes[1].children[0];
        p.set(sets, false);
        let smart = p.nodes[0].children[1];
        p.set(smart, true);
        let choices = p.choices();
        assert_eq!(choices.len(), 3, "{choices:?}");
        assert!(!choices["list:Playlists/Sets/Friday"]);
        assert!(choices["smart:Folders/Old ones"]);
        // Read again with them, the tree is ticked the same way.
        let again = picker(&choices);
        assert_eq!(again.ticked(again.nodes[1].children[0]), Ticked::None);
        assert_eq!(again.ticked(again.nodes[0].children[1]), Ticked::All);
        assert_eq!(p.groups(), vec!["playlists", "filters", "history"]);
    }

    #[test]
    fn lists_of_only_online_tracks_start_unticked() {
        let mut streams = list(&["Playlists"], "Streams", ImportedListKind::List);
        streams.entries[0].path = "netsearch://td1".into();
        let p = Picker::new(
            vec![(
                group("playlists", "Playlists"),
                vec![
                    streams,
                    list(&["Playlists"], "Mine", ImportedListKind::List),
                ],
            )],
            &["playlists"],
            &HashMap::new(),
        );
        let streams = &p.nodes[p.nodes[1].children[0]];
        assert_eq!(
            (streams.on, streams.detail.as_str()),
            (false, "Online only")
        );
        assert!(p.nodes[p.nodes[1].children[1]].on);
        // Nothing to save: unticked is its default.
        assert!(p.choices().is_empty());
    }
}
