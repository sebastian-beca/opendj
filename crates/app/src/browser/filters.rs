//! The filter bar above the track list: a chip per field, as djay Pro
//! has, that narrows the list along with the search. Each chip turns into
//! search text (see `mixxx_library::query`), so chips, typed searches and
//! smart lists all agree.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use mixxx_engine::key::Key;
use mixxx_engine::Track;
use mixxx_library::query::{self, and_text, keyword, quoted, FieldNames, COLOR_NAMES};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::Browser;
use crate::{settings, AppWindow, FilterChip, FilterChoice, FilterUi};

/// How close a tempo must be to the playing track's, when BPM follows it.
pub const TOLERANCES: [f64; 5] = [0.02, 0.04, 0.06, 0.08, 0.10];

/// What a field's chip edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Pick values of a text field (genre, artist, ...).
    Text,
    /// Pick words of a tag field, from 0; all of them or any.
    Tags(usize),
    /// A tempo range, or near the playing track's.
    Bpm,
    /// Keys, or those that mix with the playing track's.
    Key,
    Rating,
    Color,
    Year,
    Played,
    Added,
}

/// A field the bar can filter on.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    /// Names the field in settings: "genre", "tag1", "bpm".
    pub id: String,
    pub label: String,
    pub kind: Kind,
}

/// The fields offered, in the order "+" lists them; grouping and the tag
/// fields go by the names the DJ gave them.
pub fn fields(names: &FieldNames) -> Vec<Field> {
    let f = |id: &str, label: &str, kind| Field {
        id: id.into(),
        label: label.into(),
        kind,
    };
    let mut out = vec![
        f("grouping", &names.grouping, Kind::Text),
        f("genre", "Genre", Kind::Text),
    ];
    for (i, name) in names.tags.iter().enumerate() {
        out.push(f(&format!("tag{}", i + 1), name, Kind::Tags(i)));
    }
    out.extend([
        f("bpm", "BPM", Kind::Bpm),
        f("key", "Key", Kind::Key),
        f("rating", "Rating", Kind::Rating),
        f("color", "Colour", Kind::Color),
        f("artist", "Artist", Kind::Text),
        f("album", "Album", Kind::Text),
        f("year", "Year", Kind::Year),
        f("played", "Play Count", Kind::Played),
        f("added", "Date Added", Kind::Added),
    ]);
    out
}

/// The chips on the bar when the DJ hasn't chosen.
pub const DEFAULT_PINNED: [&str; 6] = ["grouping", "genre", "tag1", "tag2", "bpm", "key"];

/// One value a chip offers: what it says, the search it stands for, how
/// many tracks of the list have it, and a colour swatch for colours.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub label: String,
    pub term: String,
    pub count: usize,
    pub swatch: Option<u32>,
}

/// What the bar is set to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filters {
    /// Fields always on the bar, in order.
    pub pinned: Vec<String>,
    /// Fields added with "+" for now, after the pinned ones.
    pub added: Vec<String>,
    /// The values picked per field, as (search, label).
    pub picked: HashMap<String, Vec<(String, String)>>,
    /// Tag fields whose picked words must all be on a track, not any.
    pub match_all: HashSet<String>,
    /// A typed tempo range; either end may be empty.
    pub bpm_low: String,
    pub bpm_high: String,
    /// BPM follows the playing track, within `TOLERANCES[tolerance]`.
    pub bpm_follow: bool,
    pub tolerance: usize,
    /// Only keys that mix with the playing track's.
    pub key_follow: bool,
}

impl Filters {
    pub fn new(pinned: Vec<String>, tolerance: usize) -> Self {
        Filters {
            pinned,
            tolerance: tolerance.min(TOLERANCES.len() - 1),
            ..Default::default()
        }
    }

    /// Whether a field narrows the list now.
    pub fn active(&self, field: &Field) -> bool {
        match field.kind {
            Kind::Bpm => {
                self.bpm_follow
                    || !self.bpm_low.trim().is_empty()
                    || !self.bpm_high.trim().is_empty()
            }
            Kind::Key if self.key_follow => true,
            _ => self.picked.get(&field.id).is_some_and(|p| !p.is_empty()),
        }
    }

