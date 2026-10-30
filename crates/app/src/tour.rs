//! The guided tour: on first start a setup dialog (sound, controller, music)
//! and then a lesson that mixes two demo tracks with the DJ, pointing at
//! each real control in turn and going on once the DJ has used it, on
//! screen or on a controller (both just move the controls).
//!
//! A lesson is data ([`Lesson`], [`Step`]), so other lessons, such as the
//! advanced tutorials, are more of the same. The setup dialog shows the
//! Settings pages' own rows ([`Prefs::tour_items`]), so they work as they
//! do there. Settings › Help runs the tour again, whole or a part.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use opendj_control::ControlRegistry;
use opendj_controllers::{Controllers, Port};
use opendj_engine::{scan, synth, AudioEngine, Track, NUM_DECKS};
use serde_json::json;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::prefs::Prefs;
use crate::{
    settings, short_port_name, AppWindow, ControllerEditor, MixerMode, PrefItem, Tour as TourUi,
    TourSetup,
};

/// A part of the tour, which Settings › Help can run on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Audio,
    Controller,
    Library,
    Mixing,
}

/// The parts' names, in [`Part`] order, as Help's menu lists them.
pub const PART_NAMES: [&str; 4] = ["Sound", "Controller", "Your music", "Your first mix"];

impl Part {
    const ALL: [Part; 4] = [Part::Audio, Part::Controller, Part::Library, Part::Mixing];

    fn name(self) -> &'static str {
        PART_NAMES[self as usize]
    }
}

/// What a lesson's step waits for before it goes on.
#[derive(Clone, Copy, Debug)]
pub enum Wait {
    /// The DJ presses Next.
    Next,
    /// A control's value goes over a level.
    Above(&'static str, &'static str, f64),
    /// A control's value goes under a level.
    Below(&'static str, &'static str, f64),
    /// A different track on the deck (from 0) than when the step began.
    Loaded(usize),
    Both(&'static Wait, &'static Wait),
    Either(&'static Wait, &'static Wait),
}

/// What a tip's button does for the DJ.
#[derive(Clone, Copy, Debug)]
pub enum Action {
    /// Puts demo track `demo` (from 0) on `deck`.
    LoadDemo { deck: usize, demo: usize },
}

