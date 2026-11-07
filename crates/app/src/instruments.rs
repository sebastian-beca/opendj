//! The Instruments source in the library rail (`ui/instruments.slint`):
//! a tree (everything, the Groove Box, plugins by maker, inputs) and a
//! list of what the selected node holds, dragged onto a deck to play
//! through its channel. Dropping is handled on
//! the screen (`InstrumentsUi.drop`), which sets the deck's controls; the
//! engine does the rest (`[ChannelN],live_input`, `[GrooveN],enabled`).

use std::cell::RefCell;
use std::collections::HashSet;

use slint::{ComponentHandle, DataTransfer, ModelRc, SharedString, VecModel};

use crate::audio_plugins::Found;
use crate::presets::{self, Preset, GROOVE};
use crate::{AppWindow, InstrumentNode, InstrumentsUi, LiveItem, Preferences};

/// What dragging an item carries: `opendj-live:<kind>:<index>`.
const LIVE_PAYLOAD: &str = "opendj-live:";

/// Kinds of item, as `LiveItem.kind`.
const KIT: i32 = 1;
const PLUGIN: i32 = 2;
const MIC: i32 = 3;
const AUX: i32 = 4;
/// A preset, by its place in `crate::presets::all`.
const PRESET: i32 = 5;
const LAST_KIND: i32 = 5;

fn payload(kind: i32, index: i32) -> String {
    format!("{LIVE_PAYLOAD}{kind}:{index}")
}

/// The item a drag's text names, as `kind * 1000 + index`.
fn parse(text: &str) -> Option<i32> {
    let (kind, index) = text.strip_prefix(LIVE_PAYLOAD)?.split_once(':')?;
    let (kind, index): (i32, i32) = (kind.parse().ok()?, index.parse().ok()?);
    ((1..=LAST_KIND).contains(&kind) && (0..1000).contains(&index)).then_some(kind * 1000 + index)
}

/// What the tree and the list were built from, kept to rebuild them when a
/// node is opened or selected, the search changes, or plugins and inputs do.
#[derive(Default)]
struct State {
    /// The ticked instrument plugins: picker name, maker, format, kind.
    plugins: Vec<Plugin>,
    /// Each input's device and channels, by the engine's input index.
    routes: Vec<String>,
    /// Nodes opened, by key; Plugins starts open.
    open: HashSet<String>,
    /// The selected node's key.
    current: String,
    search: String,
    /// The keys of the nodes as last shown, by row.
    keys: Vec<String>,
    /// The presets, as `crate::presets::all` lists them.
    presets: Vec<Preset>,
}

struct Plugin {
    key: String,
    name: String,
    maker: String,
    format: &'static str,
    kind: &'static str,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State {
        open: HashSet::from(["plugins".to_string()]),
        current: "all".to_string(),
        ..State::default()
    });
}

/// The microphone's and aux inputs' engine input indexes (see the
/// engine's `inputs`), with their names.
const INPUTS: [(i32, i32, usize, &str); 4] = [
    (MIC, 0, 0, "Microphone"),
    (AUX, 0, 1, "Aux 1"),
    (AUX, 1, 3, "Aux 2"),
    (AUX, 2, 4, "Aux 3"),
];

pub fn install(app: &AppWindow) {
    let ui = app.global::<InstrumentsUi>();
    ui.on_payload(|kind, index| DataTransfer::from(SharedString::from(payload(kind, index))));
    ui.on_item(|data| data.plain_text().ok().and_then(|t| parse(&t)).unwrap_or(-1));
    let weak = app.as_weak();
    ui.on_open_settings(move |page, tab| {
        if let Some(app) = weak.upgrade() {
            app.global::<Preferences>().invoke_show_tab(page, tab);
        }
    });
    let weak = app.as_weak();
    ui.on_select_node(move |row| {
        STATE.with_borrow_mut(|s| {
            if let Some(key) = s.keys.get(row as usize).cloned() {
                s.current = key;
            }
        });
        if let Some(app) = weak.upgrade() {
            refresh(&app);
        }
    });
    let weak = app.as_weak();
    ui.on_toggle_node(move |row| {
        STATE.with_borrow_mut(|s| {
            if let Some(key) = s.keys.get(row as usize).cloned() {
                if !s.open.remove(&key) {
                    s.open.insert(key);
                }
            }
        });
        if let Some(app) = weak.upgrade() {
            refresh(&app);
        }
    });
    let weak = app.as_weak();
    ui.on_search(move |text| {
        STATE.with_borrow_mut(|s| s.search = text.to_string());
        if let Some(app) = weak.upgrade() {
            refresh(&app);
        }
    });
    refresh(app);
}