    pub fn any_active(&self, fields: &[Field]) -> bool {
        fields.iter().any(|f| self.active(f))
    }

    /// The fields with a chip on the bar: pinned ones, then any other
    /// that is added or narrowing the list.
    pub fn shown<'a>(&self, fields: &'a [Field]) -> Vec<&'a Field> {
        let by_id = |id: &String| fields.iter().find(|f| f.id == *id);
        let mut out: Vec<&Field> = self.pinned.iter().filter_map(by_id).collect();
        for f in self.added.iter().filter_map(by_id).chain(fields) {
            if (self.added.contains(&f.id) || self.active(f)) && !out.contains(&f) {
                out.push(f);
            }
        }
        out
    }

    /// Clears a field's chip.
    pub fn clear(&mut self, field: &Field) {
        self.picked.remove(&field.id);
        match field.kind {
            Kind::Bpm => {
                self.bpm_follow = false;
                self.bpm_low.clear();
                self.bpm_high.clear();
            }
            Kind::Key => self.key_follow = false,
            _ => {}
        }
    }

    pub fn clear_all(&mut self, fields: &[Field]) {
        for f in fields {
            self.clear(f);
        }
        self.added.clear();
    }

    /// Picks a value of a field, or unpicks it.
    pub fn toggle(&mut self, field: &str, choice: &Choice) {
        let picked = self.picked.entry(field.to_string()).or_default();
        match picked.iter().position(|(t, _)| *t == choice.term) {
            Some(i) => {
                picked.remove(i);
            }
            None => picked.push((choice.term.clone(), choice.label.clone())),
        }
    }

    pub fn is_picked(&self, field: &str, term: &str) -> bool {
        self.picked
            .get(field)
            .is_some_and(|p| p.iter().any(|(t, _)| t == term))
    }

    /// What the chip shows: its value, or "--" when it doesn't filter.
    pub fn summary(
        &self,
        field: &Field,
        deck: Option<(f64, Option<Key>)>,
        camelot: bool,
    ) -> String {
        match field.kind {
            Kind::Bpm if self.bpm_follow => {
                let pct = (TOLERANCES[self.tolerance] * 100.0).round();
                match deck.filter(|(bpm, _)| *bpm > 0.0) {
                    Some((bpm, _)) => format!("{bpm:.0} ±{pct}%"),
                    None => format!("±{pct}%"),
                }
            }
            Kind::Bpm => match (self.bpm_low.trim(), self.bpm_high.trim()) {
                ("", "") => "--".into(),
                (low, "") => format!("{low}+"),
                ("", high) => format!("≤ {high}"),
                (low, high) => format!("{low} – {high}"),
            },
            Kind::Key if self.key_follow => match deck.and_then(|(_, k)| k) {
                Some(k) if camelot => format!("Match {}", k.camelot()),
                Some(k) => format!("Match {}", k.name()),
                None => "Match".into(),
            },
            _ => {
                let picked = self.picked.get(&field.id).map_or(&[][..], Vec::as_slice);
                match picked {
                    [] => "--".into(),
                    [(_, one)] => one.clone(),
                    [(_, first), rest @ ..] => format!("{first} +{}", rest.len()),
                }
            }
        }
    }

    /// The search the bar stands for, given the playing track's tempo and
    /// key; empty when nothing is filtered.
    pub fn text(&self, fields: &[Field], deck: Option<(f64, Option<Key>)>) -> String {
        let mut text = String::new();
        for f in fields {
            text = and_text(&text, &self.field_text(f, deck));
        }
        text
    }

    fn field_text(&self, field: &Field, deck: Option<(f64, Option<Key>)>) -> String {
        let picked = self.picked.get(&field.id).map_or(&[][..], Vec::as_slice);
        let terms = || picked.iter().map(|(t, _)| t.as_str());
        let any = || terms().collect::<Vec<_>>().join(" | ");
        match field.kind {
            Kind::Bpm if self.bpm_follow => match deck.filter(|(bpm, _)| *bpm > 0.0) {
                Some((bpm, _)) => {
                    let t = TOLERANCES[self.tolerance];
                    format!("bpm:{:.2}-{:.2}", bpm * (1.0 - t), bpm * (1.0 + t))
                }
                // Nothing playing: nothing to follow, so nothing is left out.
                None => String::new(),
            },
            Kind::Bpm => {
                let num = |s: &str| s.trim().replace(',', ".").parse::<f64>().ok();
                match (num(&self.bpm_low), num(&self.bpm_high)) {
                    (Some(a), Some(b)) => format!("bpm:{a}-{b}"),
                    (Some(a), None) => format!("bpm:>={a}"),
                    (None, Some(b)) => format!("bpm:<={b}"),
                    (None, None) => String::new(),
                }
            }
            Kind::Key => {
                let follow = match deck.and_then(|(_, k)| k) {
                    Some(k) if self.key_follow => format!("~key:{}", k.camelot()),
                    _ => String::new(),
                };
                and_text(&follow, &any())
            }
            Kind::Tags(_) if self.match_all.contains(&field.id) => {
                terms().collect::<Vec<_>>().join(" ")
            }
            _ => any(),
        }
    }

    /// Whether the bar's search depends on the playing track.
    pub fn follows_deck(&self) -> bool {
        self.bpm_follow || self.key_follow
    }
}