/// One tip of a lesson.
#[derive(Debug)]
pub struct Step {
    /// The tour id of the control it points at (see TourMark in
    /// tourmark.slint), usually "group,key"; "" for none.
    pub target: &'static str,
    pub title: &'static str,
    pub body: &'static str,
    pub wait: Wait,
    pub action: Option<(&'static str, Action)>,
}

/// Tips shown one after another over the app.
#[derive(Debug)]
pub struct Lesson {
    /// The layout it needs (0 2-Deck), put back after.
    pub layout: Option<i32>,
    /// Controls set before the first tip, so it starts the same each time.
    pub prepare: &'static [(&'static str, &'static str, f64)],
    pub steps: &'static [Step],
}

const EQ_A: &str = "[EqualizerRack1_[Channel1]_Effect1]";
const EQ_B: &str = "[EqualizerRack1_[Channel2]_Effect1]";

/// The first mix: load, play and cue, then bring deck B in over deck A on
/// the headphones, with sync, the faders and a bass swap.
pub const FIRST_MIX: Lesson = Lesson {
    layout: Some(0),
    prepare: &[
        ("[Channel1]", "play", 0.0),
        ("[Channel2]", "play", 0.0),
        ("[Channel3]", "play", 0.0),
        ("[Channel4]", "play", 0.0),
        ("[Channel1]", "volume", 1.0),
        ("[Channel2]", "volume", 0.0),
        ("[Channel1]", "pfl", 0.0),
        ("[Channel2]", "pfl", 0.0),
        ("[Channel2]", "sync_enabled", 0.0),
        ("[Master]", "crossfader", 0.0),
        (EQ_A, "parameter1", 1.0),
        (EQ_B, "parameter1", 1.0),
        ("[Mixer]", "knob_mode", 0.0),
    ],
    steps: &[
        Step {
            target: "[Channel1],load",
            title: "Load a track on deck A",
            body: "Drag a track from the browser onto deck A, or load the first demo track.",
            wait: Wait::Loaded(0),
            action: Some(("Load demo track", Action::LoadDemo { deck: 0, demo: 0 })),
        },
        Step {
            target: "[Channel1],play",
            title: "Press PLAY",
            body: "Deck A starts. Its fader is up, so it plays on the speakers.",
            wait: Wait::Above("[Channel1]", "play", 0.5),
            action: None,
        },
        Step {
            target: "[Channel1],cue_default",
            title: "Press CUE",
            body: "CUE jumps back to the cue point and stops. That's the start until you set \
                   your own: pause where you want it and press CUE.",
            wait: Wait::Below("[Channel1]", "play", 0.5),
            action: None,
        },
        Step {
            target: "[Channel1],play",
            title: "Press PLAY again",
            body: "Let deck A play while you get the next track ready.",
            wait: Wait::Above("[Channel1]", "play", 0.5),
            action: None,
        },
        Step {
            target: "[Channel2],load",
            title: "Load the next track on deck B",
            body: "Deck B's fader is down, so nobody hears it yet.",
            wait: Wait::Loaded(1),
            action: Some(("Load demo track", Action::LoadDemo { deck: 1, demo: 1 })),
        },
        Step {
            target: "[Channel2],pfl",
            title: "Listen in your headphones",
            body: "Press the headphones button under deck B's fader. Deck B now plays in your \
                   headphones only, so you can get it ready.",
            wait: Wait::Above("[Channel2]", "pfl", 0.5),
            action: None,
        },
        Step {
            target: "[Channel2],sync_enabled",
            title: "Press SYNC on deck B",
            body: "SYNC matches deck B's tempo and beats to deck A, so the two play in time.",
            wait: Wait::Above("[Channel2]", "sync_enabled", 0.5),
            action: None,
        },
        Step {
            target: "[EqualizerRack1_[Channel2]_Effect1],parameter1",
            title: "Turn deck B's LOW down",
            body: "Turn its LOW knob all the way left. Without its bass, deck B's kick won't \
                   fight deck A's when it comes in.",
            wait: Wait::Below(EQ_B, "parameter1", 0.25),
            action: None,
        },
        Step {
            target: "[Channel2],play",
            title: "Start deck B",
            body: "Press PLAY on deck B. It plays in time with deck A, in your headphones only.",
            wait: Wait::Above("[Channel2]", "play", 0.5),
            action: None,
        },
        Step {
            target: "[Channel2],volume",
            title: "Bring deck B in",
            body: "Slowly push deck B's fader up. Both tracks play now, deck B without its bass.",
            wait: Wait::Above("[Channel2]", "volume", 0.85),
            action: None,
        },
        Step {
            target: "[EqualizerRack1_[Channel1]_Effect1],parameter1",
            title: "Swap the bass",
            body: "Turn deck A's LOW all the way down, then deck B's LOW back up to the middle. \
                   DJs call this a bass swap; it sounds best at the start of a phrase.",
            wait: Wait::Both(
                &Wait::Below(EQ_A, "parameter1", 0.25),
                &Wait::Above(EQ_B, "parameter1", 0.75),
            ),
            action: None,
        },
        Step {
            target: "[Channel1],volume",
            title: "Take deck A out",
            body: "Pull deck A's fader down, or slide the crossfader all the way to deck B.",
            wait: Wait::Either(
                &Wait::Below("[Channel1]", "volume", 0.1),
                &Wait::Above("[Master]", "crossfader", 0.9),
            ),
            action: None,
        },
        Step {
            target: "",
            title: "That's a mix!",
            body: "You went from one track to the next, in time and without a bump. Try it with \
                   your own music next. Settings › Help runs this tour again.",
            wait: Wait::Next,
            action: None,
        },
    ],
};

/// A page of the setup dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Welcome,
    Part(Part),
}

