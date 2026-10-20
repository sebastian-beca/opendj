//! VirtualDJ's playlists, folders and history, kept next to `database.xml`:
//!
//! - `Playlists/`: one `.m3u` per playlist, in folders. An `order` file in
//!   each folder lists its entries in the order VirtualDJ shows them.
//! - `Folders/`: `.vdjfolder` files, each either a virtual folder (a fixed
//!   list of songs) or a filter folder (a search in VirtualDJ's filter
//!   language), with a `<name>.subfolders/` folder for what sits inside.
//! - `History/`: one `.m3u` per day played, each line with when it played.
//!
//! Streamed tracks in them (`netsearch://…`) are kept as entries; the
//! library leaves out what it has no file for.

use std::path::{Path, PathBuf};

use mixxx_engine::import::{ImportedEntry, ImportedList, ImportedListKind, ListGroup};

pub const PLAYLISTS: &str = "playlists";
pub const FOLDERS: &str = "folders";
pub const FILTERS: &str = "filters";
pub const HISTORY: &str = "history";

/// The kinds of lists the DJ can bring in.
pub fn groups() -> Vec<ListGroup> {
    vec![
        ListGroup {
            id: PLAYLISTS,
            title: "Playlists",
            detail: "Your VirtualDJ playlists, in their folders.",
            on_by_default: true,
        },
        ListGroup {
            id: FOLDERS,
            title: "Virtual folders",
            detail: "Folders of songs you put together in VirtualDJ's browser.",
            on_by_default: true,
        },
        ListGroup {
            id: FILTERS,
            title: "Filter folders",
            detail: "As smart lists, where openDJ's search can say the same \
                     thing; VirtualDJ's built-in ones too.",
            on_by_default: false,
        },
        ListGroup {
            id: HISTORY,
            title: "History",
            detail: "Every session you played in VirtualDJ, in the History tab.",
            on_by_default: true,
        },
    ]
}

/// What reading the lists found besides the lists themselves.
#[derive(Debug, Default)]
pub struct Skipped {
    /// Filter folders whose search openDJ can't say (yet).
    pub filters: Vec<String>,
}

/// Reads the lists in `groups` from VirtualDJ's folder `home`.
pub fn read(home: &Path, groups: &[&str], skipped: &mut Skipped) -> Vec<ImportedList> {
    let mut out = Vec::new();
    if groups.contains(&PLAYLISTS) {
        playlists(&home.join("Playlists"), &["Playlists".into()], &mut out);
    }
    let (virtual_folders, filters) = (groups.contains(&FOLDERS), groups.contains(&FILTERS));
    if virtual_folders || filters {
        let dir = home.join("Folders");
        for name in ordered(&dir, |p| p.is_dir()) {
            // Online music is VirtualDJ's streaming catalogues.
            if name == "OnlineMusic" {
                continue;
            }
            let folders = vec!["Folders".to_string(), name.clone()];
            vdj_folders(
                &dir.join(&name),
                &folders,
                virtual_folders,
                filters,
                &mut out,
                skipped,
            );
        }
    }
    if groups.contains(&HISTORY) {
        history(&home.join("History"), &mut out);
    }
    out
}

