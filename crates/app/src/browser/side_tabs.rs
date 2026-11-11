//! Which side panel tabs show and in what order: the chooser that a
//! right-click on the tab titles opens (library.slint, `SideTabs`).
//!
//! The order and the switches are kept in settings.json under `side_tabs`,
//! by name. Lyrics starts switched off; Cues and Checklist are listed only
//! in the Prep layout.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, VecModel};

use crate::{AppWindow, CueList, SideTab, SideTabs};

/// Each tab's name, by its number as SidePanel's `tab` has it.
const NAMES: [&str; 7] = [
    "Automix",
    "Sidelist",
    "Related",
    "Info",
    "Cues",
    "Checklist",
    "Lyrics",
];

/// The tabs on a fresh install, in order, and whether each shows.
const DEFAULT: [(usize, bool); 7] = [
    (0, true),
    (1, true),
    (2, true),
    (3, true),
    (6, false),
    (4, true),
    (5, true),
];

/// Cues and Checklist: only in the Prep layout.
fn prep_only(tab: usize) -> bool {
    tab == 4 || tab == 5
}

thread_local! {
    static TABS: RefCell<Vec<(usize, bool)>> = RefCell::new(DEFAULT.to_vec());
}

/// The saved tabs, with any the save doesn't know (new in this version, or
/// a mangled file) added at the end.
fn from_saved(saved: Option<Vec<(String, bool)>>) -> Vec<(usize, bool)> {
    let Some(saved) = saved else {
        return DEFAULT.to_vec();
    };
    let mut tabs: Vec<(usize, bool)> = Vec::new();
    for (name, on) in saved {
        if let Some(i) = NAMES.iter().position(|n| *n == name) {
            if !tabs.iter().any(|(t, _)| *t == i) {
                tabs.push((i, on));
            }
        }
    }
    for &(i, on) in DEFAULT.iter() {
        if !tabs.iter().any(|(t, _)| *t == i) {
            tabs.push((i, on));
        }
    }
    // One tab always shows.
    if !tabs.iter().any(|&(t, on)| on && !prep_only(t)) {
        tabs.iter_mut().for_each(|(t, on)| *on |= *t == 0);
    }
    tabs
}

/// The tabs the chooser lists, by their place in `tabs`.
fn listed(tabs: &[(usize, bool)], prep: bool) -> Vec<usize> {
    (0..tabs.len())
        .filter(|&k| prep || !prep_only(tabs[k].0))
        .collect()
}

/// Moves tab `tab` `steps` rows along the chooser's list; tabs it doesn't
/// list keep their places.
fn moved(tabs: &mut [(usize, bool)], prep: bool, tab: usize, steps: i32) {
    let slots = listed(tabs, prep);
    let Some(from) = slots.iter().position(|&k| tabs[k].0 == tab) else {
        return;
    };
    let to = (from as i32 + steps).clamp(0, slots.len() as i32 - 1) as usize;
    if to == from {
        return;
    }
    let mut order: Vec<(usize, bool)> = slots.iter().map(|&k| tabs[k]).collect();
    let item = order.remove(from);
    order.insert(to, item);
    for (&k, t) in slots.iter().zip(order) {
        tabs[k] = t;
    }
}

/// Switches tab `tab` on or off; the last one on in the list stays on.
fn toggled(tabs: &mut [(usize, bool)], prep: bool, tab: usize) {
    let on_count = listed(tabs, prep).iter().filter(|&&k| tabs[k].1).count();
    if let Some(t) = tabs.iter_mut().find(|(t, _)| *t == tab) {
        if t.1 && on_count <= 1 {
            return;
        }
        t.1 = !t.1;
    }
}

/// The first tab that shows, for when the open one goes away.
fn first_shown(tabs: &[(usize, bool)], prep: bool) -> usize {
    tabs.iter()
        .find(|&&(t, on)| on && (prep || !prep_only(t)))
        .map_or(0, |&(t, _)| t)
}

/// Whether tab `tab` is switched on and listed.
fn shows(tabs: &[(usize, bool)], prep: bool, tab: usize) -> bool {
    tabs.iter()
        .any(|&(t, on)| t == tab && on && (prep || !prep_only(t)))
}

fn publish(app: &AppWindow) {
    let tabs = TABS.with(|t| t.borrow().clone());
    let (mut pos, mut plain) = (0, 0);
    let rows: Vec<SideTab> = tabs
        .iter()
        .map(|&(i, on)| {
            let row = SideTab {
                name: NAMES[i].into(),
                i: i as i32,
                on,
                pos,
                pos_plain: if prep_only(i) { -1 } else { plain },
            };
            pos += 1;
            if !prep_only(i) {
                plain += 1;
            }
            row
        })
        .collect();
    let ui = app.global::<SideTabs>();
    ui.set_listed(pos);
    ui.set_listed_plain(plain);
    ui.set_on_count(tabs.iter().filter(|t| t.1).count() as i32);
    ui.set_on_count_plain(tabs.iter().filter(|t| t.1 && !prep_only(t.0)).count() as i32);
    ui.set_tabs(Rc::new(VecModel::from(rows)).into());
}

fn save() {
    TABS.with(|t| {
        let tabs: Vec<(&str, bool)> = t.borrow().iter().map(|&(i, on)| (NAMES[i], on)).collect();
        crate::settings::set_side_tabs(&tabs);
    });
}