impl Page {
    fn name(self) -> &'static str {
        match self {
            Page::Welcome => "Welcome",
            Page::Part(p) => p.name(),
        }
    }
}

enum State {
    Off,
    Setup {
        pages: Vec<Page>,
        at: usize,
    },
    Lesson {
        lesson: &'static Lesson,
        step: usize,
        /// The decks' tracks when the step began, for [`Wait::Loaded`].
        loaded: [Option<Arc<Track>>; NUM_DECKS],
        /// When the step was done, to go on a moment later.
        done_at: Option<Instant>,
        /// The layout to put back.
        layout: i32,
    },
}

/// How long a done step shows its tick before the next one.
const DONE_PAUSE: Duration = Duration::from_millis(900);
/// How long a test tone beeps.
const TONE_TIME: Duration = Duration::from_secs(4);
/// Ids for the demo tracks, far from any in the library.
const DEMO_ID: usize = usize::MAX - 64;

pub struct Tour {
    app: slint::Weak<AppWindow>,
    registry: Arc<ControlRegistry>,
    engine: Rc<AudioEngine>,
    controllers: Option<Rc<Controllers>>,
    prefs: Rc<RefCell<Prefs>>,
    state: State,
    /// Whether this run is the whole tour, which counts as taking it.
    whole: bool,
    /// The setup dialog's rows want filling again.
    refill: bool,
    /// [`Prefs::generation`] when they were last filled.
    filled: u64,
    /// The controllers plugged in when the rows were filled.
    ports: Vec<Port>,
    /// What the controller check heard.
    heard: String,
    tone_until: Option<Instant>,
    demo: Vec<Arc<Track>>,
}

impl Tour {
    /// Sets up the tour's callbacks. `first_run` is true when openDJ has no
    /// settings yet: the tour starts. A DJ who had openDJ before it is
    /// offered it once instead.
    pub fn install(
        app: &AppWindow,
        registry: Arc<ControlRegistry>,
        engine: Rc<AudioEngine>,
        controllers: Option<Rc<Controllers>>,
        prefs: Rc<RefCell<Prefs>>,
        first_run: bool,
    ) -> Rc<RefCell<Tour>> {
        let tour = Rc::new(RefCell::new(Tour {
            app: app.as_weak(),
            registry,
            engine,
            controllers,
            prefs,
            state: State::Off,
            whole: false,
            refill: false,
            filled: 0,
            ports: Vec::new(),
            heard: String::new(),
            tone_until: None,
            demo: Vec::new(),
        }));
        let g = app.global::<TourUi>();
        let t = tour.clone();
        g.on_next(move || t.borrow_mut().next());
        let t = tour.clone();
        g.on_back(move || t.borrow_mut().back());
        let t = tour.clone();
        g.on_skip(move || t.borrow_mut().skip());
        let t = tour.clone();
        g.on_end(move || t.borrow_mut().end());
        let t = tour.clone();
        g.on_action_pressed(move || t.borrow_mut().action());
        let t = tour.clone();
        g.on_pressed(move |id| t.borrow_mut().pressed(&id));
        let t = tour.clone();
        g.on_start(move |part| {
            let part = usize::try_from(part)
                .ok()
                .and_then(|p| Part::ALL.get(p).copied());
            t.borrow_mut().start(part)
        });
        let weak = app.as_weak();
        g.on_dismiss_offer(move || {
            settings::set(TOUR_KEY, json!(TOUR_DONE));
            if let Some(app) = weak.upgrade() {
                app.global::<TourUi>().set_offer(false);
            }
        });
        if settings::string(TOUR_KEY).is_empty() {
            if first_run {
                tour.borrow_mut().start(None);
            } else {
                g.set_offer(true);
            }
        }
        tour
    }

