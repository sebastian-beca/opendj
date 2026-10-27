//! The sampler's sample packs and what each of its 16 pads plays.
//!
//! A pack is a folder in `<config>/samples/` with a `pack.json` naming its
//! samples, in slot order (see openspec/changes/add-sample-packs). The
//! built-in sounds are the Default DJ Sample Pack, which has no folder.
//! What the DJ changes on a pad (its sample, name, level and play mode) is
//! kept per pack in `settings.json` under `sampler`, never in the pack's
//! files, so Reset to Pack Sample just forgets it.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use mixxx_control::{ControlHandle, ControlRegistry};
use mixxx_engine::sampler::{sampler_group, Sample as Audio, NUM_SAMPLERS, SAMPLE_NAMES};
use mixxx_engine::{AudioEngine, SampleSource};
use serde_json::{json, Map, Value};
use slint::{ComponentHandle, DataTransfer, Image, ModelRc, SharedString, VecModel, Weak};

use crate::browser::{file_stem, Browser};
use crate::{settings, AppWindow, SampleChoice, SamplePack, SamplerPad, SamplerUi};

pub const DEFAULT_PACK: &str = "default";
const DEFAULT_PACK_NAME: &str = "Default DJ Sample Pack";
const PACK_FILE: &str = "pack.json";
/// How long the level knob rests before a change is saved.
const SAVE_AFTER: Duration = Duration::from_millis(600);

/// A sample on a pad or in a pack.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub name: String,
    pub source: SampleSource,
    /// `[SamplerN],pregain`: 0 silent, 1 as recorded, 2 +6 dB.
    pub level: f64,
    /// `[SamplerN],play_mode`.
    pub mode: u8,
}

impl Sample {
    fn new(name: String, source: SampleSource) -> Self {
        Sample {
            name,
            source,
            level: 1.0,
            mode: 0,
        }
    }

    fn missing(&self) -> bool {
        matches!(&self.source, SampleSource::File(p) if !p.exists())
    }
}

#[derive(Clone, Debug)]
pub struct Pack {
    pub id: String,
    pub name: String,
    pub cover: Option<PathBuf>,
    /// Its samples by slot; shorter than 16 when the last slots are empty.
    pub samples: Vec<Option<Sample>>,
}

impl Pack {
    fn builtin() -> Pack {
        Pack {
            id: DEFAULT_PACK.into(),
            name: DEFAULT_PACK_NAME.into(),
            cover: None,
            samples: SAMPLE_NAMES
                .iter()
                .enumerate()
                .map(|(n, name)| Some(Sample::new((*name).into(), SampleSource::Builtin(n))))
                .collect(),
        }
    }

