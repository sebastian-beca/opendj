//! The track list's columns: which are shown, in what order and how wide,
//! as the DJ sets them from the header (right-click to pick, drag an edge
//! to resize, drag a heading to move it). Kept in settings as `columns`.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::rc::Rc;

use mixxx_engine::key::Key;
use mixxx_engine::Track;
use mixxx_library::query::FieldNames;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use super::Browser;
use crate::{settings, AppWindow, ColumnUi, ListColumn, TrackRow};

/// How a column is drawn.
pub const TEXT: i32 = 0;
/// Takes the room left over.
pub const TITLE: i32 = 1;
pub const RATING: i32 = 2;
pub const KEY: i32 = 3;
/// Quieter text: genre, comment and the like.
pub const DIM: i32 = 4;
/// A tag field's words, in lilac.
pub const TAG_WORDS: i32 = 5;

/// Column ids. The first eight are the list's original sort columns, so
/// sort orders saved before stay as they were.
pub const ARTIST: i32 = 0;
pub const TITLE_ID: i32 = 1;
pub const ALBUM: i32 = 2;
pub const GENRE: i32 = 3;
pub const BPM: i32 = 4;
pub const KEY_ID: i32 = 5;
pub const LENGTH: i32 = 6;
pub const RATING_ID: i32 = 7;
pub const GROUPING: i32 = 8;
pub const YEAR: i32 = 9;
pub const PLAYED: i32 = 10;
pub const ADDED: i32 = 11;
pub const LAST_PLAYED: i32 = 12;
pub const COMMENT: i32 = 13;
pub const LABEL: i32 = 14;
pub const REMIX: i32 = 15;
pub const COMPOSER: i32 = 16;
pub const TRACK_NUMBER: i32 = 17;
/// Tag field `i` (from 0) is column `TAGS + i`.
pub const TAGS: i32 = 100;

/// A column the list can show.
#[derive(Clone, Debug, PartialEq)]
pub struct Def {
    pub id: i32,
    pub label: String,
    pub width: f32,
    pub right: bool,
    pub kind: i32,
}

/// Every column, in the order the header's menu lists them; grouping and
/// the tag fields go by the names the DJ gave them.
pub fn all(names: &FieldNames) -> Vec<Def> {
    let d = |id, label: &str, width, right, kind| Def {
        id,
        label: label.into(),
        width,
        right,
        kind,
    };
    let mut out = vec![
        d(RATING_ID, "Rating", 64.0, false, RATING),
        d(TITLE_ID, "Title", 0.0, false, TITLE),
        d(ARTIST, "Artist", 170.0, false, TEXT),
        d(ALBUM, "Album", 150.0, false, TEXT),
        d(REMIX, "Remix", 120.0, false, TEXT),
        d(LENGTH, "Length", 60.0, true, TEXT),
        d(BPM, "Bpm", 56.0, true, TEXT),
        d(KEY_ID, "Key", 44.0, false, KEY),
        d(GENRE, "Genre", 110.0, false, DIM),
        d(GROUPING, &names.grouping, 90.0, false, TEXT),
    ];
    for (i, name) in names.tags.iter().enumerate() {
        out.push(d(TAGS + i as i32, name, 140.0, false, TAG_WORDS));
    }
    out.extend([
        d(YEAR, "Year", 48.0, true, DIM),
        d(TRACK_NUMBER, "Track #", 52.0, true, DIM),
        d(PLAYED, "Plays", 48.0, true, DIM),
        d(ADDED, "Added", 86.0, false, DIM),
        d(LAST_PLAYED, "Last Played", 86.0, false, DIM),
        d(LABEL, "Label", 110.0, false, DIM),
        d(COMPOSER, "Composer", 110.0, false, DIM),
        d(COMMENT, "Comment", 160.0, false, DIM),
    ]);
    out
}

/// The columns shown when the DJ hasn't chosen: the list as it always was.
pub const DEFAULT: [i32; 7] = [RATING_ID, TITLE_ID, ARTIST, LENGTH, BPM, KEY_ID, GENRE];

