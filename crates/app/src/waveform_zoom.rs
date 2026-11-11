//! The large waveforms' zoom: ten steps, shared by every large scrolling
//! waveform in every layout, Prep's included, so beats sit the same
//! distance apart on every deck and the zoom stays put when the layout
//! changes. Deck overviews and mini waveforms keep showing the whole track.
//!
//! The steps are where the zoom stops, not how it moves: each change glides
//! there over [`GLIDE`], the waveforms stretching as it goes. Every step
//! multiplies the width by the same [`STEP`], so each feels the same size;
//! step [`DEFAULT_LEVEL`] shows [`DEFAULT_SECONDS`], as before there was a
//! zoom.
//!
//! The mouse wheel or trackpad over a waveform, its edge bar, its right-click
//! menu, ⌘= / ⌘− / ⌘0 and View all go through [`set`] (from main.slint's
//! `WaveZoom`). Controllers use Mixxx's names on each deck's group:
//! `waveform_zoom` (1 to 10, higher is tighter), `waveform_zoom_up`,
//! `waveform_zoom_down` and `waveform_zoom_set_default`. Every deck's
//! controls set the one shared zoom.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use opendj_control::{Behavior, ControlHandle, ControlRegistry};
use slint::ComponentHandle;

use crate::{settings, AppWindow, WaveZoom, NUM_DECKS};

/// The steps, 1 (widest) to `LEVELS` (tightest).
pub const LEVELS: i32 = 10;
/// The step it starts on, and goes back to on reset.
pub const DEFAULT_LEVEL: i32 = 4;
/// Track seconds across a large waveform at [`DEFAULT_LEVEL`].
pub const DEFAULT_SECONDS: f64 = 10.0;
/// How much wider each step out is: about 34 s across at step 1, under a
/// second at step 10, tight enough to set a grid on a kick by eye.
const STEP: f64 = 1.5;
/// How long the waveforms take to stretch to a new step.
const GLIDE: Duration = Duration::from_millis(150);
const SETTING: &str = "waveform_zoom";

/// Track seconds across a large waveform at `level`, at normal speed;
/// between steps while gliding.
pub fn seconds(level: f64) -> f64 {
    DEFAULT_SECONDS * STEP.powf(DEFAULT_LEVEL as f64 - level)
}

/// The zoom on its way to a step. Gliding in steps rather than seconds keeps
/// the stretch even: each step's share of the glide changes the width by
/// the same factor.
#[derive(Clone, Copy, Debug)]
struct Glide {
    from: f64,
    to: f64,
    start: Option<Instant>,
}

impl Glide {
    fn new(level: i32) -> Self {
        Self {
            from: level as f64,
            to: level as f64,
            start: None,
        }
    }

    fn go(&mut self, level: i32, now: Instant) {
        self.from = self.at(now);
        self.to = level as f64;
        self.start = Some(now);
    }

    /// Where it is now: quick at first, settling into the step.
    fn at(&self, now: Instant) -> f64 {
        let Some(start) = self.start else {
            return self.to;
        };
        let t = now.saturating_duration_since(start).as_secs_f64() / GLIDE.as_secs_f64();
        if t >= 1.0 {
            return self.to;
        }
        let eased = 1.0 - (1.0 - t).powi(3);
        self.from + (self.to - self.from) * eased
    }

    fn gliding(&self, now: Instant) -> bool {
        self.start
            .is_some_and(|s| now.saturating_duration_since(s) < GLIDE)
    }
}

thread_local! {
    /// The zoom, read by the renderer each frame (both on the UI thread).
    static ZOOM: RefCell<Glide> = RefCell::new(Glide::new(DEFAULT_LEVEL));
    /// Each deck's controller controls, and the versions last seen.
    static CONTROLS: RefCell<Vec<DeckControls>> = const { RefCell::new(Vec::new()) };
}

/// The seconds across the large waveforms now, and whether the zoom is
/// still gliding (so the next frame should come at once).
pub fn now(now: Instant) -> (f64, bool) {
    ZOOM.with(|z| {
        let z = z.borrow();
        (seconds(z.at(now)), z.gliding(now))
    })
}

/// One deck's zoom controls.
struct DeckControls {
    zoom: ControlHandle,
    up: ControlHandle,
    down: ControlHandle,
    reset: ControlHandle,
    seen: [u32; 4],
}

impl DeckControls {
    fn versions(&self) -> [u32; 4] {
        [
            self.zoom.version(),
            self.up.version(),
            self.down.version(),
            self.reset.version(),
        ]
    }
}

/// The deck groups' zoom controls, for `--list-controls` and controllers.
pub fn create_controls(registry: &ControlRegistry) -> Vec<[ControlHandle; 4]> {
    (1..=NUM_DECKS)
        .map(|d| {
            let g = format!("[Channel{d}]");
            [
                registry.create(
                    &g,
                    "waveform_zoom",
                    DEFAULT_LEVEL as f64,
                    1.0,
                    LEVELS as f64,
                    Behavior::Linear,
                ),
                registry.push(&g, "waveform_zoom_up"),
                registry.push(&g, "waveform_zoom_down"),
                registry.push(&g, "waveform_zoom_set_default"),
            ]
        })
        .collect()
}