/// Opens the first tab that shows if the open one was switched off, or is
/// Prep's outside Prep.
pub(super) fn keep_open_tab(app: &AppWindow, prep: bool) {
    let tab = app.get_side_tab().max(0) as usize;
    let gone = TABS.with(|t| {
        let t = t.borrow();
        let known = t.iter().any(|&(i, _)| i == tab);
        // A tab switched off may stay open (Show Info opens Info), but not
        // one of Prep's outside Prep.
        (!prep && prep_only(tab)) || !known
    });
    if gone {
        let first = TABS.with(|t| first_shown(&t.borrow(), prep));
        app.set_side_tab(first as i32);
    }
}

/// Loads the saved tabs and answers the chooser.
pub(super) fn install(app: &AppWindow) {
    TABS.with(|t| *t.borrow_mut() = from_saved(crate::settings::side_tabs()));
    publish(app);
    // The panel opens on Automix; on the first tab that shows if that's off.
    let prep = app.global::<CueList>().get_prep();
    let first = TABS.with(|t| {
        let t = t.borrow();
        (!shows(&t, prep, app.get_side_tab().max(0) as usize)).then(|| first_shown(&t, prep))
    });
    if let Some(first) = first {
        app.set_side_tab(first as i32);
    }
    let ui = app.global::<SideTabs>();
    let weak = app.as_weak();
    ui.on_toggled(move |tab| {
        let Some(app) = weak.upgrade() else { return };
        let prep = app.global::<CueList>().get_prep();
        let tab = tab.max(0) as usize;
        let next = TABS.with(|t| {
            let mut t = t.borrow_mut();
            toggled(&mut t, prep, tab);
            // Switching off the open tab opens the first one left.
            let open = app.get_side_tab().max(0) as usize;
            (open == tab && !shows(&t, prep, tab)).then(|| first_shown(&t, prep))
        });
        if let Some(next) = next {
            app.set_side_tab(next as i32);
        }
        save();
        publish(&app);
    });
    let weak = app.as_weak();
    ui.on_moved(move |tab, steps| {
        let Some(app) = weak.upgrade() else { return };
        let prep = app.global::<CueList>().get_prep();
        TABS.with(|t| moved(&mut t.borrow_mut(), prep, tab.max(0) as usize, steps));
        save();
        publish(&app);
    });
    let weak = app.as_weak();
    ui.on_reset(move || {
        let Some(app) = weak.upgrade() else { return };
        let prep = app.global::<CueList>().get_prep();
        TABS.with(|t| *t.borrow_mut() = DEFAULT.to_vec());
        let open = app.get_side_tab().max(0) as usize;
        if !TABS.with(|t| shows(&t.borrow(), prep, open)) {
            app.set_side_tab(TABS.with(|t| first_shown(&t.borrow(), prep)) as i32);
        }
        save();
        publish(&app);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(tabs: &[(usize, bool)]) -> Vec<&'static str> {
        tabs.iter().map(|&(i, _)| NAMES[i]).collect()
    }

    #[test]
    fn fresh_install_hides_lyrics() {
        let tabs = from_saved(None);
        assert_eq!(
            names(&tabs),
            [
                "Automix",
                "Sidelist",
                "Related",
                "Info",
                "Lyrics",
                "Cues",
                "Checklist"
            ]
        );
        assert!(!shows(&tabs, false, 6));
        assert!(shows(&tabs, false, 3));
    }

    #[test]
    fn saved_order_kept_and_unknown_ones_filled_in() {
        let saved = vec![
            ("Info".to_string(), true),
            ("Bogus".to_string(), true),
            ("Automix".to_string(), false),
            ("Info".to_string(), false),
        ];
        let tabs = from_saved(Some(saved));
        assert_eq!(tabs[0], (3, true));
        assert_eq!(tabs[1], (0, false));
        assert_eq!(tabs.len(), 7);
        assert!(names(&tabs).contains(&"Lyrics"));
    }

    #[test]
    fn nothing_on_turns_automix_on() {
        let saved = NAMES.iter().map(|n| (n.to_string(), false)).collect();
        let tabs = from_saved(Some(saved));
        assert!(shows(&tabs, false, 0));
    }

    #[test]
    fn moving_outside_prep_skips_prep_tabs() {
        let mut tabs = vec![(0, true), (4, true), (1, true), (5, true), (2, true)];
        // Automix down two of the listed rows: past Sidelist and Related.
        moved(&mut tabs, false, 0, 2);
        assert_eq!(
            tabs,
            vec![(1, true), (4, true), (2, true), (5, true), (0, true)]
        );
        moved(&mut tabs, false, 0, -10);
        assert_eq!(
            tabs,
            vec![(0, true), (4, true), (1, true), (5, true), (2, true)]
        );
    }

    #[test]
    fn moving_in_prep() {
        let mut tabs = DEFAULT.to_vec();
        moved(&mut tabs, true, 5, -6);
        assert_eq!(names(&tabs)[0], "Checklist");
    }

    #[test]
    fn last_tab_on_stays_on() {
        let mut tabs = vec![(0, true), (1, false), (4, true)];
        toggled(&mut tabs, false, 0);
        assert!(tabs[0].1);
        // In Prep, Cues counts, so Automix can go.
        toggled(&mut tabs, true, 0);
        assert!(!tabs[0].1);
        assert_eq!(first_shown(&tabs, true), 4);
    }
}