/// Narrowest a column can be dragged.
pub const MIN_WIDTH: f32 = 30.0;

/// The columns shown, in order, with their widths.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub shown: Vec<(i32, f32)>,
}

impl Layout {
    /// The saved layout, or the default; unknown columns are dropped and
    /// the title is always there.
    pub fn new(saved: Vec<(i32, f32)>, defs: &[Def]) -> Self {
        let mut l = Layout { shown: saved };
        if l.shown.is_empty() {
            l = Self::default_for(defs);
        }
        l.fit(defs);
        l
    }

    pub fn default_for(defs: &[Def]) -> Self {
        Layout {
            shown: DEFAULT
                .iter()
                .filter_map(|id| defs.iter().find(|d| d.id == *id))
                .map(|d| (d.id, d.width))
                .collect(),
        }
    }

    /// Drops columns that no longer exist (a tag field removed) and puts
    /// the title back if it went missing.
    pub fn fit(&mut self, defs: &[Def]) {
        let mut seen = Vec::new();
        self.shown.retain(|(id, _)| {
            defs.iter().any(|d| d.id == *id) && !seen.contains(id) && {
                seen.push(*id);
                true
            }
        });
        if !self.shown.iter().any(|(id, _)| *id == TITLE_ID) {
            self.shown.insert(0, (TITLE_ID, 0.0));
        }
    }

    pub fn has(&self, id: i32) -> bool {
        self.shown.iter().any(|(c, _)| *c == id)
    }

    /// Shows a column (at the end) or hides it; the title stays.
    pub fn toggle(&mut self, id: i32, defs: &[Def]) {
        if id == TITLE_ID {
            return;
        }
        match self.shown.iter().position(|(c, _)| *c == id) {
            Some(i) => {
                self.shown.remove(i);
            }
            None => {
                if let Some(d) = defs.iter().find(|d| d.id == id) {
                    self.shown.push((id, d.width));
                }
            }
        }
    }

    pub fn resize(&mut self, index: usize, width: f32) {
        if let Some(c) = self.shown.get_mut(index) {
            let min = if c.0 == TITLE_ID { 80.0 } else { MIN_WIDTH };
            c.1 = width.max(min).round();
        }
    }

    /// Where a column dropped at `x` along the header goes among the columns
    /// as they are now (before the first whose middle is right of `x`), and
    /// where that slot starts, given the title's width.
    pub fn slot(&self, x: f32, title_width: f32, spacing: f32) -> (usize, f32) {
        let mut left = 0.0;
        for (i, (id, w)) in self.shown.iter().enumerate() {
            let w = if *id == TITLE_ID { title_width } else { *w };
            if x < left + w / 2.0 {
                return (i, left);
            }
            left += w + spacing;
        }
        (self.shown.len(), left)
    }

    /// Moves the column at `from` to where `x` falls along the header,
    /// given the title's width.
    pub fn move_to(&mut self, from: usize, x: f32, title_width: f32, spacing: f32) {
        if from >= self.shown.len() {
            return;
        }
        let (to, _) = self.slot(x, title_width, spacing);
        let c = self.shown.remove(from);
        let to = if to > from { to - 1 } else { to };
        self.shown.insert(to.min(self.shown.len()), c);
    }
}