/// Lists the ticked instrument plugins, in the Instrument input's order,
/// under the names its picker shows.
pub fn show_plugins(app: &AppWindow, instruments: &[Found], names: &[String]) {
    STATE.with_borrow_mut(|s| {
        s.plugins = instruments
            .iter()
            .zip(names)
            .map(|(f, name)| Plugin {
                key: f.key.clone(),
                name: name.clone(),
                maker: f.vendor.clone(),
                format: f.format,
                kind: plugin_kind(&f.features),
            })
            .collect();
    });
    crate::instrument_art::show_plugins(app, instruments.iter().map(|f| f.key.clone()).collect());
    presets::set_plugins(
        instruments
            .iter()
            .zip(names)
            .map(|(f, name)| (f.key.clone(), name.clone()))
            .collect(),
    );
    refresh(app);
}

/// Lists the presets again, after one was saved or renamed.
pub fn presets_changed(app: &AppWindow) {
    refresh(app);
}

/// What an instrument plugin says it is, in a word.
fn plugin_kind(features: &[String]) -> &'static str {
    let has = |word: &str| features.iter().any(|f| f.to_lowercase().contains(word));
    if has("drum") {
        "Drums"
    } else if has("sampler") {
        "Sampler"
    } else if has("synth") {
        "Synth"
    } else {
        "Instrument"
    }
}

/// The plugins' makers, in name order, "Other" for none.
fn makers(plugins: &[Plugin]) -> Vec<String> {
    let mut makers: Vec<String> = plugins.iter().map(|p| maker(p).to_string()).collect();
    makers.sort_by_key(|m| m.to_lowercase());
    makers.dedup_by_key(|m| m.to_lowercase());
    makers
}

fn maker(p: &Plugin) -> &str {
    if p.maker.is_empty() {
        "Other"
    } else {
        &p.maker
    }
}

/// The inputs listed: the microphone and the first aux input always, the
/// other two once set up.
fn inputs(routes: &[String]) -> Vec<&'static (i32, i32, usize, &'static str)> {
    INPUTS
        .iter()
        .filter(|(_, _, input, _)| *input <= 1 || routes.get(*input).is_some_and(|r| !r.is_empty()))
        .collect()
}

/// The tree: (key, node).
fn nodes(s: &State) -> Vec<(String, InstrumentNode)> {
    let node = |name: &str, depth, children, kind, count| InstrumentNode {
        name: name.into(),
        depth,
        children,
        kind,
        count,
    };
    let open = |key: &str| if s.open.contains(key) { 2 } else { 1 };
    let inputs = inputs(&s.routes).len() as i32;
    let total = 1 + s.plugins.len() as i32 + inputs;
    let mut out = vec![
        ("all".to_string(), node("All instruments", 0, 0, 6, total)),
        (
            "presets".to_string(),
            node("My presets", 0, 0, 7, s.presets.len() as i32),
        ),
        ("groove".to_string(), node("Groove Box", 0, 0, 1, -1)),
        (
            "plugins".to_string(),
            node(
                "Plugins",
                0,
                if s.plugins.is_empty() {
                    0
                } else {
                    open("plugins")
                },
                0,
                s.plugins.len() as i32,
            ),
        ),
    ];
    if s.open.contains("plugins") {
        for m in makers(&s.plugins) {
            let key = format!("maker:{m}");
            let mine: Vec<(usize, &Plugin)> = s
                .plugins
                .iter()
                .enumerate()
                .filter(|(_, p)| maker(p).eq_ignore_ascii_case(&m))
                .collect();
            out.push((key.clone(), node(&m, 1, open(&key), 5, mine.len() as i32)));
            if s.open.contains(&key) {
                for (i, p) in mine {
                    out.push((format!("plugin:{i}"), node(&p.name, 2, 0, 2, -1)));
                }
            }
        }
    }
    out.push(("inputs".to_string(), node("Inputs", 0, 0, 4, inputs)));
    out
}

