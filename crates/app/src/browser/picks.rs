//! The tracks picked in the track list: one row, a shift-click range, or
//! any rows added and dropped with Cmd-click (Ctrl-click on Windows and
//! Linux). The list asks for picks by row; the browser keeps them, so a
//! re-sorted or refreshed list keeps the same tracks picked.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashSet};
use std::rc::Rc;

use slint::{ComponentHandle, Model, ModelRc, VecModel};

use super::Browser;
use crate::{AppWindow, LibraryEdit};

/// The rows picked in the track list.
#[derive(Default)]
pub(super) struct Picks {
    /// Rows of the list, in order.
    rows: BTreeSet<usize>,
    /// What was picked before the shift-click range now being drawn, which
    /// a Cmd-shift-click keeps.
    base: BTreeSet<usize>,
    /// One flag per list row, for the list to draw.
    model: Rc<VecModel<bool>>,
}

impl Picks {
    /// Picks rows `from` to `to` (either way round), adding them to what
    /// was picked before the range when `add`; a negative `from` picks
    /// nothing.
    fn range(&mut self, from: i32, to: i32, add: bool, len: usize) {
        if !add {
            self.base.clear();
        }
        self.rows = self.base.clone();
        if from >= 0 && to >= 0 {
            let (a, b) = (from.min(to) as usize, from.max(to) as usize);
            self.rows.extend((a..=b).filter(|&r| r < len));
        }
    }

    /// Picks the row, or drops it when it was picked.
    fn toggle(&mut self, row: i32, len: usize) {
        let Ok(row) = usize::try_from(row) else {
            return;
        };
        if row >= len {
            return;
        }
        if !self.rows.remove(&row) {
            self.rows.insert(row);
        }
        // A later shift-click adds its range to these.
        self.base = self.rows.clone();
    }

    /// The rows an action on rows `first` to `last` applies to: every
    /// picked row when `first` is one of several picked, else that range.
    fn chosen(&self, first: usize, last: usize, len: usize) -> Vec<usize> {
        if self.rows.len() > 1 && self.rows.contains(&first) {
            self.rows.iter().copied().filter(|&r| r < len).collect()
        } else if len == 0 {
            Vec::new()
        } else {
            (first..=last.max(first).min(len - 1)).collect()
        }
    }

    /// Sets the flags the list draws, touching only the rows that changed.
    fn show(&self, len: usize) {
        let want = |i: usize| self.rows.contains(&i);
        if self.model.row_count() != len {
            self.model.set_vec((0..len).map(want).collect::<Vec<_>>());
            return;
        }
        for i in 0..len {
            if self.model.row_data(i) != Some(want(i)) {
                self.model.set_row_data(i, want(i));
            }
        }
    }
}

impl Browser {
    pub(super) fn install_picks(app: &AppWindow, b: &Rc<RefCell<Self>>) {
        let edit = app.global::<LibraryEdit>();
        edit.set_picks(ModelRc::from(b.borrow().picks.model.clone()));
        let w = Rc::downgrade(b);
        edit.on_pick_rows(move |from, to, add| {
            if let Some(b) = w.upgrade() {
                let mut b = b.borrow_mut();
                let len = b.rows.len();
                b.picks.range(from, to, add, len);
                b.show_picks();
            }
        });
        let w = Rc::downgrade(b);
        edit.on_toggle_row(move |row| {
            if let Some(b) = w.upgrade() {
                let mut b = b.borrow_mut();
                let len = b.rows.len();
                b.picks.toggle(row, len);
                b.show_picks();
            }
        });
    }

    /// Draws the picks and says how many there are.
    pub(super) fn show_picks(&self) {
        let len = self.rows.len();
        self.picks.show(len);
        if let Some(app) = self.app.upgrade() {
            let n = self.picks.rows.iter().filter(|&&r| r < len).count();
            let edit = app.global::<LibraryEdit>();
            edit.set_picked_count(n as i32);
            if n > 1 {
                let seconds: f64 = self
                    .picked_tracks()
                    .into_iter()
                    .map(|i| self.library[i].duration)
                    .sum();
                edit.set_picked_time(super::total_time(seconds).into());
            }
        }
    }