/// What a column shows for a track in the library. Columns the list fills
/// from its own row (title, artist, album, genre, length, BPM, key) aren't
/// asked for.
pub fn cell(id: i32, t: &Track, camelot: bool) -> String {
    let i = &t.info;
    let date = |at: i64| {
        use chrono::TimeZone;
        match chrono::Local.timestamp_opt(at, 0).single() {
            Some(d) if at > 0 => d.format("%Y-%m-%d").to_string(),
            _ => String::new(),
        }
    };
    let number = |n: u32| if n > 0 { n.to_string() } else { String::new() };
    match id {
        GROUPING => i.grouping.clone(),
        YEAR => i.year.get(..4).unwrap_or(&i.year).to_string(),
        PLAYED => number(i.play_count),
        ADDED => date(i.added),
        LAST_PLAYED => i.last_played.map(date).unwrap_or_default(),
        COMMENT => i.comment.replace('\n', " "),
        LABEL => i.label.clone(),
        REMIX => i.remix.clone(),
        COMPOSER => i.composer.clone(),
        TRACK_NUMBER => number(i.track_number),
        KEY_ID => crate::settings::show_key(&t.key, camelot),
        id if id >= TAGS => i
            .user_tag((id - TAGS) as usize)
            .split_whitespace()
            .map(|w| w.trim_start_matches('#'))
            .collect::<Vec<_>>()
            .join("  "),
        _ => String::new(),
    }
}

/// A row's cells for the columns `ids`: what the row already holds for
/// the list's own columns, the rest from its track (none for a store
/// result or a Spotify track the collection doesn't have). The title and
/// rating are drawn from the row itself.
pub fn row_cells(ids: &[i32], row: &TrackRow, track: Option<&Track>) -> ModelRc<SharedString> {
    let cells: Vec<SharedString> = ids
        .iter()
        .map(|&id| match id {
            ARTIST => row.artist.clone(),
            ALBUM => row.album.clone(),
            GENRE => row.genre.clone(),
            LENGTH => row.length.clone(),
            BPM => row.bpm.clone(),
            KEY_ID => row.key.clone(),
            TITLE_ID | RATING_ID => SharedString::new(),
            id => track.map(|t| cell(id, t, true).into()).unwrap_or_default(),
        })
        .collect();
    ModelRc::new(VecModel::from(cells))
}

/// Orders two tracks by a column the row data doesn't hold; numbers and
/// dates by value, the rest as text with a leading number counted as one
/// ("2 ONS" before "10 SONG"), empty last.
pub fn compare(id: i32, a: &Track, b: &Track) -> Ordering {
    let (x, y) = (&a.info, &b.info);
    match id {
        PLAYED => x.play_count.cmp(&y.play_count),
        ADDED => x.added.cmp(&y.added),
        LAST_PLAYED => x.last_played.cmp(&y.last_played),
        TRACK_NUMBER => x.track_number.cmp(&y.track_number),
        KEY_ID => key_order(&a.key).cmp(&key_order(&b.key)),
        _ => {
            let (s, t) = (cell(id, a, true), cell(id, b, true));
            match (s.is_empty(), t.is_empty()) {
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                _ => natural(&s).cmp(&natural(&t)),
            }
        }
    }
}

/// Keys in Camelot order (1A, 1B, 2A …), unknown keys last.
fn key_order(key: &str) -> (u32, bool) {
    match Key::parse(key) {
        Some(k) => {
            let c = k.camelot();
            (
                c.trim_end_matches(['A', 'B']).parse().unwrap_or(99),
                c.ends_with('B'),
            )
        }
        None => (99, false),
    }
}

fn natural(s: &str) -> (u64, String) {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    (digits.parse().unwrap_or(u64::MAX), s.to_lowercase())
}

/// The list's columns in the browser.
pub(super) struct ColumnState {
    pub defs: Vec<Def>,
    pub layout: Layout,
}

impl ColumnState {
    pub fn new(names: &FieldNames) -> Self {
        let defs = all(names);
        let layout = Layout::new(settings::columns(), &defs);
        ColumnState { defs, layout }
    }

    fn def(&self, id: i32) -> Option<&Def> {
        self.defs.iter().find(|d| d.id == id)
    }

    /// The ids of the columns shown, in order.
    pub fn ids(&self) -> Vec<i32> {
        self.layout.shown.iter().map(|(id, _)| *id).collect()
    }

    /// How wide the list must be for the columns, with room for the title.
    fn min_width(&self) -> f32 {
        let fixed: f32 = self
            .layout
            .shown
            .iter()
            .map(|&(id, w)| shown_width(id, w) + SPACING)
            .sum();
        HEADER_LEAD + HEADER_TRAIL + fixed
    }