/// The rows the selected node lists, narrowed by the search.
fn rows(s: &State) -> Vec<LiveItem> {
    let groove = || LiveItem {
        kind: KIT,
        index: 0,
        name: "Groove Box".into(),
        r#type: "Drum machine".into(),
        maker: "openDJ".into(),
        format: "Built in".into(),
        problem: "".into(),
    };
    let plugin = |i: usize, p: &Plugin| LiveItem {
        kind: PLUGIN,
        index: i as i32,
        name: p.name.as_str().into(),
        r#type: p.kind.into(),
        maker: p.maker.as_str().into(),
        format: p.format.into(),
        problem: "".into(),
    };
    let input = |&(kind, index, input, name): &(i32, i32, usize, &str)| {
        let route = s.routes.get(input).cloned().unwrap_or_default();
        LiveItem {
            kind,
            index,
            name: name.into(),
            r#type: if kind == MIC {
                "Microphone"
            } else {
                "Line input"
            }
            .into(),
            problem: if route.is_empty() {
                "No input picked"
            } else {
                ""
            }
            .into(),
            maker: route.into(),
            format: "".into(),
        }
    };
    let preset = |i: usize, p: &Preset| LiveItem {
        kind: PRESET,
        index: i as i32,
        name: p.name.as_str().into(),
        r#type: "Preset".into(),
        maker: p.instrument_name.as_str().into(),
        format: "".into(),
        problem: "".into(),
    };
    // An instrument's presets, after it (its default sound).
    let presets_of = |key: &str| -> Vec<LiveItem> {
        s.presets
            .iter()
            .enumerate()
            .filter(|(_, p)| p.instrument == key)
            .map(|(i, p)| preset(i, p))
            .collect()
    };
    let plugins = |only: &dyn Fn(&Plugin) -> bool| -> Vec<LiveItem> {
        s.plugins
            .iter()
            .enumerate()
            .filter(|(_, p)| only(p))
            .map(|(i, p)| plugin(i, p))
            .collect()
    };
    let mut rows: Vec<LiveItem> = match s.current.as_str() {
        "groove" => {
            let mut rows = vec![groove()];
            rows.extend(presets_of(GROOVE));
            rows
        }
        "presets" => s
            .presets
            .iter()
            .enumerate()
            .map(|(i, p)| preset(i, p))
            .collect(),
        "plugins" => plugins(&|_| true),
        "inputs" => inputs(&s.routes).into_iter().map(input).collect(),
        key => {
            if let Some(m) = key.strip_prefix("maker:") {
                plugins(&|p| maker(p).eq_ignore_ascii_case(m))
            } else if let Some(i) = key
                .strip_prefix("plugin:")
                .and_then(|i| i.parse::<usize>().ok())
            {
                let mut rows: Vec<LiveItem> =
                    s.plugins.get(i).map(|p| plugin(i, p)).into_iter().collect();
                if let Some(p) = s.plugins.get(i) {
                    rows.extend(presets_of(&p.key));
                }
                rows
            } else {
                let mut all = vec![groove()];
                all.extend(plugins(&|_| true));
                all.extend(inputs(&s.routes).into_iter().map(input));
                all
            }
        }
    };
    let words: Vec<String> = s.search.split_whitespace().map(str::to_lowercase).collect();
    if !words.is_empty() {
        rows.retain(|r| {
            let text = format!("{} {} {} {}", r.name, r.r#type, r.maker, r.format).to_lowercase();
            words.iter().all(|w| text.contains(w.as_str()))
        });
    }
    rows
}

/// Shows the tree and the list again from the state.
fn refresh(app: &AppWindow) {
    let ui = app.global::<InstrumentsUi>();
    let found = presets::all();
    STATE.with_borrow_mut(|s| {
        s.presets = found;
        let nodes = nodes(s);
        // A node gone (a plugin unticked, a maker folded away) selects its parent.
        if !nodes.iter().any(|(k, _)| *k == s.current) {
            s.current = if s.current.starts_with("plugin") || s.current.starts_with("maker") {
                "plugins".to_string()
            } else {
                "all".to_string()
            };
        }
        let current = nodes.iter().position(|(k, _)| *k == s.current).unwrap_or(0);
        let rows = rows(s);
        let (empty, page, tab) = if !rows.is_empty() {
            ("", -1, 0)
        } else if !s.search.is_empty() {
            ("Nothing matches the search.", -1, 0)
        } else if s.current.starts_with("plugin") || s.current.starts_with("maker") {
            ("Tick the instruments you play in Settings › Plugins…", 7, 1)
        } else {
            ("", -1, 0)
        };
        s.keys = nodes.iter().map(|(k, _)| k.clone()).collect();
        let nodes: Vec<InstrumentNode> = nodes.into_iter().map(|(_, n)| n).collect();
        ui.set_nodes(ModelRc::new(VecModel::from(nodes)));
        ui.set_current_node(current as i32);
        ui.set_rows(ModelRc::new(VecModel::from(rows)));
        ui.set_empty_text(empty.into());
        ui.set_empty_page(page);
        ui.set_empty_tab(tab);
    });
}