    /// The list rows an action on rows `first` to `last` applies to: all
    /// the picked rows when `first` is one of several picked.
    pub(super) fn chosen_rows(&self, first: usize, last: usize) -> Vec<usize> {
        self.picks.chosen(first, last, self.rows.len())
    }

    /// The library tracks of [`Browser::chosen_rows`].
    pub(super) fn chosen_tracks(&self, first: usize, last: usize) -> Vec<usize> {
        self.chosen_rows(first, last)
            .into_iter()
            .filter_map(|r| self.rows.get(r).copied().flatten())
            .collect()
    }

    /// The library tracks picked, in list order, when there are several.
    pub(super) fn picked_many(&self) -> Vec<usize> {
        if self.picks.rows.len() < 2 {
            return Vec::new();
        }
        self.picks
            .rows
            .iter()
            .filter_map(|&r| self.rows.get(r).copied().flatten())
            .collect()
    }

    /// The tracks picked now, to find again once the list is rebuilt.
    pub(super) fn picked_tracks(&self) -> HashSet<usize> {
        self.picks
            .rows
            .iter()
            .filter_map(|&r| self.rows.get(r).copied().flatten())
            .collect()
    }

    /// Picks the rows of `tracks` in the list as it is now (rows that are
    /// not collection tracks are dropped).
    pub(super) fn repick(&mut self, tracks: &HashSet<usize>) {
        let rows: BTreeSet<usize> = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, id)| id.is_some_and(|id| tracks.contains(&id)))
            .map(|(r, _)| r)
            .collect();
        self.picks.base = rows.clone();
        self.picks.rows = rows;
        self.show_picks();
    }

    /// Picks just this row, or nothing for a negative one.
    pub(super) fn pick_only(&mut self, row: i32) {
        let len = self.rows.len();
        self.picks.range(row, row, false, len);
        self.show_picks();
    }

    /// Drops every pick (the list shows something else).
    pub(super) fn clear_picks(&mut self) {
        self.picks.rows.clear();
        self.picks.base.clear();
        self.show_picks();
    }
}

#[cfg(test)]
mod tests {
    use super::Picks;

    fn rows(p: &Picks) -> Vec<usize> {
        p.rows.iter().copied().collect()
    }

    #[test]
    fn click_shift_click_and_cmd_click() {
        let mut p = Picks::default();
        p.range(2, 2, false, 10);
        assert_eq!(rows(&p), [2]);
        p.range(2, 5, false, 10);
        assert_eq!(rows(&p), [2, 3, 4, 5], "shift-click");
        p.toggle(8, 10);
        assert_eq!(rows(&p), [2, 3, 4, 5, 8], "cmd-click adds");
        p.toggle(3, 10);
        assert_eq!(rows(&p), [2, 4, 5, 8], "cmd-click drops");
        p.range(8, 9, true, 10);
        assert_eq!(rows(&p), [2, 4, 5, 8, 9], "cmd-shift-click adds a range");
        p.range(8, 7, true, 10);
        assert_eq!(
            rows(&p),
            [2, 4, 5, 7, 8],
            "the range is redrawn, not added up"
        );
        p.range(1, 1, false, 10);
        assert_eq!(rows(&p), [1], "a plain click starts again");
        p.range(-1, -1, false, 10);
        assert!(p.rows.is_empty(), "nothing current, nothing picked");
    }

    #[test]
    fn actions_take_the_picks_when_on_one_of_them() {
        let mut p = Picks::default();
        p.range(1, 1, false, 10);
        p.toggle(6, 10);
        assert_eq!(p.chosen(6, 6, 10), [1, 6]);
        assert_eq!(
            p.chosen(3, 3, 10),
            [3],
            "a row outside the picks goes alone"
        );
        assert_eq!(p.chosen(3, 20, 10), [3, 4, 5, 6, 7, 8, 9], "clipped");
        p.range(4, 4, false, 10);
        assert_eq!(p.chosen(4, 4, 10), [4]);
        assert!(p.chosen(0, 0, 0).is_empty());
    }
}