/// The bar's state in the browser: the fields, the chips and the popup open.
pub(super) struct FilterState {
    pub names: FieldNames,
    pub fields: Vec<Field>,
    pub filters: Filters,
    /// The field whose popup is open, every value it offers, and the text
    /// typed to find one.
    editing: Option<String>,
    choices: Vec<Choice>,
    find: String,
}

impl FilterState {
    pub fn new() -> Self {
        let names = settings::field_names();
        query::set_field_names(names.clone());
        FilterState {
            fields: fields(&names),
            names,
            filters: Filters::new(settings::filter_pinned(), settings::filter_tolerance()),
            editing: None,
            choices: Vec::new(),
            find: String::new(),
        }
    }

    pub fn clear_all(&mut self) {
        let fields = self.fields.clone();
        self.filters.clear_all(&fields);
    }

    /// The bar as a smart list's search: following the playing track is
    /// saved as `~deck` (key and tempo), not as today's tempo.
    pub fn saved_text(&self) -> String {
        let fixed = self.filters.text(&self.fields, None);
        if self.filters.follows_deck() {
            and_text(&fixed, "~deck")
        } else {
            fixed
        }
    }

    fn field(&self, id: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.id == id)
    }

    /// The values the popup lists, narrowed by what was typed to find one.
    fn shown_choices(&self) -> Vec<&Choice> {
        let find = self.find.trim().to_lowercase();
        self.choices
            .iter()
            .filter(|c| find.is_empty() || c.label.to_lowercase().contains(&find))
            .collect()
    }
}