/// Reads the saved step, shows it, and takes changes from the window and
/// controllers. Keep the returned timer: it watches the controllers.
pub fn install(app: &AppWindow, registry: &ControlRegistry) -> slint::Timer {
    let level = clamp(settings::number(SETTING, DEFAULT_LEVEL as f64).round() as i32);
    ZOOM.with(|z| *z.borrow_mut() = Glide::new(level));
    let ui = app.global::<WaveZoom>();
    ui.set_steps(LEVELS);
    ui.set_default_level(DEFAULT_LEVEL);
    ui.set_level(level);
    ui.set_seconds(seconds(level as f64) as f32);
    let decks = create_controls(registry)
        .into_iter()
        .map(|[zoom, up, down, reset]| {
            zoom.set(level as f64);
            let mut c = DeckControls {
                zoom,
                up,
                down,
                reset,
                seen: [0; 4],
            };
            c.seen = c.versions();
            c
        })
        .collect();
    CONTROLS.with(|c| *c.borrow_mut() = decks);
    let weak = app.as_weak();
    ui.on_set_level(move |level| {
        if let Some(app) = weak.upgrade() {
            set(&app, level);
        }
    });
    let timer = slint::Timer::default();
    let weak = app.as_weak();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(16),
        move || {
            if let (Some(app), Some(level)) = (weak.upgrade(), from_controllers()) {
                set(&app, level);
            }
        },
    );
    timer
}

fn clamp(level: i32) -> i32 {
    level.clamp(1, LEVELS)
}

fn level() -> i32 {
    ZOOM.with(|z| z.borrow().to.round() as i32)
}

/// A step a controller asked for since the last look, if any.
fn from_controllers() -> Option<i32> {
    CONTROLS.with(|c| {
        let mut asked = None;
        for d in c.borrow_mut().iter_mut() {
            let now = d.versions();
            let seen = std::mem::replace(&mut d.seen, now);
            let pressed = |i: usize, h: &ControlHandle| now[i] != seen[i] && h.get_bool();
            if now[0] != seen[0] {
                asked = Some(d.zoom.get().round() as i32);
            }
            let current = asked.unwrap_or_else(level);
            if pressed(1, &d.up) {
                asked = Some(current + 1);
            } else if pressed(2, &d.down) {
                asked = Some(current - 1);
            } else if pressed(3, &d.reset) {
                asked = Some(DEFAULT_LEVEL);
            }
        }
        asked.map(clamp).filter(|l| *l != level())
    })
}

/// Zooms every large waveform to `level`: glides there, shows it, saves it
/// and tells controllers.
pub fn set(app: &AppWindow, level: i32) {
    let level = clamp(level);
    if level == self::level() {
        return;
    }
    ZOOM.with(|z| z.borrow_mut().go(level, Instant::now()));
    let ui = app.global::<WaveZoom>();
    ui.set_level(level);
    ui.set_changes(ui.get_changes().wrapping_add(1));
    settings::set(SETTING, serde_json::json!(level));
    CONTROLS.with(|c| {
        for d in c.borrow_mut().iter_mut() {
            d.zoom.set(level as f64);
            d.seen = d.versions();
        }
    });
    app.window().request_redraw();
}

/// One step in (1) or out (-1).
pub fn step(app: &AppWindow, by: i32) {
    set(app, level() + by);
}

pub fn reset(app: &AppWindow) {
    set(app, DEFAULT_LEVEL);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_step_shows_ten_seconds() {
        assert_eq!(seconds(DEFAULT_LEVEL as f64), 10.0);
    }

    #[test]
    fn each_step_in_is_the_same_factor_tighter() {
        for l in 1..LEVELS {
            let ratio = seconds(l as f64) / seconds(l as f64 + 1.0);
            assert!((ratio - STEP).abs() < 1e-9);
        }
        assert!(seconds(1.0) > 30.0);
        assert!(seconds(LEVELS as f64) < 1.0);
    }

    #[test]
    fn a_glide_reaches_its_step_and_stops() {
        let t0 = Instant::now();
        let mut g = Glide::new(4);
        g.go(6, t0);
        assert!(g.gliding(t0));
        let mid = g.at(t0 + GLIDE / 2);
        assert!(mid > 4.0 && mid < 6.0, "{mid}");
        assert_eq!(g.at(t0 + GLIDE), 6.0);
        assert!(!g.gliding(t0 + GLIDE));
    }

    #[test]
    fn a_new_step_mid_glide_starts_where_the_zoom_is() {
        let t0 = Instant::now();
        let mut g = Glide::new(4);
        g.go(5, t0);
        let t1 = t0 + GLIDE / 3;
        let there = g.at(t1);
        g.go(6, t1);
        assert_eq!(g.at(t1), there);
        assert_eq!(g.at(t1 + GLIDE), 6.0);
    }
}
