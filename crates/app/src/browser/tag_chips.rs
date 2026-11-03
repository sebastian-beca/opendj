//! The DJ's tag fields as chips in the Info tab: each word of a field is a
//! tag, shown on all the tracks shown or on some ("#dreamy 5/12"). Tags
//! are added to and taken off every track at once, as one change Undo takes
//! back, and the field suggests tags already in the collection.

use std::collections::{BTreeMap, HashMap};

use opendj_library::cleanup::Field;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::Browser;
use crate::{LibraryEdit, MenuTag, TagChip, TrackInfo};
use opendj_engine::Track;

/// How many tags a field suggests while typing.
const SUGGESTIONS: usize = 6;
/// How many tags the track menu's Add Tag offers, and remembers as added
/// lately.
const MENU_TAGS: usize = 12;
const RECENT: usize = 8;

/// The tags in a tag field: its words.
pub(super) fn tags_in(words: &str) -> impl Iterator<Item = &str> {
    words.split_whitespace()
}

/// Two tags are the same tag whatever their case.
fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// `words` with `tag` added at the end, unless it is there already.
pub(super) fn with_tag(words: &str, tag: &str) -> String {
    let mut out: Vec<&str> = tags_in(words).collect();
    if !out.iter().any(|t| same(t, tag)) {
        out.push(tag);
    }
    out.join(" ")
}

/// `words` without `tag`.
pub(super) fn without_tag(words: &str, tag: &str) -> String {
    tags_in(words)
        .filter(|t| !same(t, tag))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The tags of field `n` on `tracks`, each with how many tracks have it:
/// in the order of the first track's field, then the others' by how many
/// have them.
pub(super) fn chips_of(tracks: &[&Track], n: usize) -> Vec<(String, usize)> {
    let mut order: Vec<String> = Vec::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for t in tracks {
        // A tag twice in one field counts once.
        let mut seen: Vec<String> = Vec::new();
        for tag in tags_in(t.info.user_tag(n)) {
            let key = tag.to_lowercase();
            if seen.contains(&key) {
                continue;
            }
            seen.push(key.clone());
            if !counts.contains_key(&key) {
                order.push(tag.to_string());
            }
            *counts.entry(key).or_default() += 1;
        }
    }
    let count = |t: &String| counts[&t.to_lowercase()];
    let first = tracks
        .first()
        .map_or(0, |t| tags_in(t.info.user_tag(n)).count());
    let (mine, rest) = order.split_at(first.min(order.len()));
    let mut rest = rest.to_vec();
    rest.sort_by(|a, b| {
        count(b)
            .cmp(&count(a))
            .then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
    });
    mine.iter()
        .chain(rest.iter())
        .map(|t| (t.clone(), count(t)))
        .collect()
}

/// Tags of field `n` in the collection that start with `typed` (the `#`
/// ignored), most used first, leaving out `taken`.
pub(super) fn suggest(library: &[&Track], n: usize, typed: &str, taken: &[String]) -> Vec<String> {
    let bare = |s: &str| s.trim_start_matches('#').to_lowercase();
    let typed = bare(typed.trim());
    if typed.is_empty() {
        return Vec::new();
    }
    let mut counts: BTreeMap<String, (String, usize)> = BTreeMap::new();
    for t in library {
        for tag in tags_in(t.info.user_tag(n)) {
            if bare(tag).starts_with(&typed) && !taken.iter().any(|k| same(k, tag)) {
                counts
                    .entry(tag.to_lowercase())
                    .or_insert_with(|| (tag.to_string(), 0))
                    .1 += 1;
            }
        }
    }
    let mut found: Vec<(String, usize)> = counts.into_values().collect();
    found.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    found
        .into_iter()
        .take(SUGGESTIONS)
        .map(|(t, _)| t)
        .collect()
}

/// The tags the track menu offers: `recent` ones (field, tag) first, then
/// the most used in every tag field of `library`; those containing `find`
/// (the `#` ignored) when it isn't empty.
pub(super) fn menu_tags(
    library: &[&Track],
    fields: usize,
    recent: &[(usize, String)],
    find: &str,
) -> Vec<(usize, String)> {
    let bare = |s: &str| s.trim_start_matches('#').to_lowercase();
    let find = bare(find.trim());
    let mut counts: BTreeMap<(usize, String), (String, usize)> = BTreeMap::new();
    for t in library {
        for n in 0..fields {
            for tag in tags_in(t.info.user_tag(n)) {
                counts
                    .entry((n, tag.to_lowercase()))
                    .or_insert_with(|| (tag.to_string(), 0))
                    .1 += 1;
            }
        }
    }
    let mut used: Vec<(usize, String, usize)> = counts
        .into_iter()
        .map(|((n, _), (tag, c))| (n, tag, c))
        .collect();
    used.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.1.cmp(&b.1)));
    let mut out: Vec<(usize, String)> = Vec::new();
    let candidates = recent
        .iter()
        .filter(|(n, _)| *n < fields)
        .cloned()
        .chain(used.into_iter().map(|(n, t, _)| (n, t)));
    for (n, tag) in candidates {
        if !find.is_empty() && !bare(&tag).contains(&find) {
            continue;
        }
        if !out.iter().any(|(m, t)| *m == n && same(t, &tag)) {
            out.push((n, tag));
        }
        if out.len() == MENU_TAGS {
            break;
        }
    }
    out
}