impl Browser {
    pub(super) fn install_filters(app: &AppWindow, b: &Rc<RefCell<Self>>) {
        let ui = app.global::<FilterUi>();
        // The bar starts hidden; the filter button lights up while chips
        // narrow the list.
        ui.set_shown(false);
        let weak = Rc::downgrade(b);
        let with = move |f: &dyn Fn(&mut Browser)| {
            if let Some(b) = weak.upgrade() {
                f(&mut b.borrow_mut());
            }
        };
        {
            let app = app.as_weak();
            ui.on_set_shown(move |on| {
                if let Some(app) = app.upgrade() {
                    app.global::<FilterUi>().set_shown(on);
                }
            });
        }
        let w = with.clone();
        ui.on_add(move |id| {
            let chip = std::cell::Cell::new(-1);
            w(&|b| chip.set(b.add_filter(&id)));
            chip.get()
        });
        let w = with.clone();
        ui.on_edit(move |chip| w(&|b| b.edit_filter(chip)));
        let w = with.clone();
        ui.on_toggle(move |i| {
            w(&|b| {
                let f = &mut b.filter;
                let (Some(id), Some(c)) = (
                    f.editing.clone(),
                    f.shown_choices()
                        .get(i.max(0) as usize)
                        .map(|c| (*c).clone()),
                ) else {
                    return;
                };
                f.filters.toggle(&id, &c);
                b.filters_changed();
            })
        });
        let w = with.clone();
        ui.on_search_choices(move |text| {
            w(&|b| {
                b.filter.find = text.to_string();
                b.refresh_filter_ui();
            })
        });
        let w = with.clone();
        ui.on_set_match_all(move |on| {
            w(&|b| {
                let Some(id) = b.filter.editing.clone() else {
                    return;
                };
                let all = &mut b.filter.filters.match_all;
                if on {
                    all.insert(id);
                } else {
                    all.remove(&id);
                }
                b.filters_changed();
            })
        });
        let w = with.clone();
        ui.on_set_follow(move |on| {
            w(&|b| {
                let f = &mut b.filter;
                match f
                    .editing
                    .as_deref()
                    .and_then(|id| f.field(id))
                    .map(|f| f.kind)
                {
                    Some(Kind::Bpm) => f.filters.bpm_follow = on,
                    Some(Kind::Key) => f.filters.key_follow = on,
                    _ => return,
                }
                b.update_reference();
                b.filters_changed();
            })
        });
        let w = with.clone();
        ui.on_set_bpm(move |low, high| {
            w(&|b| {
                b.filter.filters.bpm_low = low.to_string();
                b.filter.filters.bpm_high = high.to_string();
                b.filters_changed();
            })
        });
        let w = with.clone();
        ui.on_set_tolerance(move |i| {
            w(&|b| {
                let i = (i.max(0) as usize).min(TOLERANCES.len() - 1);
                b.filter.filters.tolerance = i;
                settings::set_filter_tolerance(i);
                b.filters_changed();
            })
        });
        let w = with.clone();
        ui.on_set_pinned(move |chip, on| {
            w(&|b| {
                let Some(id) = b.chip_field(chip) else { return };
                let f = &mut b.filter.filters;
                f.pinned.retain(|p| *p != id);
                f.added.retain(|p| *p != id);
                if on {
                    f.pinned.push(id);
                } else {
                    // Stays on the bar until the list is filtered afresh.
                    f.added.push(id);
                }
                settings::set_filter_pinned(&f.pinned);
                b.refresh_filter_ui();
            })
        });
        let w = with.clone();
        ui.on_clear(move |chip| {
            w(&|b| {
                let Some(id) = b.chip_field(chip) else { return };
                if let Some(field) = b.filter.field(&id).cloned() {
                    b.filter.filters.clear(&field);
                }
                b.filters_changed();
            })
        });
        let w = with.clone();
        ui.on_clear_all(move || {
            w(&|b| {
                b.filter.clear_all();
                b.search.clear();
                if let Some(app) = b.app.upgrade() {
                    app.global::<crate::StoreUi>()
                        .set_search_text(SharedString::new());
                }
                b.filters_changed();
            })
        });
        b.borrow_mut().refresh_filter_ui();
    }

    /// The field of the chip at `chip` on the bar.
    fn chip_field(&self, chip: i32) -> Option<String> {
        let f = &self.filter;
        f.filters
            .shown(&f.fields)
            .get(usize::try_from(chip).ok()?)
            .map(|field| field.id.clone())
    }

    /// Puts a field on the bar for now; returns its chip.
    fn add_filter(&mut self, id: &str) -> i32 {
        let f = &mut self.filter;
        if !f.filters.pinned.iter().any(|p| p == id) && !f.filters.added.iter().any(|p| p == id) {
            f.filters.added.push(id.to_string());
        }
        self.refresh_filter_ui();
        let f = &self.filter;
        f.filters
            .shown(&f.fields)
            .iter()
            .position(|field| field.id == id)
            .map_or(-1, |i| i as i32)
    }

    /// Opens a chip's popup: lists the values of its field in the list.
    fn edit_filter(&mut self, chip: i32) {
        self.filter.editing = self.chip_field(chip);
        self.filter.find.clear();
        self.update_reference();
        self.refresh_choices();
        self.refresh_filter_ui();
    }

