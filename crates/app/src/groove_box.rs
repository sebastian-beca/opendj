//! The app's side of the Groove Box (`opendj_engine::groove`): the kits'
//! and grooves' names for the screen, and the Groove Boxes kept across
//! restarts in `settings.json` under `groove_box`, one object per deck of
//! its controls' values.

use std::time::{Duration, Instant};

use opendj_control::{ControlHandle, ControlRegistry};
use opendj_engine::groove::{groove_group, GROOVES, ROWS};
use opendj_engine::kits;
use opendj_engine::NUM_DECKS;
use serde_json::{json, Map, Value};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::{settings, AppWindow, GrooveKit, GrooveUi};

/// The controls a Groove Box keeps, by name.
fn kept_keys() -> Vec<String> {
    let mut keys: Vec<String> = [
        "enabled",
        "kit",
        "groove",
        "bpm",
        "swing",
        "duck_deck",
        "duck_amount",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    for g in 1..=GROOVES {
        for r in 1..=ROWS {
            keys.push(format!("pattern{g}_row{r}"));
        }
    }
    for r in 1..=ROWS {
        for k in ["mute", "solo", "level", "pitch", "decay", "sound"] {
            keys.push(format!("row{r}_{k}"));
        }
    }
    keys
}

/// Shows the kits' and grooves' names.
pub fn install(app: &AppWindow) {
    let ui = app.global::<GrooveUi>();
    let kits: Vec<GrooveKit> = kits::KITS
        .iter()
        .map(|k| GrooveKit {
            name: k.name.into(),
            about: k.about.into(),
            sounds: ModelRc::new(VecModel::from(
                k.sounds
                    .iter()
                    .map(|&s| SharedString::from(s))
                    .collect::<Vec<_>>(),
            )),
        })
        .collect();
    ui.set_kits(ModelRc::new(VecModel::from(kits)));
    let grooves: Vec<SharedString> = kits::GROOVES.iter().map(|g| g.name.into()).collect();
    ui.set_grooves(ModelRc::new(VecModel::from(grooves)));
}

/// Keeps the Groove Boxes: puts back what was saved, then saves what
/// changes (looked at every couple of seconds).
pub struct Keeper {
    controls: Vec<Vec<(String, ControlHandle)>>,
    seen: Vec<Vec<f64>>,
    last_look: Instant,
}

impl Keeper {
    pub fn new(registry: &ControlRegistry) -> Self {
        let keys = kept_keys();
        let controls: Vec<Vec<(String, ControlHandle)>> = (0..NUM_DECKS)
            .map(|d| {
                let g = groove_group(d);
                keys.iter()
                    .filter_map(|k| Some((k.clone(), registry.get(&g, k)?)))
                    .collect()
            })
            .collect();
        let saved = settings::load()["groove_box"].clone();
        for (d, deck) in controls.iter().enumerate() {
            let Some(values) = saved[groove_group(d)].as_object() else {
                continue;
            };
            for (key, c) in deck {
                if let Some(v) = values.get(key).and_then(Value::as_f64) {
                    c.set(v);
                }
            }
        }
        let seen = snapshot(&controls);
        Self {
            controls,
            seen,
            last_look: Instant::now(),
        }
    }

    /// Saves the Groove Boxes if anything changed since the last look.
    pub fn tick(&mut self) {
        if self.last_look.elapsed() < Duration::from_secs(2) {
            return;
        }
        self.last_look = Instant::now();
        let now = snapshot(&self.controls);
        if now == self.seen {
            return;
        }
        self.seen = now;
        let mut all = Map::new();
        for (d, deck) in self.controls.iter().enumerate() {
            let values: Map<String, Value> = deck
                .iter()
                .map(|(k, c)| (k.clone(), json!(c.get())))
                .collect();
            all.insert(groove_group(d), Value::Object(values));
        }
        settings::set("groove_box", Value::Object(all));
    }
}

fn snapshot(controls: &[Vec<(String, ControlHandle)>]) -> Vec<Vec<f64>> {
    controls
        .iter()
        .map(|deck| deck.iter().map(|(_, c)| c.get()).collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kept_control_exists() {
        let reg = ControlRegistry::new();
        let (_engine, _input) = opendj_engine::engine::Engine::new(&reg, 48_000.0);
        for key in kept_keys() {
            assert!(reg.get("[Groove1]", &key).is_some(), "{key}");
        }
    }
}