    /// The pack in folder `dir`, if it has a readable `pack.json`.
    fn read(dir: &Path) -> Option<Pack> {
        let text = std::fs::read_to_string(dir.join(PACK_FILE)).ok()?;
        let v: Value = serde_json::from_str(&text).ok()?;
        let id = dir.file_name()?.to_string_lossy().into_owned();
        let samples = v["samples"]
            .as_array()
            .map(|a| {
                a.iter()
                    .take(NUM_SAMPLERS)
                    .map(|s| {
                        let file = s["file"].as_str().filter(|f| !f.is_empty())?;
                        let path = dir.join(file);
                        let name = s["name"]
                            .as_str()
                            .map_or_else(|| file_stem(&path), str::to_owned);
                        let mut sample = Sample::new(name, SampleSource::File(path));
                        apply_settings(&mut sample, s);
                        Some(sample)
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Pack {
            name: v["name"].as_str().unwrap_or(&id).to_owned(),
            cover: v["cover"]
                .as_str()
                .map(|c| dir.join(c))
                .filter(|c| c.exists()),
            id,
            samples,
        })
    }

    fn sample(&self, slot: usize) -> Option<&Sample> {
        self.samples.get(slot).and_then(Option::as_ref)
    }
}

/// Where packs live: `<config>/samples`.
pub fn packs_dir() -> Option<PathBuf> {
    settings::config_dir().map(|d| d.join("samples"))
}

/// The Default DJ Sample Pack, then the packs in `dir` by name.
pub fn load_packs(dir: Option<&Path>) -> Vec<Pack> {
    let mut packs: Vec<Pack> = dir
        .and_then(|d| std::fs::read_dir(d).ok())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| Pack::read(&e.path()))
        .collect();
    packs.sort_by_key(|p| p.name.to_lowercase());
    packs.insert(0, Pack::builtin());
    packs
}

/// Makes an empty pack called `name` in `dir`; its id is the folder name.
pub fn create_pack(dir: &Path, name: &str) -> std::io::Result<String> {
    let base: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let base = if base.is_empty() || base == DEFAULT_PACK {
        "pack".to_string()
    } else {
        base
    };
    let mut id = base.clone();
    let mut n = 2;
    while dir.join(&id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    let folder = dir.join(&id);
    std::fs::create_dir_all(&folder)?;
    let body = json!({ "name": name, "samples": [] });
    std::fs::write(
        folder.join(PACK_FILE),
        serde_json::to_string_pretty(&body).unwrap_or_default(),
    )?;
    Ok(id)
}

/// Takes `level` and `mode` from a pack.json sample or a pad's settings.
fn apply_settings(sample: &mut Sample, v: &Value) {
    if let Some(level) = v["level"].as_f64() {
        sample.level = level.clamp(0.0, 2.0);
    }
    if let Some(mode) = v["mode"].as_u64() {
        sample.mode = mode.min(3) as u8;
    }
}

/// What the DJ changed on a pad, as saved in settings: the sample (`file`,
/// `builtin`, or `empty`), `name`, `level` and `mode`.
pub fn edit_json(sample: Option<&Sample>, base: Option<&Sample>) -> Value {
    let mut m = Map::new();
    let Some(s) = sample else {
        m.insert("empty".into(), json!(true));
        return Value::Object(m);
    };
    if base.map(|b| &b.source) != Some(&s.source) {
        match &s.source {
            SampleSource::File(p) => m.insert("file".into(), json!(p.to_string_lossy())),
            SampleSource::Builtin(n) => m.insert("builtin".into(), json!(n)),
            SampleSource::Empty => m.insert("empty".into(), json!(true)),
        };
    }
    if base.map(|b| &b.name) != Some(&s.name) {
        m.insert("name".into(), json!(s.name));
    }
    if base.is_none_or(|b| (b.level - s.level).abs() > 1e-4) {
        m.insert("level".into(), json!((s.level * 1000.0).round() / 1000.0));
    }
    if base.is_none_or(|b| b.mode != s.mode) {
        m.insert("mode".into(), json!(s.mode));
    }
    Value::Object(m)
}

/// A pad: its pack's sample with the DJ's changes (`edit`) on top.
pub fn resolve(base: Option<&Sample>, edit: Option<&Value>) -> Option<Sample> {
    let Some(e) = edit else {
        return base.cloned();
    };
    if e["empty"].as_bool() == Some(true) {
        return None;
    }
    let source = if let Some(f) = e["file"].as_str() {
        Some(SampleSource::File(PathBuf::from(f)))
    } else {
        e["builtin"]
            .as_u64()
            .map(|n| SampleSource::Builtin(n as usize))
    };
    let mut s = match (source, base) {
        (Some(src), Some(b)) => Sample {
            source: src,
            ..b.clone()
        },
        (Some(src), None) => {
            let name = match &src {
                SampleSource::File(p) => file_stem(p),
                SampleSource::Builtin(n) => SAMPLE_NAMES.get(*n).copied().unwrap_or("").into(),
                SampleSource::Empty => String::new(),
            };
            Sample::new(name, src)
        }
        (None, Some(b)) => b.clone(),
        (None, None) => return None,
    };
    if let Some(name) = e["name"].as_str() {
        s.name = name.to_owned();
    }
    apply_settings(&mut s, e);
    Some(s)
}

/// "0:00.7", or "0:12" from ten seconds on.
pub fn length_text(seconds: f64) -> String {
    let m = (seconds / 60.0).floor();
    let s = seconds - m * 60.0;
    if seconds < 10.0 {
        format!("{m}:{s:04.1}")
    } else {
        format!("{m}:{:02}", s.floor() as u32)
    }
}

/// How long a sample is, read from its file's header.
fn sample_seconds(source: &SampleSource) -> Option<f64> {
    match source {
        SampleSource::Builtin(n) => Some(Audio::builtin(*n, 48_000.0).seconds()),
        SampleSource::File(p) => {
            let d = mixxx_engine::decode::Decoder::open(p).ok()?;
            let a = d.audio();
            (d.length_known() && a.sample_rate > 0).then(|| a.len() as f64 / a.sample_rate as f64)
        }
        SampleSource::Empty => None,
    }
}

/// A sample finished loading onto a slot (or didn't).
struct Loaded {
    slot: usize,
    sample: Option<Sample>,
    /// A change the DJ made, kept only if the sample loads.
    save: bool,
    result: Result<f64, String>,
}

struct Slot {
    pregain: ControlHandle,
    mode: ControlHandle,
    load_selected: ControlHandle,
    eject: ControlHandle,
}

pub struct Sampler {
    app: Weak<AppWindow>,
    engine: Rc<AudioEngine>,
    browser: Rc<RefCell<Browser>>,
    dir: Option<PathBuf>,
    packs: Vec<Pack>,
    pack: usize,
    /// What each slot plays.
    pads: Vec<Option<Sample>>,
    controls: Vec<Slot>,
    /// The pack listed in Choose Sample, and its samples' slots by row.
    choose_pack: usize,
    choose_rows: Vec<usize>,
    loaded_tx: Sender<Loaded>,
    loaded_rx: Receiver<Loaded>,
    picked_tx: Sender<(usize, PathBuf)>,
    picked_rx: Receiver<(usize, PathBuf)>,
    /// A level or mode changed and isn't saved yet: when it last changed.
    unsaved: Option<Instant>,
}

impl Sampler {
    pub fn install(
        app: &AppWindow,
        engine: Rc<AudioEngine>,
        browser: Rc<RefCell<Browser>>,
        registry: &ControlRegistry,
    ) -> Rc<RefCell<Sampler>> {
        let dir = packs_dir();
        let packs = load_packs(dir.as_deref());
        let chosen = settings::load()["sampler"]["pack"]
            .as_str()
            .unwrap_or(DEFAULT_PACK)
            .to_owned();
        let pack = packs.iter().position(|p| p.id == chosen).unwrap_or(0);
        let controls = (0..NUM_SAMPLERS)
            .map(|n| {
                let g = sampler_group(n);
                let get = |key: &str| {
                    registry
                        .get(&g, key)
                        .unwrap_or_else(|| registry.free(&g, key))
                };
                Slot {
                    pregain: get("pregain"),
                    mode: get("play_mode"),
                    load_selected: get("LoadSelectedTrack"),
                    eject: get("eject"),
                }
            })
            .collect();
        let (loaded_tx, loaded_rx) = mpsc::channel();
        let (picked_tx, picked_rx) = mpsc::channel();
        let s = Rc::new(RefCell::new(Sampler {
            app: app.as_weak(),
            engine,
            browser,
            dir,
            packs,
            pack,
            pads: vec![None; NUM_SAMPLERS],
            controls,
            choose_pack: pack,
            choose_rows: Vec::new(),
            loaded_tx,
            loaded_rx,
            picked_tx,
            picked_rx,
            unsaved: None,
        }));
        s.borrow_mut().apply_pack();
        let g = app.global::<SamplerUi>();
        let with = {
            let s = s.clone();
            move |f: Box<dyn FnOnce(&mut Sampler)>| f(&mut s.borrow_mut())
        };
        let w = with.clone();
        g.on_pick_pack(move |i| w(Box::new(move |s| s.pick_pack(i.max(0) as usize))));
        let w = with.clone();
        g.on_new_pack(move |name| w(Box::new(move |s| s.new_pack(&name))));
        let w = with.clone();
        g.on_browse(move |i| w(Box::new(move |s| s.browse(i.max(0) as usize))));
        let w = with.clone();
        g.on_choose(move |slot, row| w(Box::new(move |s| s.choose(slot as usize, row as usize))));
        let w = with.clone();
        g.on_preview(move |row| w(Box::new(move |s| s.preview(row as usize))));
        let w = with.clone();
        g.on_load_file(move |slot| w(Box::new(move |s| s.pick_file(slot as usize))));
        let w = with.clone();
        g.on_rename(move |slot, name| w(Box::new(move |s| s.rename(slot as usize, &name))));
        let w = with.clone();
        g.on_reset(move |slot| w(Box::new(move |s| s.reset(slot as usize))));
        let w = with.clone();
        g.on_clear(move |slot| w(Box::new(move |s| s.put(slot as usize, None))));
        g.on_accepts(|data| Browser::accepts_sample(&data));
        let w = with.clone();
        g.on_drop(move |slot, data| w(Box::new(move |s| s.dropped(slot as usize, &data))));
        s
    }

    /// Answers loads and file dialogs, controllers' load and eject, and
    /// saves level and mode changes once they settle.
    pub fn poll(&mut self) {
        while let Ok(l) = self.loaded_rx.try_recv() {
            self.finish(l);
        }
        while let Ok((slot, path)) = self.picked_rx.try_recv() {
            let name = file_stem(&path);
            self.put(slot, Some(Sample::new(name, SampleSource::File(path))));
        }
        let app = self.app.upgrade();
        for slot in 0..NUM_SAMPLERS {
            let c = &self.controls[slot];
            if c.load_selected.get_bool() {
                c.load_selected.set(0.0);
                let picked = app
                    .as_ref()
                    .and_then(|a| self.browser.borrow().selected_sample(a));
                if let Some((path, name)) = picked {
                    self.put(slot, Some(Sample::new(name, SampleSource::File(path))));
                }
            }
            let c = &self.controls[slot];
            if c.eject.get_bool() {
                c.eject.set(0.0);
                self.put(slot, None);
            }
            let c = &self.controls[slot];
            let (level, mode) = (c.pregain.get(), c.mode.get().round().clamp(0.0, 3.0) as u8);
            if let Some(p) = self.pads[slot].as_mut() {
                if (p.level - level).abs() > 1e-4 || p.mode != mode {
                    p.level = level;
                    p.mode = mode;
                    self.unsaved = Some(Instant::now());
                }
            }
        }
        if self.unsaved.is_some_and(|t| t.elapsed() >= SAVE_AFTER) {
            self.unsaved = None;
            self.save();
        }
    }

    fn pack(&self) -> &Pack {
        &self.packs[self.pack]
    }

    /// The DJ's changes to the chosen pack, by slot.
    fn edits(&self) -> BTreeMap<usize, Value> {
        let v = settings::load();
        v["sampler"]["packs"][&self.pack().id]
            .as_object()
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| {
                        Some((k.parse::<usize>().ok()?.checked_sub(1)?, v.clone()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Puts the chosen pack, with the DJ's changes, on all 16 slots.
    fn apply_pack(&mut self) {
        let edits = self.edits();
        for slot in 0..NUM_SAMPLERS {
            let sample = resolve(self.pack().sample(slot), edits.get(&slot));
            self.load(slot, sample, false);
        }
        self.refresh();
    }

    /// Sends `sample` to the engine; [`Sampler::finish`] takes it when it's
    /// there. A missing file shows on the pad and plays nothing.
    fn load(&mut self, slot: usize, sample: Option<Sample>, save: bool) {
        let source = match &sample {
            Some(s) if !s.missing() => s.source.clone(),
            _ => SampleSource::Empty,
        };
        if !save {
            // The pad shows it straight away; a file that won't load says so.
            self.set_pad(slot, sample.clone());
        }
        let tx = self.loaded_tx.clone();
        self.engine.set_sample(slot, source, move |result| {
            let _ = tx.send(Loaded {
                slot,
                sample,
                save,
                result,
            });
        });
    }

    fn finish(&mut self, l: Loaded) {
        match l.result {
            Ok(_) if l.save => {
                self.set_pad(l.slot, l.sample);
                self.save();
                self.refresh();
            }
            Ok(_) => {}
            Err(e) => self.engine.alert(e),
        }
    }

    /// What slot `slot` plays, with its level and mode on its controls.
    fn set_pad(&mut self, slot: usize, sample: Option<Sample>) {
        if let Some(s) = &sample {
            self.controls[slot].pregain.set(s.level);
            self.controls[slot].mode.set(s.mode as f64);
        }
        self.pads[slot] = sample;
    }

    /// Puts `sample` on `slot` as the DJ's change, once it has loaded.
    fn put(&mut self, slot: usize, sample: Option<Sample>) {
        if slot >= NUM_SAMPLERS {
            return;
        }
        // A new sample keeps the pad's mode; its level starts as recorded.
        let sample = sample.map(|mut s| {
            if let Some(old) = &self.pads[slot] {
                if s.mode == 0 {
                    s.mode = old.mode;
                }
            }
            s
        });
        self.load(slot, sample, true);
    }

    /// Saves the chosen pack and every pad that differs from it.
    fn save(&self) {
        let pack = self.pack();
        let mut slots = Map::new();
        for (slot, pad) in self.pads.iter().enumerate() {
            let base = pack.sample(slot);
            if pad.as_ref() != base {
                slots.insert((slot + 1).to_string(), edit_json(pad.as_ref(), base));
            }
        }
        let mut all = settings::load()["sampler"].clone();
        if !all.is_object() {
            all = json!({});
        }
        all["pack"] = json!(pack.id);
        if !all["packs"].is_object() {
            all["packs"] = json!({});
        }
        if slots.is_empty() {
            if let Some(m) = all["packs"].as_object_mut() {
                m.remove(&pack.id);
            }
        } else {
            all["packs"][&pack.id] = Value::Object(slots);
        }
        settings::set("sampler", all);
    }

    /// Saves which pack is chosen, before its pads are on the slots.
    fn save_choice(&self) {
        let mut all = settings::load()["sampler"].clone();
        if !all.is_object() {
            all = json!({});
        }
        all["pack"] = json!(self.pack().id);
        settings::set("sampler", all);
    }

    fn pick_pack(&mut self, index: usize) {
        if index >= self.packs.len() || index == self.pack {
            return;
        }
        self.pack = index;
        self.save_choice();
        self.apply_pack();
    }

    fn new_pack(&mut self, name: &str) {
        let Some(dir) = self.dir.clone() else {
            return;
        };
        match create_pack(&dir, name.trim()) {
            Ok(id) => {
                self.packs = load_packs(Some(&dir));
                self.pack = self.packs.iter().position(|p| p.id == id).unwrap_or(0);
                self.save_choice();
                self.apply_pack();
            }
            Err(e) => self.engine.alert(format!("Couldn't make the pack: {e}")),
        }
    }

    /// Lists pack `index` in Choose Sample.
    fn browse(&mut self, index: usize) {
        let index = index.min(self.packs.len() - 1);
        self.choose_pack = index;
        let pack = &self.packs[index];
        self.choose_rows = (0..pack.samples.len())
            .filter(|&i| pack.sample(i).is_some())
            .collect();
        let rows: Vec<SampleChoice> = self
            .choose_rows
            .iter()
            .filter_map(|&i| pack.sample(i))
            .map(|s| SampleChoice {
                name: s.name.clone().into(),
                length: sample_seconds(&s.source)
                    .map(length_text)
                    .unwrap_or_default()
                    .into(),
            })
            .collect();
        if let Some(app) = self.app.upgrade() {
            let g = app.global::<SamplerUi>();
            g.set_choose_pack(index as i32);
            g.set_choices(ModelRc::new(VecModel::from(rows)));
        }
    }

    fn chosen(&self, row: usize) -> Option<Sample> {
        let i = *self.choose_rows.get(row)?;
        self.packs.get(self.choose_pack)?.sample(i).cloned()
    }

    fn choose(&mut self, slot: usize, row: usize) {
        if let Some(s) = self.chosen(row) {
            self.load(slot, Some(s), true);
        }
    }

    fn preview(&self, row: usize) {
        if let Some(s) = self.chosen(row) {
            self.engine.preview_sample(s.source);
        }
    }

    fn pick_file(&self, slot: usize) {
        let tx = self.picked_tx.clone();
        crate::prefs::choose_file("Choose a sample", move |path| {
            let _ = tx.send((slot, path));
        });
    }

    fn rename(&mut self, slot: usize, name: &str) {
        let name = name.trim();
        if let Some(p) = self.pads.get_mut(slot).and_then(Option::as_mut) {
            if !name.is_empty() && p.name != name {
                p.name = name.to_owned();
                self.save();
                self.refresh();
            }
        }
    }

    fn reset(&mut self, slot: usize) {
        let base = self.pack().sample(slot).cloned();
        self.load(slot, base, true);
    }

    fn dropped(&mut self, slot: usize, data: &DataTransfer) {
        let found = self.browser.borrow().dropped_sample(data);
        if let Some((path, name)) = found {
            self.put(slot, Some(Sample::new(name, SampleSource::File(path))));
        }
    }

    /// Shows the packs and pads.
    fn refresh(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<SamplerUi>();
        let packs: Vec<SamplePack> = self
            .packs
            .iter()
            .map(|p| {
                let cover = p
                    .cover
                    .as_deref()
                    .and_then(|c| Image::load_from_path(c).ok());
                SamplePack {
                    name: p.name.clone().into(),
                    has_cover: cover.is_some(),
                    cover: cover.unwrap_or_default(),
                }
            })
            .collect();
        g.set_packs(ModelRc::new(VecModel::from(packs)));
        g.set_pack(self.pack as i32);
        let pads: Vec<SamplerPad> = self
            .pads
            .iter()
            .map(|p| SamplerPad {
                name: p
                    .as_ref()
                    .map(|s| SharedString::from(s.name.as_str()))
                    .unwrap_or_default(),
                empty: p.is_none(),
                missing: p.as_ref().is_some_and(Sample::missing),
            })
            .collect();
        g.set_pads(ModelRc::new(VecModel::from(pads)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kick() -> Sample {
        Sample::new("Kick".into(), SampleSource::Builtin(0))
    }

    #[test]
    fn default_pack_has_the_built_in_sounds() {
        let packs = load_packs(None);
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].name, "Default DJ Sample Pack");
        assert_eq!(packs[0].sample(4).unwrap().name, "Horn");
        assert!(packs[0].sample(8).is_none());
    }

    #[test]
    fn edits_round_trip() {
        let base = kick();
        let mut mine = Sample::new(
            "Pull Up".into(),
            SampleSource::File("/x/pull-up.wav".into()),
        );
        mine.level = 0.5;
        mine.mode = 1;
        let e = edit_json(Some(&mine), Some(&base));
        assert_eq!(e["file"], "/x/pull-up.wav");
        assert_eq!(resolve(Some(&base), Some(&e)), Some(mine.clone()));
        // Only the level changed: only the level is saved.
        let quieter = Sample {
            level: 0.25,
            ..base.clone()
        };
        let e = edit_json(Some(&quieter), Some(&base));
        assert_eq!(e, json!({ "level": 0.25 }));
        assert_eq!(resolve(Some(&base), Some(&e)), Some(quieter));
        // Cleared, and a sample on a slot the pack leaves empty.
        let e = edit_json(None, Some(&base));
        assert_eq!(resolve(Some(&base), Some(&e)), None);
        let e = edit_json(Some(&mine), None);
        assert_eq!(resolve(None, Some(&e)), Some(mine));
    }

    #[test]
    fn packs_are_read_and_made() {
        let dir = std::env::temp_dir().join(format!("opendj-packs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let id = create_pack(&dir, "Dancehall Drops!").unwrap();
        assert_eq!(id, "dancehall-drops");
        assert_eq!(
            create_pack(&dir, "Dancehall Drops").unwrap(),
            "dancehall-drops-2"
        );
        std::fs::write(
            dir.join(&id).join(PACK_FILE),
            r#"{ "name": "Dancehall Drops", "samples": [
                { "file": "pull-up.wav", "name": "Pull Up", "level": 0.5, "mode": 3 },
                null,
                { "file": "horn.wav" } ] }"#,
        )
        .unwrap();
        let packs = load_packs(Some(&dir));
        let p = packs.iter().find(|p| p.id == id).unwrap();
        assert_eq!(p.name, "Dancehall Drops");
        let s = p.sample(0).unwrap();
        assert_eq!((s.name.as_str(), s.level, s.mode), ("Pull Up", 0.5, 3));
        assert!(s.missing());
        assert!(p.sample(1).is_none());
        assert_eq!(p.sample(2).unwrap().name, "horn");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lengths() {
        assert_eq!(length_text(0.7), "0:00.7");
        assert_eq!(length_text(2.0), "0:02.0");
        assert_eq!(length_text(12.4), "0:12");
        assert_eq!(length_text(61.0), "1:01");
    }
}