    /// Starts the whole tour (`part` None) or one part of it.
    pub fn start(&mut self, part: Option<Part>) {
        self.stop_lesson();
        self.whole = part.is_none();
        let pages = match part {
            Some(p) => vec![Page::Part(p)],
            None => {
                // Seen now, so it doesn't come back however it ends.
                settings::set(TOUR_KEY, json!(TOUR_DONE));
                let mut pages = vec![Page::Welcome, Page::Part(Part::Audio)];
                if !self.plugged_in().is_empty() {
                    pages.push(Page::Part(Part::Controller));
                }
                pages.extend([Page::Part(Part::Library), Page::Part(Part::Mixing)]);
                pages
            }
        };
        if let Some(app) = self.app.upgrade() {
            let g = app.global::<TourUi>();
            g.set_offer(false);
            app.global::<TourSetup>()
                .set_parts(strings(&pages.iter().map(|p| p.name()).collect::<Vec<_>>()));
        }
        self.state = State::Setup { pages, at: 0 };
        self.show_page();
    }

    /// Called every frame.
    pub fn poll(&mut self) {
        if self.tone_until.is_some_and(|t| Instant::now() >= t) {
            self.tone(0);
        }
        match &self.state {
            State::Off => {}
            State::Setup { pages, at } => {
                let page = pages[*at];
                if page == Page::Part(Part::Controller) {
                    self.listen();
                    let ports = self.ports_now();
                    if ports != self.ports {
                        self.refill = true;
                    }
                }
                if self.refill || self.prefs.borrow().generation != self.filled {
                    self.fill(page);
                }
            }
            State::Lesson { .. } => self.check_step(),
        }
    }

    fn next(&mut self) {
        match &mut self.state {
            State::Off => {}
            State::Setup { pages, at } if pages[*at] == Page::Part(Part::Mixing) => {
                self.start_lesson(&FIRST_MIX)
            }
            State::Setup { pages, at } if *at + 1 < pages.len() => {
                *at += 1;
                self.show_page();
            }
            State::Lesson { lesson, step, .. } if *step + 1 < lesson.steps.len() => {
                *step += 1;
                self.show_step();
            }
            State::Setup { .. } | State::Lesson { .. } => self.end(),
        }
    }

    fn back(&mut self) {
        match &mut self.state {
            State::Off => return,
            State::Setup { at, .. } => *at = at.saturating_sub(1),
            State::Lesson { step, .. } => *step = step.saturating_sub(1),
        }
        if matches!(self.state, State::Setup { .. }) {
            self.show_page();
        } else {
            self.show_step();
        }
    }

    /// The setup dialog's quieter button: Not now on the welcome page,
    /// skipping the lesson on its page.
    fn skip(&mut self) {
        match &self.state {
            State::Setup { pages, at } if pages[*at] == Page::Welcome => self.end(),
            State::Setup { pages, at } if pages[*at] == Page::Part(Part::Mixing) => self.end(),
            _ => self.next(),
        }
    }

    fn end(&mut self) {
        self.stop_lesson();
        self.listen_off();
        self.tone(0);
        self.state = State::Off;
        if let Some(app) = self.app.upgrade() {
            app.global::<TourUi>().set_mode(0);
        }
    }

    fn action(&mut self) {
        let State::Lesson { lesson, step, .. } = &self.state else {
            return;
        };
        match lesson.steps[*step].action {
            Some((_, Action::LoadDemo { deck, demo })) => {
                if self.demo.is_empty() {
                    self.demo = demo_tracks();
                }
                if let Some(t) = self.demo.get(demo % self.demo.len().max(1)) {
                    self.engine.load_track(deck, t.clone());
                }
            }
            None => {}
        }
    }

    /// A tour button on a settings row in the dialog.
    fn pressed(&mut self, id: &str) {
        if let Some(to) = id.strip_prefix("tour_tone:").and_then(|t| t.parse().ok()) {
            self.tone(to);
        }
    }

    /// Beeps in the master (1) or the headphones (2) for a while, or stops.
    fn tone(&mut self, to: u8) {
        if let Some(c) = self.registry.get("[Master]", "test_tone") {
            c.set(to as f64);
        }
        self.tone_until = (to != 0).then(|| Instant::now() + TONE_TIME);
    }

