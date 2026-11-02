//! The plugin host: starts a helper process for every effect slot that has a
//! plugin loaded on a target its unit is routed to, attaches it to the audio
//! thread, and watches it.
//!
//! The audio thread never waits for a plugin (see `opendj-plugin-host`): a
//! block the helper can't deliver in time plays dry. This thread decides what
//! happens next. A helper that crashes, or misses too many blocks in a row,
//! is restarted once with its slot switched off; the second time, or when it
//! keeps using too much of the block's time, the plugin is switched off for
//! the rest of the session and its slots are emptied. Nothing turns back on
//! by itself in the middle of a set.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use opendj_control::{ControlHandle, ControlRegistry};
pub use opendj_plugin_host::{scan, scan_audio_units, Descriptor, Limits, Plugin, ScanError};
use opendj_plugin_host::{spawn, Action, Helper, Options, Policy, Status};

use crate::controls::{effect_group, effect_unit_group, EFFECTS_PER_UNIT, NUM_EFFECT_UNITS};
use crate::effects::{
    instance_indices, plugin_index, route_name, Attached, PluginLink, PluginMsg, BLOCK,
    MAX_PARAMETERS, MAX_PLUGINS, PLUGIN_BASE, TARGETS,
};

/// One plugin the slots can load, from the app's scan.
#[derive(Clone, Debug)]
pub struct PluginEntry {
    /// Stays the same between lists and sessions (format, file and id), so
    /// a slot keeps its plugin when the list changes around it.
    pub key: String,
    pub name: String,
    pub plugin: Plugin,
}

/// What happened to a plugin, for the app to tell the DJ.
#[derive(Clone, Debug, PartialEq)]
pub enum PluginEvent {
    /// It didn't start; its slot is emptied.
    Failed { name: String, error: String },
    /// It crashed or hung and was restarted; its slot is switched off.
    Restarted { name: String },
    /// It crashed again, or ran too slow: off for the rest of the session.
    Disabled { name: String, reason: String },
}

impl std::fmt::Display for PluginEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginEvent::Failed { name, error } => {
                write!(f, "{name} didn't start ({error}), so its slot is empty.")
            }
            PluginEvent::Restarted { name } => write!(
                f,
                "{name} stopped working and was restarted. The music kept playing; switch the effect on again when you want it."
            ),
            PluginEvent::Disabled { name, reason } => write!(
                f,
                "{name} is off until you restart openDJ: {reason}. The music kept playing."
            ),
        }
    }
}

#[derive(Clone, Debug)]
pub struct HostOptions {
    /// The `opendj-plugin-host` binary.
    pub helper: PathBuf,
    pub sample_rate: u32,
    /// How often the controls and the helpers are checked.
    pub poll: Duration,
    pub limits: Limits,
}

impl HostOptions {
    pub fn new(helper: PathBuf, sample_rate: u32) -> Self {
        Self {
            helper,
            sample_rate,
            poll: Duration::from_millis(50),
            limits: Limits::default(),
        }
    }
}

#[derive(Default)]
struct List {
    entries: Vec<PluginEntry>,
    version: u64,
}

