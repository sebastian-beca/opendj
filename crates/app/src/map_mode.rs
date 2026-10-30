//! Map mode, as in Ableton: with MIDI on, every control on screen that can
//! be mapped is tinted. Click one, then move or press a control on a
//! controller, and that control does it from then on: a row of the DJ's own
//! mapping for the controller (made, "My …", the first time), put over the
//! Mixxx mapping it's based on. Meanwhile the controllers' messages are only
//! watched, so a press teaches rather than plays. Each change can be undone.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use opendj_controllers::edit;
use opendj_controllers::format::OpenDjMapping;
use opendj_controllers::learn::{self, Heard};
use opendj_controllers::own::{self, OwnMapping};
use opendj_controllers::{Controllers, Mapping};
use slint::ComponentHandle;

use crate::{settings, short_port_name, AppWindow, ControllerEditor, MapMode};

/// How long a knob or fader is listened to before it's learned, if it
/// doesn't send enough to tell sooner.
const LISTEN: Duration = Duration::from_millis(350);

/// A change Map mode made, to undo it, or to do again keeping what the
/// control did before.
struct Step {
    based_on: String,
    id: String,
    before: OpenDjMapping,
    heard: Heard,
    control: String,
    name: String,
}

pub struct Mapper {
    app: slint::Weak<AppWindow>,
    controllers: Option<Rc<Controllers>>,
    dir: Option<PathBuf>,
    active: bool,
    armed: Option<String>,
    /// What one controller has sent since the control on screen was picked.
    heard: Option<(String, Instant, Vec<Vec<u8>>)>,
    steps: Vec<Step>,
    /// The controls bound on the controllers plugged in, for the tint.
    mapped: Rc<RefCell<HashSet<String>>>,
    tick: i32,
}

impl Mapper {
    pub fn install(app: &AppWindow, controllers: Option<Rc<Controllers>>) -> Rc<RefCell<Mapper>> {
        let mapped = Rc::new(RefCell::new(HashSet::new()));
        let m = Rc::new(RefCell::new(Mapper {
            app: app.as_weak(),
            controllers,
            dir: opendj_controllers::default_mappings_dir(),
            active: false,
            armed: None,
            heard: None,
            steps: Vec::new(),
            mapped: mapped.clone(),
            tick: 0,
        }));
        let g = app.global::<MapMode>();
        g.on_mapped(move |key, _| mapped.borrow().contains(key.as_str()));
        let e = m.clone();
        g.on_start(move || e.borrow_mut().set_active(true));
        let e = m.clone();
        g.on_done(move || e.borrow_mut().set_active(false));
        let e = m.clone();
        g.on_pick(move |key| e.borrow_mut().pick(&key));
        let e = m.clone();
        g.on_undo(move || e.borrow_mut().undo());
        let e = m.clone();
        g.on_keep_both(move || e.borrow_mut().keep_both());
        m
    }

