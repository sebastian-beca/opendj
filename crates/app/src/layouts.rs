//! The DJ's layouts: which of openDJ's layouts the layout menu at the top
//! left offers, in their order (`layouts` in settings.json), and Settings ›
//! Layouts where they are added, taken out and reordered. A controller's
//! VIEW (`[Skin],layout_next`) steps through them.

use slint::{ComponentHandle, ModelRc, VecModel};

use crate::{settings, AppWindow, LayoutEntry, LayoutsUi};

/// The setting holding the DJ's layouts, as layout numbers in menu order.
const KEY: &str = "layouts";

/// Every layout, in the order More layouts shows them: its number (the
/// value of `layout`), its name and what it's for.
const ALL: [(i32, &str, &str); 5] = [
    (
        1,
        "Player",
        "A music player that mixes for you, with one bar along the bottom.",
    ),
    (
        4,
        "2-Deck",
        "Two light decks without jogs, with pads and loops in a bar under them.",
    ),
    (
        0,
        "2-Deck Pro",
        "Two full decks with jogs, tempo faders and pads.",
    ),
    (2, "4-Deck", "Four deck strips around the mixer."),
    (
        3,
        "Prep",
        "Deck 1 alone, for getting tracks ready: grid, cues and a checklist.",
    ),
];

/// The list a DJ starts with: the layouts the menu offered before it could
/// be changed. Prep, and layouts added later, start under More layouts.
const START: [i32; 4] = [1, 4, 0, 2];

/// Whether layout `l` exists and is offered (Prep only once it is).
fn offered(l: i32) -> bool {
    ALL.iter().any(|&(n, _, _)| n == l) && (l != crate::PREP_LAYOUT || crate::PREP_OFFERED)
}

/// The saved list made safe: known, offered layouts, each once; the
/// starting list when nothing usable is saved.
fn clean(saved: Option<Vec<i32>>, offered: impl Fn(i32) -> bool) -> Vec<i32> {
    let mut list: Vec<i32> = Vec::new();
    for l in saved.unwrap_or_default() {
        if offered(l) && !list.contains(&l) {
            list.push(l);
        }
    }
    if list.is_empty() {
        list = START.into_iter().filter(|&l| offered(l)).collect();
    }
    list
}

/// The DJ's layouts, in menu order.
pub fn chosen() -> Vec<i32> {
    let saved = settings::load()[KEY].as_array().map(|a| {
        a.iter()
            .filter_map(|v| v.as_i64())
            .map(|v| v as i32)
            .collect()
    });
    clean(saved, offered)
}

fn save(list: &[i32]) {
    settings::set(KEY, serde_json::json!(list));
}

/// The layout after `current` in `list`, back to the first after the last
/// (or the first when `current` isn't in it).
fn next_in(list: &[i32], current: i32) -> i32 {
    match list.iter().position(|&l| l == current) {
        Some(i) => list[(i + 1) % list.len()],
        None => list.first().copied().unwrap_or(current),
    }
}

/// The layout VIEW goes to from `current`.
pub fn next(current: i32) -> i32 {
    next_in(&chosen(), current)
}

/// `list` with `l` taken out, unless it is the one in use or the last one.
fn removed(list: &[i32], l: i32, current: i32) -> Vec<i32> {
    if l == current || list.len() <= 1 {
        return list.to_vec();
    }
    list.iter().copied().filter(|&x| x != l).collect()
}

/// `list` with `l` added at the end.
fn added(list: &[i32], l: i32) -> Vec<i32> {
    let mut list = list.to_vec();
    if !list.contains(&l) {
        list.push(l);
    }
    list
}

/// `list` with `l` moved `by` places (−1 up, 1 down), kept in bounds.
fn moved(list: &[i32], l: i32, by: i32) -> Vec<i32> {
    let mut list = list.to_vec();
    if let Some(i) = list.iter().position(|&x| x == l) {
        let j = (i as i32 + by).clamp(0, list.len() as i32 - 1) as usize;
        let x = list.remove(i);
        list.insert(j, x);
    }
    list
}

fn entry(l: i32) -> LayoutEntry {
    let (_, name, detail) = ALL
        .iter()
        .find(|&&(n, _, _)| n == l)
        .copied()
        .unwrap_or(ALL[2]);
    LayoutEntry {
        layout: l,
        name: name.into(),
        detail: detail.into(),
    }
}