    /// Shows the setup dialog's page.
    fn show_page(&mut self) {
        let State::Setup { pages, at } = &self.state else {
            return;
        };
        let (page, at, last) = (pages[*at], *at, *at + 1 == pages.len());
        self.listen_off();
        if page == Page::Part(Part::Controller) {
            self.heard.clear();
            if let Some(c) = &self.controllers {
                c.set_monitor(true);
            }
        }
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let names: Vec<String> = self
            .plugged_in()
            .iter()
            .map(|p| short_port_name(p).to_string())
            .collect();
        let (title, body) = page_words(page, &names);
        let s = app.global::<TourSetup>();
        s.set_part(at as i32);
        s.set_title(title.into());
        s.set_body(body.into());
        s.set_skip_label(
            match page {
                Page::Welcome => "Not now",
                Page::Part(Part::Mixing) => "Skip the lesson",
                _ => "",
            }
            .into(),
        );
        let g = app.global::<TourUi>();
        g.set_can_back(at > 0);
        g.set_next_label(
            match page {
                Page::Welcome => "Let's go",
                Page::Part(Part::Mixing) => "Start the lesson",
                _ if last => "Done",
                _ => "Next",
            }
            .into(),
        );
        g.set_mode(1);
        // Filled on the next frame: this may run from inside Settings,
        // whose rows are being handled.
        s.set_items(ModelRc::default());
        self.refill = true;
    }

    /// Fills the page's settings rows.
    fn fill(&mut self, page: Page) {
        self.refill = false;
        self.ports = self.ports_now();
        let items = match page {
            Page::Welcome => self.welcome_items(),
            Page::Part(p) => self.prefs.borrow_mut().tour_items(p, &self.heard),
        };
        self.filled = self.prefs.borrow().generation;
        if let Some(app) = self.app.upgrade() {
            app.global::<TourSetup>()
                .set_items(ModelRc::new(VecModel::from(items)));
        }
    }

    /// What the welcome page says is coming: the pages after it.
    fn welcome_items(&self) -> Vec<PrefItem> {
        let State::Setup { pages, .. } = &self.state else {
            return Vec::new();
        };
        pages
            .iter()
            .filter_map(|&p| match p {
                Page::Welcome => None,
                Page::Part(part) => Some(part),
            })
            .map(|part| PrefItem {
                kind: 6,
                title: part.name().into(),
                detail: match part {
                    Part::Audio => {
                        "Pick your speakers and headphones, and hear a test tone in each."
                    }
                    Part::Controller => "Check your controller is found and heard.",
                    Part::Library => {
                        "Add your music folders, or bring in another DJ app's library."
                    }
                    Part::Mixing => "Mix two demo tracks together, one control at a time.",
                }
                .into(),
                ..Default::default()
            })
            .collect()
    }

    /// Picks up what the controllers sent, for the controller check.
    fn listen(&mut self) {
        let Some(c) = &self.controllers else {
            return;
        };
        let seen = c.take_monitored();
        if let Some((port, _)) = seen.last() {
            let heard = format!(
                "✓ Heard your {}: it's working. Try the rest of its controls whenever you like.",
                short_port_name(port)
            );
            if heard != self.heard {
                self.heard = heard;
                self.refill = true;
            }
        }
    }

    /// Stops the controller check, unless the Controller Editor uses it.
    fn listen_off(&self) {
        let editing = self
            .app
            .upgrade()
            .is_some_and(|a| a.global::<ControllerEditor>().get_open());
        if let (Some(c), false) = (&self.controllers, editing) {
            c.set_monitor(false);
        }
    }

    fn ports_now(&self) -> Vec<Port> {
        self.controllers
            .as_ref()
            .map(|c| c.ports())
            .unwrap_or_default()
    }

    /// The MIDI devices plugged in now, by port name.
    fn plugged_in(&self) -> Vec<String> {
        self.ports_now().into_iter().map(|p| p.name).collect()
    }