/// The entries of `dir` that `keep` takes, by name without the extension,
/// in the order its `order` file gives; anything it leaves out follows, by
/// name.
fn ordered(dir: &Path, keep: impl Fn(&Path) -> bool) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<(String, PathBuf)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| keep(p))
        .filter_map(|p| {
            let name = p.file_name()?.to_string_lossy().into_owned();
            (!name.starts_with('.')).then_some((name, p))
        })
        .collect();
    names.sort();
    let order = std::fs::read_to_string(dir.join("order")).unwrap_or_default();
    let mut out: Vec<String> = Vec::new();
    for line in order
        .lines()
        .map(|l| l.trim_start_matches('\u{feff}').trim())
    {
        for (name, _) in &names {
            if stem(name) == line && !out.contains(name) {
                out.push(name.clone());
            }
        }
    }
    for (name, _) in names {
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// A name without its extension ("Set.m3u" is "Set"); folder names stay.
fn stem(name: &str) -> &str {
    for ext in [".m3u", ".m3u8", ".vdjfolder", ".subfolders"] {
        if let Some(s) = name.strip_suffix(ext) {
            return s;
        }
    }
    name
}

fn playlists(dir: &Path, folders: &[String], out: &mut Vec<ImportedList>) {
    for name in ordered(dir, |p| p.is_dir() || is_m3u(p)) {
        let path = dir.join(&name);
        if path.is_dir() {
            let mut inner = folders.to_vec();
            inner.push(name);
            playlists(&path, &inner, out);
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let list_name = match stem(&name) {
            // VirtualDJ's sidelist is kept as a playlist file too.
            "sidelist" => "Sidelist",
            s => s,
        };
        out.push(ImportedList {
            folders: folders.to_vec(),
            name: list_name.to_string(),
            kind: ImportedListKind::List,
            entries: m3u(&text),
        });
    }
}

fn is_m3u(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("m3u") || e.eq_ignore_ascii_case("m3u8"))
}

/// The entries of an M3U playlist. VirtualDJ writes an `#EXTVDJ:` line of
/// tags before each path, with `<lastplaytime>` in history files.
pub fn m3u(text: &str) -> Vec<ImportedEntry> {
    let mut out = Vec::new();
    let mut played_at = None;
    for line in text.lines() {
        let line = line.trim_start_matches('\u{feff}').trim();
        if line.is_empty() {
            continue;
        }
        if let Some(info) = line.strip_prefix("#EXTVDJ:") {
            played_at = between(info, "<lastplaytime>", "</lastplaytime>")
                .and_then(|t| t.trim().parse::<i64>().ok())
                .filter(|t| *t > 0);
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        out.push(ImportedEntry {
            path: PathBuf::from(line),
            played_at: played_at.take(),
        });
    }
    out
}

fn between<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = text.find(open)? + open.len();
    let end = text[start..].find(close)? + start;
    Some(&text[start..end])
}

/// One level of `Folders/`: each `X.vdjfolder` is a list (or a search), and
/// `X.subfolders/` holds what sits inside X.
fn vdj_folders(
    dir: &Path,
    folders: &[String],
    virtual_folders: bool,
    filters: bool,
    out: &mut Vec<ImportedList>,
    skipped: &mut Skipped,
) {
    let names = ordered(dir, |p| {
        p.is_dir() || p.extension().is_some_and(|e| e == "vdjfolder")
    });
    // "X.vdjfolder" and "X.subfolders" share a place in the order.
    let mut seen: Vec<String> = Vec::new();
    for name in names {
        let base = stem(&name).to_string();
        if seen.contains(&base) {
            continue;
        }
        seen.push(base.clone());
        let file = dir.join(format!("{base}.vdjfolder"));
        if let Ok(text) = std::fs::read_to_string(&file) {
            match vdj_folder(&text) {
                Some(Folder::Songs(entries)) if virtual_folders => out.push(ImportedList {
                    folders: folders.to_vec(),
                    name: base.clone(),
                    kind: ImportedListKind::List,
                    entries,
                }),
                Some(Folder::Filter(filter)) if filters => match translate_filter(&filter) {
                    Some(query) => out.push(ImportedList {
                        folders: folders.to_vec(),
                        name: base.clone(),
                        kind: ImportedListKind::Smart { query },
                        entries: Vec::new(),
                    }),
                    None => skipped.filters.push(format!("{base}: {filter}")),
                },
                _ => {}
            }
        }
        let sub = dir.join(format!("{base}.subfolders"));
        let plain = dir.join(&base);
        let inner_dir = if sub.is_dir() { sub } else { plain };
        if inner_dir.is_dir() {
            let mut inner = folders.to_vec();
            inner.push(base);
            vdj_folders(&inner_dir, &inner, virtual_folders, filters, out, skipped);
        }
    }
}

#[derive(Debug, PartialEq)]
enum Folder {
    Songs(Vec<ImportedEntry>),
    Filter(String),
}

/// A `.vdjfolder` file: `<VirtualFolder>` with a `<song path=…>` per song,
/// or `<FilterFolder filter="…">`.
fn vdj_folder(text: &str) -> Option<Folder> {
    let mut out = None;
    super::parse_xml(text.trim_start_matches('\u{feff}'), |doc| {
        out = folder(doc.root_element());
    })
    .ok()?;
    out
}

fn folder(root: roxmltree::Node) -> Option<Folder> {
    if root.has_tag_name("FilterFolder") {
        return Some(Folder::Filter(root.attribute("filter")?.trim().to_string()));
    }
    if root.has_tag_name("VirtualFolder") {
        let mut songs: Vec<(f64, ImportedEntry)> = root
            .children()
            .filter(|n| n.has_tag_name("song"))
            .enumerate()
            .filter_map(|(i, n)| {
                let idx = n
                    .attribute("idx")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(i as f64);
                Some((
                    idx,
                    ImportedEntry {
                        path: PathBuf::from(n.attribute("path")?),
                        played_at: None,
                    },
                ))
            })
            .collect();
        songs.sort_by(|a, b| a.0.total_cmp(&b.0));
        return Some(Folder::Songs(songs.into_iter().map(|(_, e)| e).collect()));
    }
    None
}

/// A history session per `.m3u` under `History/` (VirtualDJ names them by
/// day, sometimes in a folder per year or month), oldest first.
fn history(dir: &Path, out: &mut Vec<ImportedList>) {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if is_m3u(&p) {
                files.push(p);
            }
        }
    }
    let mut sessions: Vec<ImportedList> = files
        .iter()
        .filter_map(|p| {
            let text = std::fs::read_to_string(p).ok()?;
            let entries = m3u(&text);
            if entries.is_empty() {
                return None;
            }
            let name = p.file_stem()?.to_string_lossy().into_owned();
            let started = entries
                .iter()
                .filter_map(|e| e.played_at)
                .min()
                .or_else(|| day_start(&name))
                .unwrap_or(0);
            Some(ImportedList {
                folders: Vec::new(),
                name,
                kind: ImportedListKind::History { started },
                entries,
            })
        })
        .collect();
    sessions.sort_by_key(|s| match s.kind {
        ImportedListKind::History { started } => started,
        _ => 0,
    });
    out.extend(sessions);
}