fn model(list: impl IntoIterator<Item = i32>) -> ModelRc<LayoutEntry> {
    ModelRc::new(VecModel::from(
        list.into_iter().map(entry).collect::<Vec<_>>(),
    ))
}

/// Fills the layout menu and Settings › Appearance › Layouts from the saved list and
/// the layout showing. Call it whenever either changes.
pub fn refresh(app: &AppWindow) {
    let current = app.get_layout();
    let mine = chosen();
    let ui = app.global::<LayoutsUi>();
    ui.set_current(current);
    // The menu also shows the layout in use when it isn't one of the DJ's
    // (the tour, or a mapping that sets a layout by number), at the end.
    let mut shown = mine.clone();
    if !shown.contains(&current) && ALL.iter().any(|&(n, _, _)| n == current) {
        shown.push(current);
    }
    ui.set_menu(model(shown));
    ui.set_mine(model(mine.clone()));
    ui.set_more(model(
        ALL.iter()
            .map(|&(n, _, _)| n)
            .filter(|&n| offered(n) && !mine.contains(&n)),
    ));
}

/// Wires Settings › Appearance › Layouts and the menu's cog. `switch` changes the layout
/// as the menu does; `settings_page` is Settings › Appearance › Layouts.
pub fn install(app: &AppWindow, switch: impl Fn(i32) + 'static, settings_page: i32) {
    let ui = app.global::<LayoutsUi>();
    let weak = app.as_weak();
    ui.on_pick(move |l| {
        switch(l);
        if let Some(app) = weak.upgrade() {
            refresh(&app);
        }
    });
    let weak = app.as_weak();
    let change = move |f: &dyn Fn(&[i32], i32) -> Vec<i32>| {
        if let Some(app) = weak.upgrade() {
            save(&f(&chosen(), app.get_layout()));
            refresh(&app);
        }
    };
    let change = std::rc::Rc::new(change);
    let c = change.clone();
    ui.on_add(move |l| c(&|list, _| added(list, l)));
    let c = change.clone();
    ui.on_remove(move |l| c(&|list, current| removed(list, l, current)));
    let c = change.clone();
    ui.on_move(move |l, by| c(&|list, _| moved(list, l, by)));
    let weak = app.as_weak();
    ui.on_edit(move || {
        if let Some(app) = weak.upgrade() {
            app.global::<crate::Preferences>()
                .invoke_show(settings_page);
        }
    });
    refresh(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(_: i32) -> bool {
        true
    }

    #[test]
    fn starts_with_the_old_menu() {
        assert_eq!(clean(None, all), vec![1, 4, 0, 2]);
        assert_eq!(clean(Some(vec![]), all), vec![1, 4, 0, 2]);
    }

    #[test]
    fn drops_unknown_hidden_and_repeated() {
        assert_eq!(
            clean(Some(vec![2, 9, 2, 3, 1]), |l| (0..=4).contains(&l)
                && l != 3),
            vec![2, 1]
        );
    }

    #[test]
    fn view_steps_round() {
        assert_eq!(next_in(&[1, 4], 4), 1);
        assert_eq!(next_in(&[1, 4], 1), 4);
        assert_eq!(next_in(&[1, 4, 2], 0), 1);
        assert_eq!(next_in(&[4], 4), 4);
    }

    #[test]
    fn the_layout_in_use_stays() {
        assert_eq!(removed(&[1, 4, 0], 4, 4), vec![1, 4, 0]);
        assert_eq!(removed(&[1, 4, 0], 0, 4), vec![1, 4]);
        assert_eq!(removed(&[4], 4, 1), vec![4]);
    }

    #[test]
    fn adds_at_the_end_once() {
        assert_eq!(added(&[1, 4], 2), vec![1, 4, 2]);
        assert_eq!(added(&[1, 4], 4), vec![1, 4]);
    }

    #[test]
    fn moves_within_bounds() {
        assert_eq!(moved(&[1, 4, 0], 0, -1), vec![1, 0, 4]);
        assert_eq!(moved(&[1, 4, 0], 1, -1), vec![1, 4, 0]);
        assert_eq!(moved(&[1, 4, 0], 1, 1), vec![4, 1, 0]);
        assert_eq!(moved(&[1, 4, 0], 0, 1), vec![1, 4, 0]);
    }
}