    fn start_lesson(&mut self, lesson: &'static Lesson) {
        self.listen_off();
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let layout = app.get_layout();
        if let Some(l) = lesson.layout {
            self.set_layout(l);
        }
        // The decks and mixer show, with the EQ knobs.
        app.set_library_expanded(false);
        app.global::<MixerMode>().set_mode(0);
        for &(group, key, value) in lesson.prepare {
            if let Some(c) = self.registry.get(group, key) {
                c.set(value);
            }
        }
        self.state = State::Lesson {
            lesson,
            step: 0,
            loaded: Default::default(),
            done_at: None,
            layout,
        };
        app.global::<TourUi>().set_mode(2);
        self.show_step();
    }

    /// Puts back what the lesson changed, if one runs.
    fn stop_lesson(&mut self) {
        if let State::Lesson { lesson, layout, .. } = self.state {
            if lesson.layout.is_some() {
                self.set_layout(layout);
            }
            self.state = State::Off;
        }
    }

    /// Switches layout as a controller would, through `[Skin],layout`, so it
    /// is remembered the same way.
    fn set_layout(&self, layout: i32) {
        if let Some(c) = self.registry.get("[Skin]", "layout") {
            c.set(layout as f64);
        }
    }

    /// Shows the lesson's step.
    fn show_step(&mut self) {
        let tracks = self.engine.loaded_tracks().read().unwrap().clone();
        let State::Lesson {
            lesson,
            step,
            loaded,
            done_at,
            ..
        } = &mut self.state
        else {
            return;
        };
        *loaded = tracks;
        *done_at = None;
        let s = &lesson.steps[*step];
        let last = *step + 1 == lesson.steps.len();
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<TourUi>();
        g.set_placed(false);
        g.set_target(s.target.into());
        g.set_step(format!("STEP {} OF {}", *step + 1, lesson.steps.len()).into());
        g.set_title(s.title.into());
        g.set_body(s.body.into());
        g.set_action(s.action.map_or("", |(label, _)| label).into());
        g.set_waiting(!matches!(s.wait, Wait::Next));
        g.set_done(false);
        g.set_can_back(*step > 0);
        g.set_next_label(if last { "Finish" } else { "Next" }.into());
    }

    /// Goes on once the step is done, after showing so for a moment.
    fn check_step(&mut self) {
        let tracks = self.engine.loaded_tracks().read().unwrap().clone();
        let registry = self.registry.clone();
        let State::Lesson {
            lesson,
            step,
            loaded,
            done_at,
            ..
        } = &mut self.state
        else {
            return;
        };
        match *done_at {
            Some(t) if t.elapsed() >= DONE_PAUSE => {}
            Some(_) => return,
            None => {
                let value = |g: &str, k: &str| registry.get(g, k).map(|c| c.get());
                if met(&lesson.steps[*step].wait, &value, loaded, &tracks) {
                    *done_at = Some(Instant::now());
                    if let Some(app) = self.app.upgrade() {
                        app.global::<TourUi>().set_done(true);
                    }
                }
                return;
            }
        }
        self.next();
    }
}

/// The settings key that says the tour was taken or turned down.
const TOUR_KEY: &str = "tour";
const TOUR_DONE: &str = "done";

/// Whether `wait` is met, with `value` reading a control and `before` the
/// decks' tracks when the step began.
fn met(
    wait: &Wait,
    value: &dyn Fn(&str, &str) -> Option<f64>,
    before: &[Option<Arc<Track>>],
    now: &[Option<Arc<Track>>],
) -> bool {
    match *wait {
        Wait::Next => false,
        Wait::Above(g, k, level) => value(g, k).is_some_and(|v| v > level),
        Wait::Below(g, k, level) => value(g, k).is_some_and(|v| v < level),
        Wait::Loaded(deck) => match (
            before.get(deck).cloned().flatten(),
            now.get(deck).cloned().flatten(),
        ) {
            (_, None) => false,
            (None, Some(_)) => true,
            (Some(a), Some(b)) => !Arc::ptr_eq(&a, &b),
        },
        Wait::Both(a, b) => met(a, value, before, now) && met(b, value, before, now),
        Wait::Either(a, b) => met(a, value, before, now) || met(b, value, before, now),
    }
}