/// The Unix time a day named "2023-11-13" starts (UTC).
fn day_start(name: &str) -> Option<i64> {
    let mut parts = name.get(..10)?.splitn(3, '-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let d: i64 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146_097 + doe - 719_468) * 86_400)
}

/// VirtualDJ's filter language (`genre is "house" and bpm > 120`) in
/// openDJ's search syntax, where it has the same terms: field comparisons
/// joined by `and` and `or`. Anything else (`top 10`, `group by`,
/// brackets, fields openDJ doesn't keep) gives None.
pub fn translate_filter(filter: &str) -> Option<String> {
    let words = tokens(filter)?;
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let w = words[i].to_ascii_lowercase();
        if w == "and" {
            i += 1;
            continue;
        }
        if w == "or" {
            out.push("|".into());
            i += 1;
            continue;
        }
        let (term, used) = condition(&words[i..])?;
        out.push(term);
        i += used;
    }
    (!out.is_empty() && out.last().map(String::as_str) != Some("|")).then(|| out.join(" "))
}

/// Splits a filter into words, keeping quoted text together (unquoted).
fn tokens(filter: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut chars = filter.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '"' || c == '\'' {
            chars.next();
            let mut s = String::new();
            loop {
                match chars.next() {
                    Some(q) if q == c => break,
                    Some(x) => s.push(x),
                    None => return None,
                }
            }
            out.push(format!("\"{s}\""));
        } else if c == '(' || c == ')' {
            return None;
        } else if "<>=!".contains(c) {
            let mut s = String::new();
            while let Some(&o) = chars.peek().filter(|o| "<>=!".contains(**o)) {
                s.push(o);
                chars.next();
            }
            out.push(s);
        } else {
            let mut s = String::new();
            while let Some(&x) = chars
                .peek()
                .filter(|x| !x.is_whitespace() && !"<>=!\"'()".contains(**x))
            {
                s.push(x);
                chars.next();
            }
            out.push(s);
        }
    }
    Some(out)
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Text,
    Number,
    DaysAgo,
}