    /// The values the open popup offers, counted over the list (before
    /// filtering, so picking one doesn't hide the rest).
    fn refresh_choices(&mut self) {
        let Some(field) = self
            .filter
            .editing
            .as_deref()
            .and_then(|id| self.filter.field(id))
        else {
            self.filter.choices.clear();
            return;
        };
        let field = field.clone();
        let tracks: Vec<usize> = self.row_data().iter().filter_map(|r| r.local).collect();
        self.filter.choices = choices(
            &field,
            tracks.iter().map(|&i| &*self.library[i]),
            &self.filter.filters,
            self.camelot,
        );
    }

    /// The search and the bar together, as one search.
    pub(super) fn full_search(&self) -> String {
        let f = &self.filter;
        and_text(&self.search, &f.filters.text(&f.fields, self.deck_ref))
    }

    pub(super) fn filtering(&self) -> bool {
        self.filter.filters.any_active(&self.filter.fields)
    }

    /// A chip changed: list again and show it.
    fn filters_changed(&mut self) {
        self.refresh_rows();
        self.refresh_filter_ui();
    }

    /// The names of grouping or the tag fields changed in Settings.
    pub(super) fn field_names_changed(&mut self, names: FieldNames) {
        let f = &mut self.filter;
        f.fields = fields(&names);
        self.column_names_changed(&names);
        let f = &mut self.filter;
        f.names = names;
        // Fields that are gone can't filter.
        let ids: HashSet<String> = f.fields.iter().map(|x| x.id.clone()).collect();
        f.filters.picked.retain(|id, _| ids.contains(id));
        if f.editing.as_ref().is_some_and(|id| !ids.contains(id)) {
            f.editing = None;
        }
        self.refresh_rows();
        self.refresh_filter_ui();
        if let Some(app) = self.app.upgrade() {
            self.refresh_info(&app);
        }
    }

    /// Shows the chips, "+"'s list and the open popup.
    pub(super) fn refresh_filter_ui(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<FilterUi>();
        let f = &self.filter;
        let shown = f.filters.shown(&f.fields);
        let chips: Vec<FilterChip> = shown
            .iter()
            .map(|field| FilterChip {
                field: field.id.clone().into(),
                label: field.label.clone().into(),
                value: f.filters.summary(field, self.deck_ref, self.camelot).into(),
                active: f.filters.active(field),
                pinned: f.filters.pinned.contains(&field.id),
                kind: match field.kind {
                    Kind::Tags(_) => 1,
                    Kind::Bpm => 2,
                    Kind::Key => 3,
                    _ => 0,
                },
            })
            .collect();
        let addable: Vec<&Field> = f.fields.iter().filter(|x| !shown.contains(x)).collect();
        let strings = |v: Vec<String>| {
            ModelRc::new(VecModel::from(
                v.into_iter().map(SharedString::from).collect::<Vec<_>>(),
            ))
        };
        let mut sorted = addable.clone();
        sorted.sort_by_key(|x| x.label.to_lowercase());
        ui.set_addable(strings(sorted.iter().map(|x| x.label.clone()).collect()));
        ui.set_addable_ids(strings(sorted.iter().map(|x| x.id.clone()).collect()));
        ui.set_editing(
            f.editing
                .as_ref()
                .and_then(|id| shown.iter().position(|x| x.id == *id))
                .map_or(-1, |i| i as i32),
        );
        ui.set_chips(ModelRc::new(VecModel::from(chips)));
        ui.set_any_active(f.filters.any_active(&f.fields));
        let editing = f.editing.as_deref().and_then(|id| f.field(id));
        let choices: Vec<FilterChoice> = f
            .shown_choices()
            .into_iter()
            .map(|c| FilterChoice {
                label: c.label.clone().into(),
                count: c.count as i32,
                on: editing.is_some_and(|e| f.filters.is_picked(&e.id, &c.term)),
                swatch: c.swatch.map_or_else(Default::default, |rgb| {
                    slint::Color::from_rgb_u8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
                }),
                has_swatch: c.swatch.is_some(),
            })
            .collect();
        ui.set_choices(ModelRc::new(VecModel::from(choices)));
        ui.set_choice_search(f.find.clone().into());
        ui.set_match_all(editing.is_some_and(|e| f.filters.match_all.contains(&e.id)));
        ui.set_follow(match editing.map(|e| e.kind) {
            Some(Kind::Bpm) => f.filters.bpm_follow,
            Some(Kind::Key) => f.filters.key_follow,
            _ => false,
        });
        ui.set_follow_note(
            match (editing.map(|e| e.kind), self.deck_ref) {
                (_, None) => "Nothing is playing, so every track is in.".to_string(),
                (Some(Kind::Bpm), Some((bpm, _))) if bpm > 0.0 => {
                    let t = TOLERANCES[f.filters.tolerance];
                    format!(
                        "Playing {bpm:.1} BPM: tracks from {:.1} to {:.1}.",
                        bpm * (1.0 - t),
                        bpm * (1.0 + t)
                    )
                }
                (Some(Kind::Key), Some((_, Some(k)))) => format!(
                    "Playing in {}: keys that mix with it.",
                    if self.camelot { k.camelot() } else { k.name() }
                ),
                _ => "The playing track has no tempo or key yet.".to_string(),
            }
            .into(),
        );
        ui.set_bpm_low(f.filters.bpm_low.clone().into());
        ui.set_bpm_high(f.filters.bpm_high.clone().into());
        ui.set_tolerance(f.filters.tolerance as i32);
    }
}