/// The setup dialog's title and words for a page; `controllers` are the
/// devices plugged in.
fn page_words(page: Page, controllers: &[String]) -> (String, String) {
    let (title, body) = match page {
        Page::Welcome => (
            "Welcome to openDJ",
            "Let's get you mixing. A few quick steps set up your sound, your controller and \
             your music, then a short lesson mixes two tracks with you. Every step can be \
             skipped, and Settings › Help runs the tour again whenever you like."
                .to_string(),
        ),
        Page::Part(Part::Audio) => (
            "Sound",
            "Choose where the master (your speakers) and the headphones play, then test \
             each one."
                .to_string(),
        ),
        Page::Part(Part::Controller) => (
            "Controller",
            if controllers.is_empty() {
                "No controller is plugged in. Plug one in now, or go on: openDJ finds it \
                 whenever you plug it in."
                    .to_string()
            } else {
                format!(
                    "openDJ found {}. It runs Mixxx's mapping for it by itself; change it \
                     below if you need to.",
                    controllers.join(" and ")
                )
            },
        ),
        Page::Part(Part::Library) => (
            "Your music",
            "Add the folders your music is in; openDJ reads them and keeps your collection up to \
             date. Already use another DJ app? Bring its library in, with cues, grids and \
             playlists."
                .to_string(),
        ),
        Page::Part(Part::Mixing) => (
            "Your first mix",
            "Two demo tracks are ready. The lesson points at each control in turn and waits \
             for you to use it, on screen or on your controller. It switches to the 2-Deck \
             layout while it runs and back after."
                .to_string(),
        ),
    };
    (title.to_string(), body)
}

/// Other DJ apps whose library is on this computer, by name: those openDJ
/// can't import yet are offered as coming soon.
pub fn other_dj_apps() -> Vec<&'static str> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from);
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let under = |base: &Option<PathBuf>, rel: &str| base.as_ref().map(|b| b.join(rel));
    let apps: [(&str, Vec<Option<PathBuf>>); 2] = [
        (
            "rekordbox",
            vec![
                under(&home, "Library/Pioneer/rekordbox/master.db"),
                under(&appdata, "Pioneer/rekordbox/master.db"),
            ],
        ),
        (
            "Mixxx",
            vec![
                under(
                    &home,
                    "Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx/mixxxdb.sqlite",
                ),
                under(&home, "Library/Application Support/Mixxx/mixxxdb.sqlite"),
                under(&home, ".mixxx/mixxxdb.sqlite"),
                under(&local, "Mixxx/mixxxdb.sqlite"),
            ],
        ),
    ];
    apps.into_iter()
        .filter(|(_, paths)| paths.iter().flatten().any(|p| p.is_file()))
        .map(|(name, _)| name)
        .collect()
}

/// Where the demo tracks are: next to the executable (`../Resources/demo`
/// in a macOS bundle, `demo/` beside it, or `../share/opendj/demo` in a
/// Linux install), otherwise the repository's `res/demo`.
fn demo_dir() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(PathBuf::from))
    {
        candidates.push(exe_dir.join("../Resources/demo"));
        candidates.push(exe_dir.join("demo"));
        candidates.push(exe_dir.join("../share/opendj/demo"));
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../res/demo"));
    candidates.into_iter().find(|d| d.is_dir())
}

/// The lesson's two tracks: the demo tracks shipped with openDJ, in file
/// name order, or two of the built-in synthesised ones if they're missing.
/// They aren't added to the library.
fn demo_tracks() -> Vec<Arc<Track>> {
    let files = demo_dir().map(|d| audio_files(&d)).unwrap_or_default();
    let tracks: Vec<Track> = if files.len() >= 2 {
        files
            .iter()
            .enumerate()
            .map(|(i, p)| scan::read_track(DEMO_ID + i, p))
            .collect()
    } else {
        let samples = synth::sample_library();
        [0, 4]
            .iter()
            .filter_map(|&i| samples.get(i))
            .enumerate()
            .map(|(n, t)| Track {
                id: DEMO_ID + n,
                ..(**t).clone()
            })
            .collect()
    };
    tracks.into_iter().map(Arc::new).collect()
}