    /// The title's width: like the others, it keeps its own.
    fn title_width(&self) -> f32 {
        self.layout
            .shown
            .iter()
            .find(|(id, _)| *id == TITLE_ID)
            .map_or(TITLE_ROOM, |&(id, w)| shown_width(id, w))
    }
}

/// How the header lays out: padding before the columns, the album art and
/// deck badge, and the spacing between cells (as in `TrackList`).
const HEADER_LEAD: f32 = 6.0 + 26.0 + 8.0 + 22.0 + 8.0;
const HEADER_TRAIL: f32 = 14.0;
const SPACING: f32 = 8.0;
/// The title's width until it's resized.
const TITLE_ROOM: f32 = 220.0;

/// A column's width as laid out: the title's is unset (0) until resized.
fn shown_width(id: i32, width: f32) -> f32 {
    if id == TITLE_ID && width <= 0.0 {
        TITLE_ROOM
    } else {
        width
    }
}

impl Browser {
    pub(super) fn install_columns(app: &AppWindow, b: &Rc<RefCell<Self>>) {
        let ui = app.global::<ColumnUi>();
        let weak = Rc::downgrade(b);
        let with = move |f: &dyn Fn(&mut Browser)| {
            if let Some(b) = weak.upgrade() {
                f(&mut b.borrow_mut());
            }
        };
        let w = with.clone();
        ui.on_toggle(move |i| {
            w(&|b| {
                let Some(id) = b.columns.defs.get(i.max(0) as usize).map(|d| d.id) else {
                    return;
                };
                let c = &mut b.columns;
                c.layout.toggle(id, &c.defs);
                b.columns_changed();
            })
        });
        let w = with.clone();
        ui.on_reset(move || {
            w(&|b| {
                b.columns.layout = Layout::default_for(&b.columns.defs);
                b.columns_changed();
            })
        });
        let w = with.clone();
        ui.on_resize(move |i, width| {
            w(&|b| {
                // Only the widths change: the rows stay as they are, and the
                // layout is kept once the drag ends. The column is changed in
                // place, since a new list would rebuild the heading being
                // dragged and drop the drag.
                let i = i.max(0) as usize;
                b.columns.layout.resize(i, width);
                let Some(app) = b.app.upgrade() else {
                    return;
                };
                let ui = app.global::<ColumnUi>();
                let model = ui.get_columns();
                if let (Some(mut col), Some(&(_, width))) =
                    (model.row_data(i), b.columns.layout.shown.get(i))
                {
                    col.width = width;
                    model.set_row_data(i, col);
                }
                ui.set_min_width(b.columns.min_width());
            })
        });
        let w = with.clone();
        ui.on_resized(move || w(&|b| settings::set_columns(&b.columns.layout.shown)));
        let w = with.clone();
        ui.on_drag(move |i, x, _header| {
            w(&|b| {
                // Mark where the heading would land, unless it stays put.
                let c = &b.columns;
                let (to, left) = c.layout.slot(x - HEADER_LEAD, c.title_width(), SPACING);
                let from = i.max(0) as usize;
                let at = if to == from || to == from + 1 {
                    -1.0
                } else {
                    HEADER_LEAD + left - SPACING / 2.0
                };
                if let Some(app) = b.app.upgrade() {
                    app.global::<ColumnUi>().set_drop_x(at);
                }
            })
        });
        let w = with.clone();
        ui.on_move(move |i, x, _header| {
            w(&|b| {
                let title = b.columns.title_width();
                b.columns
                    .layout
                    .move_to(i.max(0) as usize, x - HEADER_LEAD, title, SPACING);
                b.columns_changed();
            })
        });
        b.borrow().refresh_column_ui();
    }

    /// The columns shown changed: keep them and list again.
    fn columns_changed(&mut self) {
        settings::set_columns(&self.columns.layout.shown);
        self.refresh_column_ui();
        self.refresh_rows();
    }