/// The chips the Info tab draws for field `n`.
pub(super) fn chip_model(tracks: &[&Track], n: usize) -> ModelRc<TagChip> {
    let chips: Vec<TagChip> = chips_of(tracks, n)
        .into_iter()
        .map(|(tag, count)| TagChip {
            tag: tag.into(),
            count: count as i32,
        })
        .collect();
    ModelRc::new(VecModel::from(chips))
}

impl Browser {
    pub(super) fn install_tag_chips(
        ui: &TrackInfo,
        with: impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static,
    ) {
        let w = with.clone();
        ui.on_tag_add(move |id, typed| w(&|b| b.change_tag(&id, &typed, true)));
        let w = with.clone();
        ui.on_tag_remove(move |id, tag| w(&|b| b.change_tag(&id, &tag, false)));
        let w = with;
        ui.on_tag_suggest(move |id, typed| w(&|b| b.suggest_tags(&id, &typed)));
    }

    /// The tracks the Info tab shows: the picked ones, or the one.
    fn info_tracks(&self) -> Vec<(usize, std::sync::Arc<Track>)> {
        if !self.info.many.is_empty() {
            self.info.many.clone()
        } else {
            self.info.shown.clone().into_iter().collect()
        }
    }

    /// Adds the tags typed (`add`), or takes one off, in tag field `id` of
    /// every track shown.
    fn change_tag(&mut self, id: &str, typed: &str, add: bool) {
        let Some(Field::Tag(n)) = Field::from_name(id) else {
            return;
        };
        let tags: Vec<&str> = if add {
            tags_in(typed).collect()
        } else {
            vec![typed.trim()]
        };
        if tags.is_empty() {
            return;
        }
        let shown = self.info_tracks();
        if let [(_, t)] = shown.as_slice() {
            // One track: saved as any field typed in the tab.
            let mut words = t.info.user_tag(n as usize).to_string();
            for tag in &tags {
                words = if add {
                    with_tag(&words, tag)
                } else {
                    without_tag(&words, tag)
                };
            }
            self.set_suggestions(Vec::new());
            if words != t.info.user_tag(n as usize) {
                self.info_edit(id, &words);
            }
            return;
        }
        self.set_suggestions(Vec::new());
        let indices: Vec<usize> = shown.iter().map(|(i, _)| *i).collect();
        if let Some(status) = self.tag_tracks(&indices, n as usize, &tags, add) {
            if let Some(app) = self.app.upgrade() {
                let ui = app.global::<TrackInfo>();
                ui.set_status(status.into());
                ui.set_status_ok(true);
            }
        }
    }

    /// Adds `tags` to tag field `n` of the library tracks `indices`, or
    /// takes them off, as one change Undo takes back. Says what it did,
    /// or `None` when nothing changed.
    pub(super) fn tag_tracks(
        &mut self,
        indices: &[usize],
        n: usize,
        tags: &[&str],
        add: bool,
    ) -> Option<String> {
        let mut changes = Vec::new();
        for &index in indices {
            let Some(t) = self.library.get(index) else {
                continue;
            };
            let before = t.info.user_tag(n);
            let mut words = before.to_string();
            for tag in tags {
                words = if add {
                    with_tag(&words, tag)
                } else {
                    without_tag(&words, tag)
                };
            }
            if words != before {
                changes.push((index, Field::Tag(n as u8), words));
            }
        }
        if add {
            for tag in tags {
                self.info
                    .recent_tags
                    .retain(|(m, t)| !(*m == n && same(t, tag)));
                self.info.recent_tags.insert(0, (n, tag.to_string()));
            }
            self.info.recent_tags.truncate(RECENT);
        }
        if changes.is_empty() {
            return None;
        }
        let status = self.make_changes(changes, if add { "add tag" } else { "remove tag" });
        self.refresh_tree();
        self.refresh_rows();
        self.refresh_side();
        Some(status)
    }

    /// Fills the track menu's Add Tag panel: tags added lately, then the
    /// collection's most used, those matching `find` when it isn't empty.
    pub(super) fn find_menu_tags(&mut self, find: &str) {
        let names = super::query::field_names();
        let library: Vec<&Track> = self.library.iter().map(|t| &**t).collect();
        let picks = menu_tags(&library, names.tags.len(), &self.info.recent_tags, find);
        let rows: Vec<MenuTag> = picks
            .into_iter()
            .map(|(n, tag)| MenuTag {
                tag: tag.into(),
                id: format!("tag{}", n + 1).into(),
                field: names.tags.get(n).cloned().unwrap_or_default().into(),
            })
            .collect();
        if let Some(app) = self.app.upgrade() {
            app.global::<LibraryEdit>()
                .set_tag_picks(ModelRc::new(VecModel::from(rows)));
        }
    }