/// The field a filter names at the start of `words`, as openDJ calls it,
/// and how many words it took. Fields can be several words ("play count").
fn field(words: &[String]) -> Option<(&'static str, Kind, usize)> {
    let lower: Vec<String> = words
        .iter()
        .take(3)
        .map(|w| w.to_ascii_lowercase())
        .collect();
    let two = lower.get(..2).map(|w| w.join(" "));
    let three = lower.get(..3).map(|w| w.join(" "));
    let fields: [(&str, &str, Kind); 22] = [
        ("days since added", "added", Kind::DaysAgo),
        ("days since played", "lastplayed", Kind::DaysAgo),
        ("days since firstseen", "added", Kind::DaysAgo),
        ("play count", "played", Kind::Number),
        ("user 1", "tag", Kind::Text),
        ("user 2", "tag", Kind::Text),
        ("user1", "tag", Kind::Text),
        ("user2", "tag", Kind::Text),
        ("nbplay", "played", Kind::Number),
        ("playcount", "played", Kind::Number),
        ("genre", "genre", Kind::Text),
        ("artist", "artist", Kind::Text),
        ("author", "artist", Kind::Text),
        ("title", "title", Kind::Text),
        ("album", "album", Kind::Text),
        ("comment", "comment", Kind::Text),
        ("grouping", "grouping", Kind::Text),
        ("label", "publisher", Kind::Text),
        ("composer", "composer", Kind::Text),
        ("bpm", "bpm", Kind::Number),
        ("year", "year", Kind::Number),
        ("stars", "rating", Kind::Number),
    ];
    for (name, ours, kind) in fields {
        let n = name.split(' ').count();
        let said = match n {
            1 => lower.first().cloned(),
            2 => two.clone(),
            _ => three.clone(),
        };
        if said.as_deref() == Some(name) {
            return Some((ours, kind, n));
        }
    }
    match lower.first().map(String::as_str) {
        Some("rating") => Some(("rating", Kind::Number, 1)),
        Some("key") => Some(("key", Kind::Text, 1)),
        _ => None,
    }
}