    /// Tag fields or grouping were renamed: the columns follow.
    pub(super) fn column_names_changed(&mut self, names: &FieldNames) {
        let c = &mut self.columns;
        c.defs = all(names);
        c.layout.fit(&c.defs);
        self.refresh_column_ui();
    }

    pub(super) fn refresh_column_ui(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<ColumnUi>();
        let c = &self.columns;
        let shown: Vec<ListColumn> = c
            .layout
            .shown
            .iter()
            .filter_map(|&(id, width)| {
                let d = c.def(id)?;
                Some(ListColumn {
                    id,
                    label: d.label.clone().into(),
                    width: shown_width(id, width),
                    right: d.right,
                    kind: d.kind,
                })
            })
            .collect();
        ui.set_min_width(c.min_width());
        ui.set_columns(ModelRc::new(VecModel::from(shown)));
        let labels: Vec<SharedString> = c.defs.iter().map(|d| d.label.clone().into()).collect();
        ui.set_all(ModelRc::new(VecModel::from(labels)));
        let on: Vec<bool> = c.defs.iter().map(|d| c.layout.has(d.id)).collect();
        ui.set_all_shown(ModelRc::new(VecModel::from(on)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(l: &Layout) -> Vec<i32> {
        l.shown.iter().map(|(id, _)| *id).collect()
    }

    #[test]
    fn picks_resizes_and_moves_columns() {
        let defs = all(&FieldNames::default());
        let mut l = Layout::new(Vec::new(), &defs);
        assert_eq!(ids(&l), DEFAULT);
        l.toggle(GROUPING, &defs);
        l.toggle(GENRE, &defs);
        l.toggle(TITLE_ID, &defs);
        assert_eq!(
            ids(&l),
            [RATING_ID, TITLE_ID, ARTIST, LENGTH, BPM, KEY_ID, GROUPING]
        );
        l.resize(2, 10.0);
        assert_eq!(l.shown[2].1, MIN_WIDTH);
        l.resize(2, 200.4);
        assert_eq!(l.shown[2].1, 200.0);

        // Rating 64, title 300, artist 200, length 60 ... with 8 between.
        // Dropping rating in the middle of the artist column puts it there.
        l.move_to(0, 64.0 + 8.0 + 300.0 + 8.0 + 150.0, 300.0, 8.0);
        assert_eq!(ids(&l)[..3], [TITLE_ID, ARTIST, RATING_ID]);
        // Dropped before everything: first.
        l.move_to(2, 0.0, 300.0, 8.0);
        assert_eq!(ids(&l)[..3], [RATING_ID, TITLE_ID, ARTIST]);
        // Dropped past the end: last.
        l.move_to(0, 5000.0, 300.0, 8.0);
        assert_eq!(ids(&l).last(), Some(&RATING_ID));
    }

    #[test]
    fn keeps_the_title_and_drops_gone_columns() {
        let defs = all(&FieldNames::default());
        let l = Layout::new(
            vec![(ARTIST, 100.0), (TAGS + 5, 80.0), (ARTIST, 90.0)],
            &defs,
        );
        assert_eq!(l.shown, [(TITLE_ID, 0.0), (ARTIST, 100.0)]);
    }

    #[test]
    fn shows_and_sorts_library_fields() {
        let mut a = std::sync::Arc::unwrap_or_clone(mixxx_engine::sample_library().remove(0));
        let mut b = a.clone();
        a.info.grouping = "10 SONG".into();
        b.info.grouping = "2 ONS".into();
        a.info.user_tags = vec!["#uplifting #bright".into()];
        a.info.play_count = 3;
        assert_eq!(cell(TAGS, &a, true), "uplifting  bright");
        assert_eq!(cell(PLAYED, &a, true), "3");
        assert_eq!(cell(PLAYED, &b, true), "");
        assert_eq!(compare(GROUPING, &b, &a), Ordering::Less);
        assert_eq!(compare(PLAYED, &b, &a), Ordering::Less);
        b.info.grouping.clear();
        assert_eq!(compare(GROUPING, &a, &b), Ordering::Less, "empty last");
    }
}
