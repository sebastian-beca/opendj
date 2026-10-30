//! Crates and sets in the browser (see [`opendj_library::sets`]): the crates
//! each track is in, for the Crates column and `crate:`; New set… from the
//! template in the settings; History's "Add to Set List…"; and tidying the
//! lists from Collection Health.

use opendj_library::sets::ListHealth;
use opendj_library::stages::{Prep, Stage};
use slint::{ModelRc, SharedString, VecModel};

use super::health::tracks;
use super::{Browser, KIND_SET};
use crate::{settings, LibraryEdit};

impl Browser {
    /// Reads again which crates each track is in, and what Collection Health
    /// finds in the lists.
    pub(super) fn refresh_crates(&mut self) {
        let Some(db) = &self.db else { return };
        self.crates = db.crates_by_track().unwrap_or_default();
        self.list_health = db.list_health().unwrap_or_default();
    }

    /// Makes a set from the template and selects it. Returns its row in the
    /// tree, so the tree can ask for its name.
    pub(super) fn create_set(&mut self) -> i32 {
        if !self.saved() || self.history_tab {
            return -1;
        }
        let Some(db) = &mut self.db else { return -1 };
        match db.create_set("New set", &settings::set_template()) {
            Ok(id) => {
                self.reload_tree(Some(id));
                if let Some(n) = self.nodes.get_mut(self.current_node) {
                    n.expanded = true;
                }
                self.refresh_tree();
                self.visible_nodes
                    .iter()
                    .position(|&n| n == self.current_node)
                    .map_or(-1, |p| p as i32)
            }
            Err(e) => {
                eprintln!("can't make a set: {e}");
                -1
            }
        }
    }

    /// Offers the sets in the track menu, for "Add to Set List…" in
    /// History.
    pub(super) fn refresh_set_menu(&mut self, edit: &LibraryEdit) {
        let sets = self
            .db
            .as_ref()
            .and_then(|db| db.sets().ok())
            .unwrap_or_default();
        let names: Vec<SharedString> = sets.iter().map(|(_, n)| n.into()).collect();
        self.menu_sets = sets.into_iter().map(|(id, _)| id).collect();
        edit.set_sets(ModelRc::new(VecModel::from(names)));
        edit.set_history(self.history_tab);
    }

    /// Adds the tracks of rows `first..=last` to the Live set of the set
    /// at `set` in the track menu.
    pub(super) fn add_rows_to_set(&mut self, first: usize, last: usize, set: usize) {
        let Some(&set) = self.menu_sets.get(set) else {
            return;
        };
        let Some(db) = &self.db else { return };
        match db.live_set(set) {
            Ok(list) => {
                self.add_rows_to_playlist(first, last, list);
                // A Live set made just now shows in the tree.
                self.reload_tree(None);
            }
            Err(e) => eprintln!("can't find the set's Live set: {e}"),
        }
    }

    /// For a set: how many of its tracks are in In Prep still missing basic
    /// prep, to add to the list's summary.
    pub(super) fn set_note(&self, indices: &[usize]) -> Option<String> {
        if self.nodes.get(self.current_node)?.kind != KIND_SET {
            return None;
        }
        let unready = indices
            .iter()
            .map(|&i| &self.library[i])
            .filter(|t| t.info.stage == Stage::Triage.name() && !Prep::of(t).done())
            .count();
        (unready > 0).then(|| {
            format!(
                "{} in In Prep {} not play ready",
                tracks(unready),
                if unready == 1 { "is" } else { "are" }
            )
        })
    }

    /// What Collection Health says about the lists; empty when they're fine.
    pub(super) fn list_health_text(&self) -> String {
        let ListHealth { empty, dead } = &self.list_health;
        let mut parts = Vec::new();
        if !empty.is_empty() {
            parts.push(format!(
                "{} empty crate{}",
                empty.len(),
                if empty.len() == 1 { "" } else { "s" }
            ));
        }
        if *dead > 0 {
            parts.push(format!(
                "{dead} entr{} for tracks removed from your collection",
                if *dead == 1 { "y" } else { "ies" }
            ));
        }
        parts.join(", ")
    }

    /// Deletes empty crates and entries for removed tracks.
    pub(super) fn tidy_lists(&mut self) {
        let Some(db) = &mut self.db else { return };
        self.health_status = match db.tidy_lists() {
            Ok((lists, entries)) => {
                let mut s = Vec::new();
                if lists > 0 {
                    s.push(format!(
                        "Deleted {lists} empty crate{}",
                        if lists == 1 { "" } else { "s" }
                    ));
                }
                if entries > 0 {
                    s.push(format!(
                        "took {entries} removed track{} out of lists",
                        if entries == 1 { "" } else { "s" }
                    ));
                }
                if s.is_empty() {
                    "The lists were already tidy.".into()
                } else {
                    let s = s.join(" and ");
                    let mut c = s.chars();
                    c.next()
                        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                        .unwrap_or_default()
                        + "."
                }
            }
            Err(e) => format!("Couldn't tidy the lists: {e}"),
        };
        self.reload_tree(None);
        self.refresh_health_ui();
    }
}