/// One `field op value` condition at the start of `words`, as an openDJ
/// search term, and how many words it took.
fn condition(words: &[String]) -> Option<(String, usize)> {
    let (name, kind, n) = field(words)?;
    let rest = &words[n..];
    let op_words: Vec<String> = rest
        .iter()
        .take(3)
        .map(|w| w.to_ascii_lowercase())
        .collect();
    let (op, m) = match op_words.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["does", "not", "contain", ..] => ("!~", 3),
        ["doesnotcontain", ..] => ("!~", 1),
        ["is", "not", ..] => ("!=", 2),
        ["isnot", ..] | ["!=", ..] => ("!=", 1),
        ["contains", ..] | ["contain", ..] => ("~", 1),
        ["is", ..] | ["=", ..] | ["==", ..] => ("=", 1),
        [">", ..] => (">", 1),
        ["<", ..] => ("<", 1),
        [">=", ..] => (">=", 1),
        ["<=", ..] => ("<=", 1),
        _ => return None,
    };
    let value = rest.get(m)?;
    let used = n + m + 1;
    let bare = value.trim_matches('"');
    let quoted = if bare.contains(' ') {
        format!("\"{bare}\"")
    } else {
        bare.to_string()
    };
    let term = match (kind, op) {
        (Kind::Text, "=") if name == "key" => format!("key:{quoted}"),
        (Kind::Text, "=") => format!("{name}:={quoted}"),
        (Kind::Text, "!=") => format!("-{name}:={quoted}"),
        (Kind::Text, "~") => format!("{name}:{quoted}"),
        (Kind::Text, "!~") => format!("-{name}:{quoted}"),
        (Kind::Text, _) => return None,
        (_, "~" | "!~") => return None,
        (Kind::Number | Kind::DaysAgo, _) => {
            let number: f64 = bare.parse().ok()?;
            let shown = if kind == Kind::DaysAgo {
                format!("{number}d")
            } else {
                format!("{number}")
            };
            match op {
                "=" => format!("{name}:{shown}"),
                "!=" => format!("-{name}:{shown}"),
                _ => format!("{name}:{op}{shown}"),
            }
        }
    };
    Some((term, used))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_vdj_m3u_with_play_times() {
        let text = "\u{feff}#EXTVDJ:<filesize>1</filesize><artist>A</artist><title>T</title>\n\
                    /music/a.m4a\n\
                    #EXTVDJ:<time>18:22</time><lastplaytime>1699928555</lastplaytime><artist>B</artist>\n\
                    netsearch://td172486431\n";
        let entries = m3u(text);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, PathBuf::from("/music/a.m4a"));
        assert_eq!(entries[0].played_at, None);
        assert_eq!(entries[1].played_at, Some(1_699_928_555));
    }

    #[test]
    fn reads_virtual_and_filter_folders() {
        let filter = r#"<?xml version="1.0" encoding="UTF-8"?>
<FilterFolder filter="group by User 2" scope="database" />"#;
        assert_eq!(
            vdj_folder(filter),
            Some(Folder::Filter("group by User 2".into()))
        );
        let songs = r#"<?xml version="1.0" encoding="UTF-8"?>
<VirtualFolder noDuplicates="yes">
 <song path="/m/b.mp3" size="1" artist="B" title="B" idx="1" />
 <song path="/m/a.mp3" size="1" artist="A" title="A" idx="0" />
</VirtualFolder>"#;
        let Some(Folder::Songs(entries)) = vdj_folder(songs) else {
            panic!("songs");
        };
        let paths: Vec<_> = entries.iter().map(|e| e.path.to_str().unwrap()).collect();
        assert_eq!(paths, ["/m/a.mp3", "/m/b.mp3"]);
    }

    #[test]
    fn translates_filters_it_can() {
        let t = |f: &str| translate_filter(f);
        assert_eq!(
            t("genre is \"Deep House\""),
            Some("genre:=\"Deep House\"".into())
        );
        assert_eq!(
            t("bpm >= 120 and bpm < 125"),
            Some("bpm:>=120 bpm:<125".into())
        );
        assert_eq!(t("days since added < 30"), Some("added:<30d".into()));
        assert_eq!(t("play count > 0"), Some("played:>0".into()));
        assert_eq!(
            t("stars >= 4 or year < 1990"),
            Some("rating:>=4 | year:<1990".into())
        );
        assert_eq!(t("user 2 contains #synth"), Some("tag:#synth".into()));
        assert_eq!(t("genre isnot techno"), Some("-genre:=techno".into()));
        assert_eq!(t("key is Am"), Some("key:Am".into()));
        assert_eq!(t("group by User 2"), None);
        assert_eq!(t("top 100 play count"), None);
        assert_eq!(t("(genre is house) or (genre is techno)"), None);
        assert_eq!(t("isvideo"), None);
    }

    #[test]
    fn reads_a_vdj_folder_tree() {
        let home = std::env::temp_dir().join(format!("opendj-vdj-lists-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let pl = home.join("Playlists");
        std::fs::create_dir_all(pl.join("GY Pro/GY Sets")).unwrap();
        std::fs::write(pl.join("order"), "B set\nGY Pro\nA set\n").unwrap();
        std::fs::write(pl.join("A set.m3u"), "/m/a.mp3\n").unwrap();
        std::fs::write(pl.join("B set.m3u"), "/m/b.mp3\n").unwrap();
        std::fs::write(pl.join("GY Pro/GY Sets/Deep.m3u"), "/m/c.mp3\n").unwrap();
        let tags = home.join("Folders/LocalMusic/Tags.subfolders");
        std::fs::create_dir_all(&tags).unwrap();
        std::fs::write(
            home.join("Folders/LocalMusic/Tags.vdjfolder"),
            r#"<FilterFolder filter="group by User 2" />"#,
        )
        .unwrap();
        std::fs::write(
            tags.join("Old.vdjfolder"),
            r#"<FilterFolder filter="year &lt; 2000" />"#,
        )
        .unwrap();
        std::fs::write(
            tags.join("Picks.vdjfolder"),
            r#"<VirtualFolder><song path="/m/a.mp3" idx="0" /></VirtualFolder>"#,
        )
        .unwrap();
        std::fs::create_dir_all(home.join("History/2023")).unwrap();
        std::fs::write(
            home.join("History/2023/2023-11-13.m3u"),
            "#EXTVDJ:<lastplaytime>1699928555</lastplaytime>\n/m/a.mp3\n",
        )
        .unwrap();

        let mut skipped = Skipped::default();
        let all = read(&home, &[PLAYLISTS, FOLDERS, FILTERS, HISTORY], &mut skipped);
        let names: Vec<String> = all
            .iter()
            .map(|l| format!("{}/{}", l.folders.join("/"), l.name))
            .collect();
        assert_eq!(
            names,
            [
                "Playlists/B set",
                "Playlists/GY Pro/GY Sets/Deep",
                "Playlists/A set",
                "Folders/LocalMusic/Tags/Old",
                "Folders/LocalMusic/Tags/Picks",
                "/2023-11-13",
            ]
        );
        assert_eq!(skipped.filters, ["Tags: group by User 2"]);
        assert_eq!(
            all[3].kind,
            ImportedListKind::Smart {
                query: "year:<2000".into()
            }
        );
        assert_eq!(
            all[5].kind,
            ImportedListKind::History {
                started: 1_699_928_555
            }
        );

        // Only the kinds the DJ chose.
        let some = read(&home, &[HISTORY], &mut Skipped::default());
        assert_eq!(some.len(), 1);
        let _ = std::fs::remove_dir_all(&home);
    }
}
