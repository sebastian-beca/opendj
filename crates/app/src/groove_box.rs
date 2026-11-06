//! The app's side of the Groove Box (`opendj_engine::groove`): the kits'
//! and the groove library's names for the screen, and the Groove Boxes kept
//! across restarts in `settings.json` under `groove_box`, one object per
//! deck of its controls' values.

use std::time::{Duration, Instant};

use opendj_control::{ControlHandle, ControlRegistry};
use opendj_engine::groove::{groove_group, GROOVES, ROWS};
use opendj_engine::NUM_DECKS;
use opendj_engine::{kits, patterns};
use serde_json::{json, Map, Value};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::{settings, AppWindow, GrooveGenre, GrooveKit, GrooveName, GrooveUi};

/// The layout of the saved Groove Boxes. Before 2 there were four kits
/// (now from index 2) and four rows, whose sounds were numbered in each
/// kit's own order.
const LAYOUT: f64 = 2.0;

/// The controls a Groove Box keeps, by name.
fn kept_keys() -> Vec<String> {
    let mut keys: Vec<String> = [
        "enabled",
        "kit",
        "groove",
        "genre",
        "pattern",
        "chain",
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
            keys.push(format!("pattern{g}_row{r}_soft"));
            keys.push(format!("pattern{g}_row{r}_accent"));
        }
    }
    for r in 1..=ROWS {
        for k in [
            "mute", "solo", "level", "pitch", "decay", "tone", "pan", "sound",
        ] {
            keys.push(format!("row{r}_{k}"));
        }
    }
    keys
}

/// Shows the kits' names and the groove library's.
pub fn install(app: &AppWindow) {
    let ui = app.global::<GrooveUi>();
    let kits: Vec<GrooveKit> = kits::KITS
        .iter()
        .map(|k| GrooveKit {
            name: k.name.into(),
            about: k.about.into(),
            sounds: ModelRc::new(VecModel::from(
                k.sounds().map(SharedString::from).collect::<Vec<_>>(),
            )),
        })
        .collect();
    ui.set_kits(ModelRc::new(VecModel::from(kits)));
    let genres: Vec<GrooveGenre> = patterns::library()
        .iter()
        .map(|g| GrooveGenre {
            name: g.name.as_str().into(),
            grooves: ModelRc::new(VecModel::from(
                g.grooves
                    .iter()
                    .map(|p| GrooveName {
                        name: p.name.as_str().into(),
                        about: p.about.as_str().into(),
                    })
                    .collect::<Vec<_>>(),
            )),
        })
        .collect();
    ui.set_genres(ModelRc::new(VecModel::from(genres)));
}

/// Brings a Groove Box saved by an older openDJ up to [`LAYOUT`]: the kit
/// it picked moves to its new place, and its rows' sounds go back to the
/// rows' own (rows 1-4 were already kick, snare, hat and open hat).
fn upgrade(values: &mut Map<String, Value>) {
    if values.get("layout").and_then(Value::as_f64).unwrap_or(1.0) >= LAYOUT {
        return;
    }
    if let Some(kit) = values.get("kit").and_then(Value::as_f64) {
        values.insert("kit".into(), json!(kit + 2.0));
    }
    for r in 1..=4 {
        values.remove(&format!("row{r}_sound"));
    }
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
                    // The input the deck plays instead (see
                    // `crate::instruments`), kept beside its Groove Box.
                    .chain(
                        registry
                            .get(&format!("[Channel{}]", d + 1), "live_input")
                            .map(|c| ("live_input".to_string(), c)),
                    )
                    .collect()
            })
            .collect();
        let saved = settings::load()["groove_box"].clone();
        for (d, deck) in controls.iter().enumerate() {
            let Some(mut values) = saved[groove_group(d)].as_object().cloned() else {
                continue;
            };
            upgrade(&mut values);
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
            let mut values: Map<String, Value> = deck
                .iter()
                .map(|(k, c)| (k.clone(), json!(c.get())))
                .collect();
            values.insert("layout".into(), json!(LAYOUT));
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

    #[test]
    fn an_old_groove_box_moves_to_the_new_kits() {
        let mut old = json!({"kit": 1.0, "row2_sound": 1.0, "row2_level": 0.8});
        let values = old.as_object_mut().unwrap();
        upgrade(values);
        assert_eq!(values["kit"], json!(3.0), "Punch is now the fourth kit");
        assert!(values.get("row2_sound").is_none());
        assert_eq!(values["row2_level"], json!(0.8));
        let mut new = json!({"kit": 1.0, "layout": LAYOUT});
        upgrade(new.as_object_mut().unwrap());
        assert_eq!(new["kit"], json!(1.0));
    }
}