/// The values `field` can be set to, counted over `tracks`, with those
/// already picked kept even when the list doesn't have them.
pub fn choices<'a>(
    field: &Field,
    tracks: impl Iterator<Item = &'a Track>,
    filters: &Filters,
    camelot: bool,
) -> Vec<Choice> {
    let choice = |label: String, term: String| Choice {
        label,
        term,
        count: 0,
        swatch: None,
    };
    let tracks: Vec<&Track> = tracks.collect();
    let count = |out: &mut Vec<Choice>, test: &dyn Fn(&Track, &Choice) -> bool| {
        for c in out.iter_mut() {
            c.count = tracks.iter().filter(|t| test(t, c)).count();
        }
    };
    let mut out: Vec<Choice> = match field.kind {
        Kind::Text | Kind::Tags(_) | Kind::Year => {
            // Distinct values, counted, in the case first seen.
            let mut seen: HashMap<String, (String, usize)> = HashMap::new();
            let mut none = 0;
            for t in &tracks {
                let values = values_of(field, t);
                if values.is_empty() {
                    none += 1;
                }
                for v in values {
                    seen.entry(v.to_lowercase()).or_insert((v, 0)).1 += 1;
                }
            }
            let name = keyword(&field.id);
            let mut list: Vec<Choice> = seen
                .into_values()
                .map(|(v, n)| Choice {
                    count: n,
                    ..match field.kind {
                        Kind::Year => choice(v.clone(), format!("year:{v}")),
                        _ => choice(v.clone(), format!("{name}:={}", quoted(&v))),
                    }
                })
                .collect();
            if field.kind == Kind::Year {
                list.sort_by(|a, b| b.label.cmp(&a.label));
            } else {
                list.sort_by_key(|c| natural(&c.label));
            }
            if none > 0 && field.kind != Kind::Year {
                list.push(Choice {
                    count: none,
                    ..choice("(none)".into(), format!("{name}:\"\""))
                });
            }
            list
        }
        Kind::Key => {
            let mut keys: Vec<Key> = tracks.iter().filter_map(|t| Key::parse(&t.key)).collect();
            keys.sort_by_key(|k| (camelot_order(*k), k.camelot()));
            keys.dedup();
            let mut list: Vec<Choice> = keys
                .into_iter()
                .map(|k| {
                    let label = if camelot { k.camelot() } else { k.name() };
                    choice(label, format!("key:{}", k.camelot()))
                })
                .collect();
            count(&mut list, &|t, c| {
                Key::parse(&t.key).map(|k| format!("key:{}", k.camelot())) == Some(c.term.clone())
            });
            list
        }
        Kind::Rating => {
            let mut list: Vec<Choice> = (0..=5u8)
                .rev()
                .map(|n| match n {
                    0 => choice("No rating".into(), "rating:0".into()),
                    n => choice("★".repeat(n as usize), format!("rating:{n}")),
                })
                .collect();
            count(&mut list, &|t, c| c.term == format!("rating:{}", t.rating));
            list
        }
        Kind::Color => {
            let mut list: Vec<Choice> = COLOR_NAMES
                .iter()
                .map(|&(name, rgb)| Choice {
                    swatch: Some(rgb),
                    ..choice(capitalized(name), format!("color:{name}"))
                })
                .chain([choice("No colour".into(), "color:\"\"".into())])
                .collect();
            count(&mut list, &|t, c| t.info.color == c.swatch);
            list
        }
        Kind::Played => {
            let mut list = vec![
                choice("Never played".into(), "played:0".into()),
                choice("Played".into(), "played:>0".into()),
            ];
            count(&mut list, &|t, c| {
                (t.info.play_count > 0) == (c.term == "played:>0")
            });
            list
        }
        Kind::Added => [7, 30, 90, 365]
            .into_iter()
            .map(|d| {
                let label = match d {
                    365 => "In the last year".to_string(),
                    d => format!("In the last {d} days"),
                };
                choice(label, format!("added:<{d}d"))
            })
            .collect(),
        Kind::Bpm => Vec::new(),
    };
    // Rarely used values stay out of short lists' way, but a picked one is
    // always there to unpick.
    let fixed = matches!(
        field.kind,
        Kind::Rating | Kind::Color | Kind::Played | Kind::Added
    );
    out.retain(|c| fixed || c.count > 0 || filters.is_picked(&field.id, &c.term));
    if let Some(picked) = filters.picked.get(&field.id) {
        for (term, label) in picked {
            if !out.iter().any(|c| c.term == *term) {
                out.push(choice(label.clone(), term.clone()));
            }
        }
    }
    out
}