fn audio_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| scan::is_audio(p))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

fn strings(v: &[&str]) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        v.iter().map(|&s| SharedString::from(s)).collect::<Vec<_>>(),
    ))
}

#[cfg(test)]
mod tests {
    use std::slice;

    use super::*;

    fn track(id: usize) -> Option<Arc<Track>> {
        synth::sample_library().get(id).cloned()
    }

    #[test]
    fn a_step_waits_for_its_controls() {
        let values = |play: f64, low: f64| {
            move |g: &str, k: &str| match (g, k) {
                ("[Channel1]", "play") => Some(play),
                (EQ_A, "parameter1") => Some(low),
                _ => None,
            }
        };
        let none: [Option<Arc<Track>>; 1] = [None];
        let play = Wait::Above("[Channel1]", "play", 0.5);
        assert!(!met(&play, &values(0.0, 1.0), &none, &none));
        assert!(met(&play, &values(1.0, 1.0), &none, &none));
        let both = Wait::Both(
            &Wait::Above("[Channel1]", "play", 0.5),
            &Wait::Below(EQ_A, "parameter1", 0.25),
        );
        assert!(!met(&both, &values(1.0, 1.0), &none, &none));
        assert!(met(&both, &values(1.0, 0.0), &none, &none));
        let either = Wait::Either(
            &Wait::Below("[Channel1]", "play", 0.5),
            &Wait::Below(EQ_A, "parameter1", 0.25),
        );
        assert!(met(&either, &values(1.0, 0.0), &none, &none));
        // A control that isn't there never counts, and Next waits for Next.
        assert!(!met(
            &Wait::Above("[Nope]", "x", 0.0),
            &values(1.0, 1.0),
            &none,
            &none
        ));
        assert!(!met(&Wait::Next, &values(1.0, 1.0), &none, &none));
    }

    #[test]
    fn loading_counts_only_a_different_track() {
        let value = |_: &str, _: &str| None;
        let (a, b) = (track(0), track(1));
        let wait = Wait::Loaded(0);
        assert!(!met(
            &wait,
            &value,
            slice::from_ref(&a),
            slice::from_ref(&a)
        ));
        assert!(met(&wait, &value, slice::from_ref(&a), slice::from_ref(&b)));
        assert!(met(&wait, &value, &[None], &[b]));
        assert!(!met(&wait, &value, &[a], &[None]));
    }

    #[test]
    fn the_first_mix_points_at_controls_that_exist() {
        let registry = ControlRegistry::new();
        opendj_engine::create_controls(&registry);
        for step in FIRST_MIX.steps {
            if let Some((group, key)) = step.target.split_once(',') {
                if key != "load" {
                    assert!(registry.get(group, key).is_some(), "{}", step.target);
                }
            }
            check(&registry, &step.wait);
        }
        for (group, key, _) in FIRST_MIX.prepare {
            assert!(registry.get(group, key).is_some(), "{group},{key}");
        }
        fn check(registry: &ControlRegistry, wait: &Wait) {
            match wait {
                Wait::Above(g, k, _) | Wait::Below(g, k, _) => {
                    assert!(registry.get(g, k).is_some(), "{g},{k}")
                }
                Wait::Both(a, b) | Wait::Either(a, b) => {
                    check(registry, a);
                    check(registry, b);
                }
                Wait::Next | Wait::Loaded(_) => {}
            }
        }
    }

    #[test]
    fn there_are_always_two_demo_tracks() {
        let tracks = demo_tracks();
        assert_eq!(tracks.len(), 2);
        assert!(tracks.iter().all(|t| t.id >= DEMO_ID));
    }
}
