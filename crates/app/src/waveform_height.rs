//! How tall the large scrolling waveforms are: four sizes, Small to Extra
//! large, with Medium the height they always had. Each layout keeps its own
//! size (a tall 2-Deck doesn't make 4-Deck taller), saved as one list by
//! layout number.
//!
//! The window works out the heights each size means in each layout, and
//! shows a smaller size while the chosen one would leave the library too
//! little room (main.slint). Here is only the choice: from the grip under
//! the waveforms, the waveforms button's menu, a waveform's right-click
//! menu, ⌥⌘↑ / ⌥⌘↓, View, and controllers through `[Skin],waveform_height`
//! (1 to 4), `waveform_height_up`, `waveform_height_down` and
//! `waveform_height_set_default`, which act on the layout showing.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use opendj_control::{Behavior, ControlHandle, ControlRegistry};
use serde_json::Value;
use slint::{ComponentHandle, Model, ModelRc, VecModel};

use crate::{settings, AppWindow, WaveHeight};

/// The sizes, 1 (Small) to `LEVELS` (Extra large).
pub const LEVELS: i32 = 4;
/// Medium: the height the waveforms had before they could change.
pub const DEFAULT_LEVEL: i32 = 2;
/// Layout numbers 0 to 4 (main.slint's `layout`); Player (1) has no large
/// waveforms but keeps its place in the list.
const LAYOUTS: usize = 5;
const SETTING: &str = "waveform_height";

fn clamp(level: i32) -> i32 {
    level.clamp(1, LEVELS)
}

/// Each layout's size from the saved list; Medium where it has none.
fn parse(saved: &Value) -> [i32; LAYOUTS] {
    let mut levels = [DEFAULT_LEVEL; LAYOUTS];
    if let Some(list) = saved.as_array() {
        for (slot, v) in levels.iter_mut().zip(list) {
            if let Some(n) = v.as_f64() {
                *slot = clamp(n.round() as i32);
            }
        }
    }
    levels
}

/// The controller controls, for `--list-controls` and controllers.
pub fn create_controls(registry: &ControlRegistry) -> [ControlHandle; 4] {
    [
        registry.create(
            "[Skin]",
            "waveform_height",
            DEFAULT_LEVEL as f64,
            1.0,
            LEVELS as f64,
            Behavior::Linear,
        ),
        registry.push("[Skin]", "waveform_height_up"),
        registry.push("[Skin]", "waveform_height_down"),
        registry.push("[Skin]", "waveform_height_set_default"),
    ]
}

thread_local! {
    /// The controls, so a change from the window shows on them.
    static HEIGHT_CONTROL: RefCell<Option<ControlHandle>> = const { RefCell::new(None) };
}

/// Reads the saved sizes, shows them, and takes changes from the window
/// and controllers. Keep the returned timer: it watches the controllers.
pub fn install(app: &AppWindow, registry: &ControlRegistry) -> slint::Timer {
    let levels = parse(&settings::load()[SETTING]);
    let ui = app.global::<WaveHeight>();
    ui.set_steps(LEVELS);
    ui.set_default_level(DEFAULT_LEVEL);
    ui.set_levels(ModelRc::from(Rc::new(VecModel::from(levels.to_vec()))));
    let weak = app.as_weak();
    ui.on_set_level(move |layout, level| {
        if let Some(app) = weak.upgrade() {
            set(&app, layout, level);
        }
    });

    let [height, up, down, reset] = create_controls(registry);
    HEIGHT_CONTROL.with(|c| *c.borrow_mut() = Some(height.clone()));
    let versions = |h: &ControlHandle, u: &ControlHandle, d: &ControlHandle, r: &ControlHandle| {
        [h.version(), u.version(), d.version(), r.version()]
    };
    // The layout the controls last showed, so a layout change shows its size.
    let shown_layout = Cell::new(-1);
    let seen = Cell::new([0u32; 4]);
    let timer = slint::Timer::default();
    let weak = app.as_weak();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(16),
        move || {
            let Some(app) = weak.upgrade() else {
                return;
            };
            let layout = app.global::<WaveHeight>().get_layout();
            if shown_layout.replace(layout) != layout {
                height.set(level(&app, layout) as f64);
                seen.set(versions(&height, &up, &down, &reset));
                return;
            }
            let now = versions(&height, &up, &down, &reset);
            let before = seen.replace(now);
            if now == before {
                return;
            }
            let current = level(&app, layout);
            let pressed = |i: usize, h: &ControlHandle| now[i] != before[i] && h.get_bool();
            let asked = if now[0] != before[0] {
                height.get().round() as i32
            } else if pressed(1, &up) {
                current + 1
            } else if pressed(2, &down) {
                current - 1
            } else if pressed(3, &reset) {
                DEFAULT_LEVEL
            } else {
                current
            };
            set(&app, layout, asked);
            // Back in range on the control if it was asked for too much.
            height.set(level(&app, layout) as f64);
            seen.set(versions(&height, &up, &down, &reset));
        },
    );
    timer
}

/// The size `layout` has now.
fn level(app: &AppWindow, layout: i32) -> i32 {
    let levels = app.global::<WaveHeight>().get_levels();
    usize::try_from(layout)
        .ok()
        .and_then(|l| levels.row_data(l))
        .unwrap_or(DEFAULT_LEVEL)
}

/// Sets `layout`'s size: shows it, saves it and tells controllers.
pub fn set(app: &AppWindow, layout: i32, level: i32) {
    let level = clamp(level);
    let Ok(index) = usize::try_from(layout) else {
        return;
    };
    if index >= LAYOUTS || self::level(app, layout) == level {
        return;
    }
    let ui = app.global::<WaveHeight>();
    let levels = ui.get_levels();
    levels.set_row_data(index, level);
    ui.set_changes(ui.get_changes().wrapping_add(1));
    let all: Vec<i32> = levels.iter().collect();
    settings::set(SETTING, serde_json::json!(all));
    if ui.get_layout() == layout {
        HEIGHT_CONTROL.with(|c| {
            if let Some(c) = c.borrow().as_ref() {
                c.set(level as f64);
            }
        });
    }
}

/// One size taller (1) or shorter (-1) in the layout showing.
pub fn step(app: &AppWindow, by: i32) {
    let layout = app.global::<WaveHeight>().get_layout();
    set(app, layout, level(app, layout) + by);
}

/// Back to Medium in the layout showing.
pub fn reset(app: &AppWindow) {
    let layout = app.global::<WaveHeight>().get_layout();
    set(app, layout, DEFAULT_LEVEL);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_saved_is_medium_everywhere() {
        assert_eq!(parse(&Value::Null), [DEFAULT_LEVEL; LAYOUTS]);
    }

    #[test]
    fn saved_sizes_come_back_by_layout() {
        let saved = serde_json::json!([4, 2, 1, 3, 3]);
        assert_eq!(parse(&saved), [4, 2, 1, 3, 3]);
    }

    #[test]
    fn odd_saved_values_are_kept_in_range() {
        let saved = serde_json::json!([9, -1, "big", 2.6]);
        assert_eq!(parse(&saved), [4, 1, DEFAULT_LEVEL, 3, DEFAULT_LEVEL]);
    }
}