    /// The track menu's Add Tag: tag `tag` in field `id` goes on the rows
    /// `first` to `last` (or all the picked rows).
    pub(super) fn add_tag_to_rows(&mut self, first: usize, last: usize, id: &str, tag: &str) {
        let Some(Field::Tag(n)) = Field::from_name(id) else {
            return;
        };
        let tags: Vec<&str> = tags_in(tag).collect();
        if tags.is_empty() {
            return;
        }
        let indices = self.chosen_tracks(first, last);
        self.adopt(&indices);
        self.tag_tracks(&indices, n as usize, &tags, true);
    }

    /// Offers the collection's tags that start with what is typed in tag
    /// field `id`.
    fn suggest_tags(&mut self, id: &str, typed: &str) {
        let Some(Field::Tag(n)) = Field::from_name(id) else {
            return;
        };
        let shown = self.info_tracks();
        let tracks: Vec<&Track> = shown.iter().map(|(_, t)| &**t).collect();
        // Tags every track has already aren't offered again.
        let taken: Vec<String> = chips_of(&tracks, n as usize)
            .into_iter()
            .filter(|(_, c)| *c == tracks.len())
            .map(|(t, _)| t)
            .collect();
        let library: Vec<&Track> = self.library.iter().map(|t| &**t).collect();
        let found = suggest(&library, n as usize, typed, &taken);
        if let Some(app) = self.app.upgrade() {
            app.global::<TrackInfo>().set_suggest_for(id.into());
        }
        self.set_suggestions(found);
    }

    fn set_suggestions(&self, found: Vec<String>) {
        if let Some(app) = self.app.upgrade() {
            let found: Vec<SharedString> = found.into_iter().map(Into::into).collect();
            app.global::<TrackInfo>()
                .set_suggestions(ModelRc::new(VecModel::from(found)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendj_engine::Source;

    fn track(tags: &str) -> Track {
        let mut t = Track {
            id: 1,
            artist: String::new(),
            title: String::new(),
            album: String::new(),
            genre: String::new(),
            rating: 0,
            bpm: 0.0,
            key: String::new(),
            duration: 0.0,
            first_beat: 0.0,
            source: Source::Synth { seed: 1 },
            waveform: Default::default(),
            audio: None,
            cover: None,
            grid: Default::default(),
            info: Default::default(),
        };
        t.info.set_user_tag(0, tags.into());
        t
    }

    #[test]
    fn adds_and_takes_off_tags() {
        assert_eq!(with_tag("#dark #synth", "#sunset"), "#dark #synth #sunset");
        assert_eq!(
            with_tag("#dark", "#DARK"),
            "#dark",
            "once, whatever the case"
        );
        assert_eq!(with_tag("", "#a"), "#a");
        assert_eq!(without_tag("#dark  #synth", "#Dark"), "#synth");
        assert_eq!(without_tag("#synth", "#vox"), "#synth");
    }

    #[test]
    fn counts_tags_on_several_tracks() {
        let a = track("#uplifting #dreamy");
        let b = track("#uplifting #synth #synth");
        let c = track("#Uplifting #synth");
        let chips = chips_of(&[&a, &b, &c], 0);
        assert_eq!(
            chips,
            [
                ("#uplifting".to_string(), 3),
                ("#dreamy".to_string(), 1),
                ("#synth".to_string(), 2),
            ],
            "the first track's order, then the rest by count"
        );
        assert!(chips_of(&[&a], 1).is_empty());
    }

    #[test]
    fn suggests_tags_in_use() {
        let a = track("#sunset #synth");
        let b = track("#sunrise #synth");
        let c = track("#synth #sunset");
        let lib = [&a, &b, &c];
        assert_eq!(suggest(&lib, 0, "su", &[]), ["#sunset", "#sunrise"]);
        assert_eq!(suggest(&lib, 0, "#sy", &[]), ["#synth"]);
        assert_eq!(
            suggest(&lib, 0, "s", &["#synth".to_string()]),
            ["#sunset", "#sunrise"],
            "tags already on every track aren't offered"
        );
        assert!(suggest(&lib, 0, " ", &[]).is_empty());
    }

    #[test]
    fn offers_recent_then_most_used_tags() {
        let a = track("#sunset #synth");
        let b = track("#synth");
        let mut c = track("#dark");
        c.info.set_user_tag(1, "#vox".into());
        let lib = [&a, &b, &c];
        let recent = [(0, "#dark".to_string())];
        assert_eq!(
            menu_tags(&lib, 2, &recent, ""),
            [
                (0, "#dark".to_string()),
                (0, "#synth".to_string()),
                (0, "#sunset".to_string()),
                (1, "#vox".to_string()),
            ]
        );
        assert_eq!(
            menu_tags(&lib, 2, &recent, "#S"),
            [(0, "#synth".to_string()), (0, "#sunset".to_string())]
        );
        assert_eq!(menu_tags(&lib, 1, &[], "vox"), []);
    }
}