/// Shows on the inputs' tiles and live decks where each input comes from,
/// from the engine's input states ("M4 · In 3 · 44.1 kHz"): the device and
/// channels only, or nothing for an input that isn't recording.
pub fn show_routes(app: &AppWindow, status: &[String]) {
    let routes: Vec<String> = status.iter().map(|s| route(s)).collect();
    let shown: Vec<SharedString> = routes.iter().map(|r| r.as_str().into()).collect();
    app.global::<InstrumentsUi>()
        .set_routes(ModelRc::new(VecModel::from(shown)));
    STATE.with_borrow_mut(|s| s.routes = routes);
    refresh(app);
}

/// "M4 · In 3" from an open input's state; "" from anything else (not set
/// up, or an error).
fn route(status: &str) -> String {
    let parts: Vec<&str> = status.split(" · ").collect();
    match parts.iter().position(|p| p.ends_with(" kHz")) {
        Some(k) if k > 0 => parts[..k].join(" · "),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payloads_read_back_as_their_items() {
        assert_eq!(parse(&payload(1, 2)), Some(1002));
        assert_eq!(parse(&payload(2, 0)), Some(2000));
        assert_eq!(parse(&payload(4, 0)), Some(4000));
        assert_eq!(parse("opendj-row:3"), None);
        assert_eq!(parse("opendj-live:9:0"), None);
        assert_eq!(parse("opendj-live:1:x"), None);
    }

    fn state(current: &str) -> State {
        let plugin = |name: &str, maker: &str| Plugin {
            key: format!("clap:{name}"),
            name: name.into(),
            maker: maker.into(),
            format: "CLAP",
            kind: "Synth",
        };
        State {
            plugins: vec![
                plugin("Acid V", "Arturia"),
                plugin("Vital", "Vital Audio"),
                plugin("Jun-6 V", "arturia"),
            ],
            routes: vec![
                "".into(),
                "M4 · In 3".into(),
                "".into(),
                "M4 · In 4".into(),
                "".into(),
            ],
            open: HashSet::from(["plugins".to_string(), "maker:Arturia".to_string()]),
            current: current.into(),
            ..State::default()
        }
    }

    #[test]
    fn plugins_group_by_maker_and_inputs_list_when_set_up() {
        let keys: Vec<String> = nodes(&state("all")).into_iter().map(|(k, _)| k).collect();
        assert_eq!(
            keys,
            [
                "all",
                "presets",
                "groove",
                "plugins",
                "maker:Arturia",
                "plugin:0",
                "plugin:2",
                "maker:Vital Audio",
                "inputs"
            ]
        );
        let names = |s: &State| {
            rows(s)
                .iter()
                .map(|r| r.name.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&state("maker:Arturia")), ["Acid V", "Jun-6 V"]);
        // The mic and aux 1 always, aux 2 once it has an input; aux 3 not.
        assert_eq!(names(&state("inputs")), ["Microphone", "Aux 1", "Aux 2"]);
        assert_eq!(rows(&state("inputs"))[0].problem, "No input picked");
        let mut s = state("all");
        s.search = "vital".into();
        assert_eq!(names(&s), ["Vital"]);
    }

    #[test]
    fn presets_list_under_their_instrument_and_in_my_presets() {
        let preset = |name: &str, instrument: &str, instrument_name: &str| Preset {
            name: name.into(),
            instrument: instrument.into(),
            instrument_name: instrument_name.into(),
            path: format!("/p/{name}.json").into(),
        };
        let mut s = state("groove");
        s.presets = vec![
            preset("Four Floor", GROOVE, "Groove Box"),
            preset("Ambient Ebow", "clap:Jun-6 V", "Jun-6 V"),
        ];
        let names = |s: &State| {
            rows(s)
                .iter()
                .map(|r| (r.kind, r.index, r.name.to_string()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(&s),
            [
                (KIT, 0, "Groove Box".into()),
                (PRESET, 0, "Four Floor".into())
            ]
        );
        s.current = "plugin:2".into();
        assert_eq!(
            names(&s),
            [
                (PLUGIN, 2, "Jun-6 V".into()),
                (PRESET, 1, "Ambient Ebow".into())
            ]
        );
        s.current = "presets".into();
        assert_eq!(names(&s).len(), 2);
        assert_eq!(parse(&payload(PRESET, 1)), Some(5001));
    }

    #[test]
    fn routes_keep_the_device_and_channels() {
        assert_eq!(route("M4 · In 3 · 44.1 kHz"), "M4 · In 3");
        assert_eq!(
            route("M4 · In 1 & 2 · 48 kHz · silent so far"),
            "M4 · In 1 & 2"
        );
        assert_eq!(route(""), "");
        assert_eq!(route("M4 unavailable"), "");
        assert_eq!(route("M4 can't record In 3 at 44.1 kHz"), "");
    }
}