/// The values a track has for a list field: one for text, each word for a
/// tag field (without its `#`), the year.
fn values_of(field: &Field, t: &Track) -> Vec<String> {
    let one = |s: &str| {
        let s = s.trim();
        if s.is_empty() {
            Vec::new()
        } else {
            vec![s.to_string()]
        }
    };
    match (field.kind, field.id.as_str()) {
        (Kind::Tags(i), _) => t
            .info
            .user_tag(i)
            .split_whitespace()
            .map(|w| w.trim_start_matches('#'))
            .filter(|w| !w.is_empty())
            .map(str::to_string)
            .collect(),
        (Kind::Year, _) => t
            .info
            .year
            .get(..4)
            .filter(|y| y.chars().all(|c| c.is_ascii_digit()))
            .map(|y| vec![y.to_string()])
            .unwrap_or_default(),
        (_, "genre") => one(&t.genre),
        (_, "artist") => one(&t.artist),
        (_, "album") => one(&t.album),
        (_, "grouping") => one(&t.info.grouping),
        _ => Vec::new(),
    }
}

/// Sorts "2 ONS" before "10 SONG": a leading number counts as a number.
fn natural(s: &str) -> (u64, String) {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    (digits.parse().unwrap_or(u64::MAX), s.to_lowercase())
}

/// Keys in Camelot order: 1A, 1B, 2A ...
fn camelot_order(k: Key) -> (u32, bool) {
    let c = k.camelot();
    let n = c.trim_end_matches(['A', 'B']).parse().unwrap_or(99);
    (n, c.ends_with('B'))
}

fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mixxx_library::query::{Indexed, Query};

    fn track(id: usize, genre: &str, grouping: &str, tags: &str, bpm: f64, key: &str) -> Track {
        let mut t = std::sync::Arc::unwrap_or_clone(mixxx_engine::sample_library().remove(0));
        t.id = id;
        t.genre = genre.into();
        t.bpm = bpm;
        t.key = key.into();
        t.info.grouping = grouping.into();
        t.info.user_tags = vec![tags.into()];
        t.info.labels = t.info.user_words();
        t.info.color = None;
        t.rating = 0;
        t
    }

    fn library() -> Vec<Track> {
        vec![
            track(1, "Ambient", "10 SONG", "#uplifting #bright", 120.0, "8A"),
            track(2, "Ambient", "2 ONS", "#lullaby", 90.0, "Dm"),
            track(3, "Deep House", "", "#uplifting", 124.0, "3B"),
        ]
    }

    fn find(lib: &[Track], text: &str) -> Vec<usize> {
        let q = Query::parse(text);
        let cx = Default::default();
        lib.iter()
            .filter(|t| q.matches(t, &Indexed::new(t), &cx))
            .map(|t| t.id)
            .collect()
    }

    fn field<'a>(fields: &'a [Field], id: &str) -> &'a Field {
        fields.iter().find(|f| f.id == id).unwrap()
    }

    #[test]
    fn lists_values_in_order_with_counts() {
        let lib = library();
        let fields = fields(&FieldNames::default());
        let f = Filters::default();
        let grouping = choices(field(&fields, "grouping"), lib.iter(), &f, true);
        let labels: Vec<_> = grouping
            .iter()
            .map(|c| (c.label.as_str(), c.count))
            .collect();
        assert_eq!(labels, [("2 ONS", 1), ("10 SONG", 1), ("(none)", 1)]);
        let tags = choices(field(&fields, "tag1"), lib.iter(), &f, true);
        let labels: Vec<_> = tags.iter().map(|c| (c.label.as_str(), c.count)).collect();
        assert_eq!(labels, [("bright", 1), ("lullaby", 1), ("uplifting", 2)]);
        let keys = choices(field(&fields, "key"), lib.iter(), &f, true);
        let labels: Vec<_> = keys.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, ["3B", "7A", "8A"]);
    }

    #[test]
    fn chips_narrow_the_list_together() {
        let lib = library();
        let fields = fields(&FieldNames::default());
        let mut f = Filters::default();
        let pick = |f: &mut Filters, id: &str, label: &str| {
            let c = choices(field(&fields, id), lib.iter(), f, true)
                .into_iter()
                .find(|c| c.label == label)
                .unwrap();
            f.toggle(id, &c);
        };
        pick(&mut f, "genre", "Ambient");
        assert_eq!(find(&lib, &f.text(&fields, None)), [1, 2]);
        pick(&mut f, "tag1", "uplifting");
        assert_eq!(find(&lib, &f.text(&fields, None)), [1]);
        pick(&mut f, "tag1", "lullaby");
        assert_eq!(find(&lib, &f.text(&fields, None)), [1, 2], "any word");
        f.match_all.insert("tag1".into());
        assert_eq!(
            find(&lib, &f.text(&fields, None)),
            Vec::<usize>::new(),
            "all words"
        );
        f.match_all.clear();
        assert_eq!(
            f.summary(field(&fields, "tag1"), None, true),
            "uplifting +1"
        );
        f.clear_all(&fields);
        assert_eq!(f.text(&fields, None), "");

        pick(&mut f, "grouping", "(none)");
        assert_eq!(find(&lib, &f.text(&fields, None)), [3]);
        f.clear_all(&fields);

        f.bpm_low = "100".into();
        assert_eq!(find(&lib, &f.text(&fields, None)), [1, 3]);
        f.bpm_high = "122".into();
        assert_eq!(find(&lib, &f.text(&fields, None)), [1]);
        f.clear_all(&fields);

        // Following the playing track: 122 BPM in 8A.
        let deck = Some((122.0, Key::parse("8A")));
        f.bpm_follow = true;
        f.tolerance = 0;
        assert_eq!(find(&lib, &f.text(&fields, deck)), [1, 3]);
        f.key_follow = true;
        assert_eq!(find(&lib, &f.text(&fields, deck)), [1]);
        assert_eq!(f.summary(field(&fields, "bpm"), deck, true), "122 ±2%");
        assert_eq!(f.summary(field(&fields, "key"), deck, true), "Match 8A");
        assert_eq!(f.text(&fields, None), "", "nothing playing leaves all in");
    }

    #[test]
    fn pinned_chips_then_the_ones_in_use() {
        let fields = fields(&FieldNames::default());
        let mut f = Filters::new(vec!["bpm".into(), "genre".into()], 2);
        let ids = |f: &Filters| {
            f.shown(&fields)
                .iter()
                .map(|x| x.id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&f), ["bpm", "genre"]);
        f.added.push("year".into());
        f.key_follow = true;
        assert_eq!(ids(&f), ["bpm", "genre", "year", "key"]);
        f.clear_all(&fields);
        assert_eq!(ids(&f), ["bpm", "genre"]);
    }
}