/// Runs the plugin host thread; dropping it stops the thread and every helper.
pub struct PluginHost {
    list: Arc<Mutex<List>>,
    num_plugins: ControlHandle,
    events: Receiver<PluginEvent>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl PluginHost {
    pub fn start(reg: &ControlRegistry, link: PluginLink, opts: HostOptions) -> Self {
        let list = Arc::new(Mutex::new(List::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, events) = mpsc::channel();
        let mut sup = Supervisor::new(reg, link, opts, list.clone(), tx);
        let thread = {
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("plugin host".into())
                .spawn(move || {
                    while !stop.load(Ordering::Relaxed) {
                        sup.tick();
                        std::thread::sleep(sup.opts.poll);
                    }
                    sup.shut_down();
                })
                .expect("the plugin host thread starts")
        };
        Self {
            list,
            num_plugins: reg.indicator("[Master]", "num_plugins", MAX_PLUGINS as f64),
            events,
            stop,
            thread: Some(thread),
        }
    }

    /// The plugins the slots can load: `loaded_effect` [`PLUGIN_BASE`] + i
    /// is `entries[i]`. With a new list, slots keep their plugin (by key)
    /// at its new place, slots whose plugin left are emptied, and every
    /// plugin gets a fresh start.
    pub fn set_plugins(&self, mut entries: Vec<PluginEntry>) {
        entries.truncate(MAX_PLUGINS);
        let n = entries.len();
        let mut list = self.list.lock().unwrap();
        list.entries = entries;
        list.version += 1;
        self.num_plugins.set(n as f64);
    }

    /// What happened since the last call.
    pub fn events(&self) -> Vec<PluginEvent> {
        self.events.try_iter().collect()
    }
}

impl Drop for PluginHost {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct SlotHandles {
    loaded_effect: ControlHandle,
    enabled: ControlHandle,
    params: [ControlHandle; MAX_PARAMETERS],
    num_parameters: ControlHandle,
}

/// A helper running for one instance.
struct Running {
    plugin: u16,
    helper: Helper,
    unit: usize,
    slot: usize,
}

struct Supervisor {
    opts: HostOptions,
    list: Arc<Mutex<List>>,
    version: u64,
    entries: Vec<PluginEntry>,
    events: Sender<PluginEvent>,
    link: PluginLink,
    slots: Vec<[SlotHandles; EFFECTS_PER_UNIT]>,
    routes: Vec<[ControlHandle; TARGETS]>,
    index: crate::effects::Indices,
    running: Vec<Option<Running>>,
    /// The plugin whose own values each slot's knobs were set to. A plugin
    /// newly chosen gets its knobs from the plugin; a restarted one, or one
    /// on another target, keeps them as the DJ left them.
    knobs: Vec<[Option<u16>; EFFECTS_PER_UNIT]>,
    /// Per plugin, this session.
    policies: HashMap<u16, Policy>,
    disabled: HashSet<u16>,
}

fn get(reg: &ControlRegistry, group: &str, item: &str) -> ControlHandle {
    reg.get(group, item)
        .unwrap_or_else(|| panic!("{group},{item} exists: the rack makes it"))
}

impl Supervisor {
    fn new(
        reg: &ControlRegistry,
        link: PluginLink,
        opts: HostOptions,
        list: Arc<Mutex<List>>,
        events: Sender<PluginEvent>,
    ) -> Self {
        let (index, n) = instance_indices();
        let slots = (0..NUM_EFFECT_UNITS)
            .map(|u| {
                std::array::from_fn(|s| {
                    let g = effect_group(u, s);
                    SlotHandles {
                        loaded_effect: get(reg, &g, "loaded_effect"),
                        enabled: get(reg, &g, "enabled"),
                        params: std::array::from_fn(|k| {
                            get(reg, &g, &format!("parameter{}", k + 1))
                        }),
                        num_parameters: get(reg, &g, "num_parameters"),
                    }
                })
            })
            .collect();
        let routes = (0..NUM_EFFECT_UNITS)
            .map(|u| {
                let g = effect_unit_group(u);
                std::array::from_fn(|t| get(reg, &g, &route_name(t)))
            })
            .collect();
        Self {
            opts,
            list,
            version: 0,
            entries: Vec::new(),
            events,
            link,
            slots,
            routes,
            index,
            running: (0..n).map(|_| None).collect(),
            knobs: vec![[None; EFFECTS_PER_UNIT]; NUM_EFFECT_UNITS],
            policies: HashMap::new(),
            disabled: HashSet::new(),
        }
    }

    fn tick(&mut self) {
        // Helpers the audio thread let go of end here, off the audio thread.
        while self.link.from_engine.pop().is_ok() {}
        self.take_list();
        self.watch();
        self.follow_slots();
    }

    fn take_list(&mut self) {
        let list = self.list.lock().unwrap();
        if list.version == self.version {
            return;
        }
        self.version = list.version;
        let old = std::mem::replace(&mut self.entries, list.entries.clone());
        drop(list);
        self.policies.clear();
        self.disabled.clear();
        self.knobs = vec![[None; EFFECTS_PER_UNIT]; NUM_EFFECT_UNITS];
        // Every helper may now run the wrong plugin: start again.
        for i in 0..self.running.len() {
            self.stop(i);
        }
        for unit in &self.slots {
            for slot in unit {
                let Some(p) = plugin_index(slot.loaded_effect.get()) else {
                    continue;
                };
                // The first time, slots restored from the last session already
                // point into this list.
                let moved = if self.version == 1 && old.is_empty() {
                    (p < self.entries.len()).then_some(p)
                } else {
                    old.get(p)
                        .and_then(|e| self.entries.iter().position(|n| n.key == e.key))
                };
                slot.loaded_effect
                    .set(moved.map_or(0.0, |n| (PLUGIN_BASE + n) as f64));
            }
        }
    }

    /// Checks each helper: crashed, hung or too slow.
    fn watch(&mut self) {
        for i in 0..self.running.len() {
            let Some(r) = self.running[i].as_mut() else {
                continue;
            };
            let plugin = r.plugin;
            let policy = self
                .policies
                .entry(plugin)
                .or_insert_with(|| Policy::new(self.opts.limits));
            let crashed = r.helper.status() != Status::Running || r.helper.is_gone();
            let missed = r.helper.missed_in_row();
            let action = if crashed {
                policy.on_crash()
            } else {
                match policy.on_missed(missed) {
                    Action::Keep => policy.on_load(r.helper.load()),
                    a => a,
                }
            };
            let (unit, slot) = (r.unit, r.slot);
            let name = self.name(plugin);
            match action {
                Action::Keep => {}
                Action::Restart => {
                    // The slot comes back off; the next pass starts a new helper.
                    self.slots[unit][slot].enabled.set_bool(false);
                    self.stop(i);
                    let _ = self.events.send(PluginEvent::Restarted { name });
                }
                Action::Disable => {
                    let reason = if crashed {
                        "it crashed again"
                    } else if missed > 0 {
                        "it stopped responding again"
                    } else {
                        "it used too much of the audio time"
                    };
                    self.disable(plugin);
                    let _ = self.events.send(PluginEvent::Disabled {
                        name,
                        reason: reason.into(),
                    });
                }
            }
        }
    }

    /// Starts helpers for slots that want one and stops the rest.
    fn follow_slots(&mut self) {
        for u in 0..NUM_EFFECT_UNITS {
            for s in 0..EFFECTS_PER_UNIT {
                let want = plugin_index(self.slots[u][s].loaded_effect.get())
                    .filter(|&p| p < self.entries.len())
                    .map(|p| p as u16)
                    .filter(|p| !self.disabled.contains(p));
                if want != self.knobs[u][s] {
                    self.knobs[u][s] = None;
                }
                for t in 0..TARGETS {
                    let Some(i) = self.index[u][s][t] else {
                        continue;
                    };
                    let i = i as usize;
                    let want = want.filter(|_| self.routes[u][t].get_bool());
                    let have = self.running[i].as_ref().map(|r| r.plugin);
                    if have == want {
                        continue;
                    }
                    if have.is_some() {
                        self.stop(i);
                    }
                    if let Some(p) = want {
                        self.start(i, u, s, p);
                    }
                }
            }
        }
    }

    fn start(&mut self, i: usize, unit: usize, slot: usize, plugin: u16) {
        let entry = &self.entries[plugin as usize];
        let opts = Options {
            helper: self.opts.helper.clone(),
            plugin: entry.plugin.clone(),
            sample_rate: self.opts.sample_rate,
            max_frames: BLOCK,
        };
        match spawn(&opts) {
            Ok((conn, helper)) => {
                let info = helper.info();
                let slot_h = &self.slots[unit][slot];
                let shown = info.params.len().min(MAX_PARAMETERS);
                let fresh = self.knobs[unit][slot] != Some(plugin);
                let mut ranges = Vec::with_capacity(shown);
                let mut sent = [0.5; MAX_PARAMETERS];
                for (k, p) in info.params.iter().take(shown).enumerate() {
                    let span = p.max - p.min;
                    let v = if span > 0.0 {
                        ((p.value - p.min) / span).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    if fresh {
                        slot_h.params[k].set(v);
                    }
                    sent[k] = slot_h.params[k].get() as f32;
                    conn.set_param(k, (p.min + sent[k] as f64 * span) as f32);
                    ranges.push((p.min as f32, p.max as f32));
                }
                slot_h.num_parameters.set(shown as f64);
                let attached = Box::new(Attached {
                    conn,
                    plugin,
                    ranges,
                    sent,
                });
                if self
                    .link
                    .to_engine
                    .push(PluginMsg::Attach(i as u16, attached))
                    .is_ok()
                {
                    self.knobs[unit][slot] = Some(plugin);
                    self.running[i] = Some(Running {
                        plugin,
                        helper,
                        unit,
                        slot,
                    });
                }
            }
            Err(e) => {
                let name = entry.name.clone();
                self.disable(plugin);
                let _ = self.events.send(PluginEvent::Failed {
                    name,
                    error: e.to_string(),
                });
            }
        }
    }

    /// Detaches instance `i`'s helper and ends it.
    fn stop(&mut self, i: usize) {
        if self.running[i].take().is_some() {
            // The engine hands the connection back; the helper itself ends
            // as it drops here, and its connection then plays dry until then.
            let _ = self.link.to_engine.push(PluginMsg::Detach(i as u16));
        }
    }

    /// Off for the session: stops its helpers and empties its slots.
    fn disable(&mut self, plugin: u16) {
        self.disabled.insert(plugin);
        for i in 0..self.running.len() {
            if self.running[i].as_ref().is_some_and(|r| r.plugin == plugin) {
                self.stop(i);
            }
        }
        let loaded = (PLUGIN_BASE + plugin as usize) as f64;
        for unit in &self.slots {
            for slot in unit {
                if slot.loaded_effect.get().round() == loaded {
                    slot.enabled.set_bool(false);
                    slot.loaded_effect.set(0.0);
                }
            }
        }
    }

    fn name(&self, plugin: u16) -> String {
        self.entries
            .get(plugin as usize)
            .map_or_else(String::new, |e| e.name.clone())
    }

    fn shut_down(&mut self) {
        for i in 0..self.running.len() {
            self.stop(i);
        }
        while self.link.from_engine.pop().is_ok() {}
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::OnceLock;
    use std::time::{Duration, Instant};

    use super::*;
    use crate::effects::EffectRack;

    const SR: u32 = 48_000;
    const FRAMES: usize = 480;

    /// Builds the helper once, next to this test binary.
    fn helper() -> PathBuf {
        static PATH: OnceLock<PathBuf> = OnceLock::new();
        PATH.get_or_init(|| {
            let exe = std::env::current_exe().unwrap();
            // target/<profile>/deps/<test>
            let profile = exe.parent().unwrap().parent().unwrap();
            let target = profile.parent().unwrap();
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
            let ok = std::process::Command::new(cargo)
                .args(["build", "-q", "-p", "opendj-plugin-host", "--bin"])
                .arg("opendj-plugin-host")
                .arg("--target-dir")
                .arg(target)
                .status()
                .expect("cargo runs")
                .success();
            assert!(ok, "the helper builds");
            let name = format!("opendj-plugin-host{}", std::env::consts::EXE_SUFFIX);
            target.join("debug").join(name)
        })
        .clone()
    }

    struct Rig {
        reg: Arc<ControlRegistry>,
        rack: EffectRack,
        host: PluginHost,
        now: f64,
    }

    impl Rig {
        /// Deck 1's own unit, its first slot loaded with `effect`, fully wet.
        fn new(effect: &str) -> Self {
            let reg = ControlRegistry::new();
            let mut rack = EffectRack::new(&reg, SR as f64);
            let link = rack.take_plugin_link().unwrap();
            let mut opts = HostOptions::new(helper(), SR);
            opts.poll = Duration::from_millis(2);
            let host = PluginHost::start(&reg, link, opts);
            host.set_plugins(vec![PluginEntry {
                key: effect.into(),
                name: format!("Test {effect}"),
                plugin: Plugin::Test(effect.into()),
            }]);
            let rig = Self {
                reg,
                rack,
                host,
                now: 0.0,
            };
            rig.unit("mix").set(1.0);
            rig.slot("loaded_effect").set(PLUGIN_BASE as f64);
            rig.slot("enabled").set_bool(true);
            rig
        }

        fn unit(&self, item: &str) -> ControlHandle {
            self.reg.get("[EffectRack1_EffectUnit1]", item).unwrap()
        }

        fn slot(&self, item: &str) -> ControlHandle {
            self.reg
                .get("[EffectRack1_EffectUnit1_Effect1]", item)
                .unwrap()
        }

        /// One buffer of deck 1: what came out for a steady 0.5 in.
        fn buffer(&mut self) -> f32 {
            self.rack.prepare([0.5; TARGETS], self.now);
            let mut buf = vec![[0.5f32, 0.5]; FRAMES];
            self.rack.process_block(0, &mut buf);
            self.rack.finish();
            self.now += FRAMES as f64 / SR as f64;
            buf[FRAMES - 1][0]
        }

        /// Plays buffers until `done` says so, for up to 10 s.
        fn until(&mut self, mut done: impl FnMut(&mut Self, f32) -> bool) {
            let end = Instant::now() + Duration::from_secs(10);
            loop {
                let out = self.buffer();
                if done(self, out) {
                    return;
                }
                assert!(Instant::now() < end, "timed out");
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    #[test]
    fn a_plugin_plays_in_an_effect_slot_and_follows_its_knob() {
        let mut rig = Rig::new("test-gain");
        // Once the helper is up the knob shows the plugin's own value (gain 1
        // of 0..2) and the deck sounds as it did.
        rig.until(|r, _| r.slot("num_parameters").get() == 1.0);
        assert_eq!(rig.slot("parameter1").get(), 0.5);
        assert_eq!(rig.slot("loaded").get(), 1.0);
        for _ in 0..5 {
            assert!((rig.buffer() - 0.5).abs() < 1e-6);
        }
        // A quarter of the way: gain 0.5.
        rig.slot("parameter1").set(0.25);
        rig.until(|_, out| (out - 0.25).abs() < 1e-6);
        // Switched off, the deck is dry again.
        rig.slot("enabled").set_bool(false);
        rig.until(|_, out| (out - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_crashing_plugin_restarts_once_then_is_switched_off() {
        let mut rig = Rig::new("test-crash");
        rig.until(|r, _| r.slot("num_parameters").get() == 1.0);
        // It crashes after 10 blocks; the deck keeps playing, dry.
        rig.slot("parameter1").set(0.01);
        let mut events = Vec::new();
        rig.until(|r, out| {
            assert!((out - 0.5).abs() < 1e-6, "the deck plays on: {out}");
            events.extend(r.host.events());
            !events.is_empty()
        });
        assert_eq!(
            events,
            vec![PluginEvent::Restarted {
                name: "Test test-crash".into()
            }]
        );
        assert!(!rig.slot("enabled").get_bool(), "back, but off");
        // Turned on again it keeps the DJ's setting (the plugin's own is
        // "never") and crashes again: off for the session.
        rig.slot("enabled").set_bool(true);
        events.clear();
        rig.until(|r, out| {
            assert!((out - 0.5).abs() < 1e-6, "the deck plays on: {out}");
            events.extend(r.host.events());
            !events.is_empty()
        });
        assert!(
            matches!(&events[0], PluginEvent::Disabled { name, .. } if name == "Test test-crash"),
            "{events:?}"
        );
        assert_eq!(rig.slot("loaded_effect").get(), 0.0, "the slot is emptied");
    }

    #[test]
    fn a_hung_plugin_plays_dry_and_is_restarted() {
        let mut rig = Rig::new("test-hang");
        rig.until(|r, _| r.slot("num_parameters").get() == 1.0);
        rig.slot("parameter1").set(1.0);
        let mut events = Vec::new();
        let start = Instant::now();
        rig.until(|r, out| {
            assert!((out - 0.5).abs() < 1e-6, "dry while it hangs: {out}");
            events.extend(r.host.events());
            !events.is_empty()
        });
        assert!(start.elapsed() < Duration::from_secs(5));
        assert_eq!(
            events,
            vec![PluginEvent::Restarted {
                name: "Test test-hang".into()
            }]
        );
    }

    #[test]
    fn a_plugin_that_fails_to_start_empties_its_slot() {
        let mut rig = Rig::new("no-such-effect");
        let mut events = Vec::new();
        rig.until(|r, _| {
            events.extend(r.host.events());
            !events.is_empty()
        });
        assert!(
            matches!(&events[0], PluginEvent::Failed { error, .. } if error.contains("unknown effect")),
            "{events:?}"
        );
        assert_eq!(rig.slot("loaded_effect").get(), 0.0);
    }

    #[test]
    fn a_slot_keeps_its_plugin_when_the_list_changes() {
        let mut rig = Rig::new("test-gain");
        rig.until(|r, _| r.slot("num_parameters").get() == 1.0);
        let entry = |key: &str| PluginEntry {
            key: key.into(),
            name: key.into(),
            plugin: Plugin::Test(key.into()),
        };
        rig.host
            .set_plugins(vec![entry("test-slow"), entry("test-gain")]);
        let second = (PLUGIN_BASE + 1) as f64;
        rig.until(|r, _| r.slot("loaded_effect").get() == second);
    }

    #[test]
    fn a_new_plugin_list_empties_slots_whose_plugin_left() {
        let mut rig = Rig::new("test-gain");
        rig.until(|r, _| r.slot("num_parameters").get() == 1.0);
        rig.host.set_plugins(Vec::new());
        assert_eq!(rig.reg.get("[Master]", "num_plugins").unwrap().get(), 0.0);
        rig.until(|r, _| r.slot("loaded_effect").get() == 0.0);
        rig.until(|_, out| (out - 0.5).abs() < 1e-6);
    }
}