    fn set_active(&mut self, on: bool) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<MapMode>();
        if on {
            app.global::<ControllerEditor>().set_open(false);
            self.steps.clear();
            g.set_last("".into());
            g.set_replaced("".into());
            g.set_can_undo(false);
            self.refresh_mapped();
        }
        self.active = on;
        self.armed = None;
        self.heard = None;
        g.set_armed("".into());
        g.set_active(on);
        if let Some(c) = &self.controllers {
            c.set_monitor(on);
            c.set_learning(on);
            // Nothing heard before now counts.
            c.take_monitored();
        }
        self.say_status();
    }

    /// The controllers plugged in that run a Mixxx mapping (or one of the
    /// DJ's own based on it): port and mapping file.
    fn ports(&self) -> Vec<(String, String)> {
        self.controllers
            .as_ref()
            .map(|c| {
                c.ports()
                    .into_iter()
                    .filter(|p| p.running.is_some())
                    .filter_map(|p| Some((p.name, p.mapping_file?)))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn say_status(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<MapMode>();
        let ports = self.ports();
        let device = match ports.as_slice() {
            [] => None,
            [(p, _)] => Some(short_port_name(p)),
            _ => Some("controller"),
        };
        let text = match (&device, &self.armed) {
            (None, _) => "Plug in a controller with a mapping to map its controls.".to_string(),
            (Some(d), None) => format!("Click a control on screen, then move one on the {d}."),
            (Some(d), Some(_)) => format!("Now press or move a control on the {d}."),
        };
        g.set_status(text.into());
    }

    fn pick(&mut self, key: &str) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<MapMode>();
        self.armed = if self.armed.as_deref() == Some(key) {
            None
        } else {
            Some(key.to_string())
        };
        self.heard = None;
        if let Some(c) = &self.controllers {
            c.take_monitored();
        }
        g.set_armed(self.armed.clone().unwrap_or_default().into());
        g.set_last("".into());
        g.set_replaced("".into());
        self.say_status();
    }

    /// Called every frame.
    pub fn poll(&mut self) {
        if !self.active {
            return;
        }
        let Some(c) = self.controllers.clone() else {
            return;
        };
        let seen = c.take_monitored();
        if self.armed.is_none() {
            return;
        }
        for (port, msg) in seen {
            if msg.len() < 2 || msg[0] >= 0xF0 {
                continue;
            }
            match &mut self.heard {
                Some((p, _, msgs)) if *p == port => msgs.push(msg),
                Some(_) => {}
                None if learn::starts(&msg) => {
                    self.heard = Some((port, Instant::now(), vec![msg]));
                }
                None => {}
            }
        }
        let ready = self
            .heard
            .as_ref()
            .is_some_and(|(_, at, msgs)| learn::enough(msgs) || at.elapsed() >= LISTEN);
        if ready {
            let (port, _, msgs) = self.heard.take().unwrap();
            self.learn(&port, &msgs);
        }
    }

    /// Binds what `port` sent to the control picked on screen.
    fn learn(&mut self, port: &str, msgs: &[Vec<u8>]) {
        let (Some(app), Some(control)) = (self.app.upgrade(), self.armed.clone()) else {
            return;
        };
        let g = app.global::<MapMode>();
        let Some(heard) = learn::hear(msgs) else {
            return;
        };
        let Some((_, based_on)) = self.ports().into_iter().find(|(p, _)| p == port) else {
            g.set_status(
                format!(
                    "The {} has no mapping to add to yet.",
                    short_port_name(port)
                )
                .into(),
            );
            return;
        };
        // The control's name on the controller's picture, if it has one.
        let (status, number) = (heard.midi.status, heard.midi.number);
        let name = opendj_controllers::layout::Layout::for_file(&based_on)
            .and_then(|l| l.find(status, number).map(|(p, _)| p.label.clone()))
            .unwrap_or_else(|| opendj_controllers::view::message_text(status, number));
        let Some(mine) = self.own_for(&based_on) else {
            g.set_status("Couldn't make a mapping of your own.".into());
            return;
        };
        let step = Step {
            based_on,
            id: mine.id.clone(),
            before: mine.mapping.clone(),
            heard,
            control,
            name,
        };
        self.apply(&step, false);
        self.steps.push(step);
        g.set_can_undo(true);
        self.armed = None;
        g.set_armed("".into());
    }

    /// Learns `step` into its own mapping, from how it was before.
    fn apply(&mut self, step: &Step, keep: bool) {
        let (Some(app), Some(dir)) = (self.app.upgrade(), settings::own_mappings_dir()) else {
            return;
        };
        let g = app.global::<MapMode>();
        let base = self.base(&step.based_on);
        let Some(mut mine) = own::all(&dir).into_iter().find(|m| m.id == step.id) else {
            return;
        };
        mine.mapping = step.before.clone();
        let was = learn::learn(
            &mut mine.mapping,
            base.as_ref(),
            &step.heard,
            &step.control,
            &step.name,
            keep,
        );
        if let Err(e) = own::save(&dir, &mine) {
            g.set_status(format!("Couldn't save the mapping: {e}").into());
            return;
        }
        if let Some(c) = &self.controllers {
            c.reload(&step.based_on);
        }
        let (group, key) = step.control.split_once(',').unwrap_or(("", ""));
        let f = opendj_controllers::catalog::describe(group, key);
        let what = if f.target.is_empty() {
            f.name
        } else {
            format!("{} · {}", f.name, f.target)
        };
        g.set_last(format!("{} now does {what}.", step.name).into());
        g.set_replaced(if keep { String::new() } else { was.join(", ") }.into());
        self.refresh_mapped();
    }

    fn base(&self, file: &str) -> Option<Mapping> {
        Mapping::load(&self.dir.as_ref()?.join(file)).ok()
    }

    /// The own mapping in use for the Mixxx mapping `based_on`, made and
    /// put in use first if there is none.
    fn own_for(&self, based_on: &str) -> Option<OwnMapping> {
        let dir = settings::own_mappings_dir()?;
        let all = own::all(&dir);
        if let Some(id) = settings::controller_version(based_on) {
            if let Some(m) = all.iter().find(|m| m.id == id) {
                return Some(m.clone());
            }
        }
        let name = self.base(based_on)?.name;
        let taken: Vec<String> = all.iter().map(|m| m.name.clone()).collect();
        let m = own::create(&dir, &edit::my_name(&name, &taken), based_on, None).ok()?;
        settings::set_controller_version(based_on, Some(&m.id));
        if let Some(c) = &self.controllers {
            c.set_version(based_on, Some(own::path(&dir, &m.id)));
        }
        Some(m)
    }

    fn undo(&mut self) {
        let (Some(app), Some(dir)) = (self.app.upgrade(), settings::own_mappings_dir()) else {
            return;
        };
        let g = app.global::<MapMode>();
        let Some(step) = self.steps.pop() else {
            return;
        };
        if let Some(mut mine) = own::all(&dir).into_iter().find(|m| m.id == step.id) {
            mine.mapping = step.before;
            if own::save(&dir, &mine).is_ok() {
                if let Some(c) = &self.controllers {
                    c.reload(&step.based_on);
                }
            }
        }
        g.set_last(format!("Undone: {}.", step.name).into());
        g.set_replaced("".into());
        g.set_can_undo(!self.steps.is_empty());
        self.refresh_mapped();
    }

    fn keep_both(&mut self) {
        let Some(step) = self.steps.pop() else {
            return;
        };
        self.apply(&step, true);
        self.steps.push(step);
    }

    /// Reads which controls the controllers plugged in have bound.
    fn refresh_mapped(&mut self) {
        let mut bound = HashSet::new();
        let dir = settings::own_mappings_dir();
        for (_, file) in self.ports() {
            let Some(base) = self.base(&file) else {
                continue;
            };
            let own = settings::controller_version(&file)
                .zip(dir.as_ref())
                .and_then(|(id, d)| OpenDjMapping::read(&own::path(d, &id)).ok());
            let built = match (own, &self.dir) {
                (Some(o), Some(d)) => o.build(Some(&base), d),
                _ => base,
            };
            bound.extend(learn::bound_controls(&built));
        }
        *self.mapped.borrow_mut() = bound;
        self.tick += 1;
        if let Some(app) = self.app.upgrade() {
            app.global::<MapMode>().set_tick(self.tick);
        }
    }
}
