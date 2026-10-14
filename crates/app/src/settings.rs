//! Preferences kept between runs, in `settings.json` in the config folder.

use std::collections::HashMap;
use std::path::PathBuf;

use mixxx_controllers::MappingChoice;
use mixxx_engine::key::Key;
use mixxx_engine::scan::default_music_dir;
use mixxx_engine::{AudioSetup, Role, Route};
use mixxx_plugins::inbox::Mode;
use serde_json::{json, Value};

/// Where openDJ keeps its settings: `OPENDJ_CONFIG`, else
/// `~/Library/Application Support/openDJ` on macOS and
/// `$XDG_CONFIG_HOME/opendj` (or `~/.config/opendj`) elsewhere.
pub fn config_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("OPENDJ_CONFIG") {
        return Some(PathBuf::from(d));
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let base = if cfg!(target_os = "macos") {
        home?.join("Library/Application Support")
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| home.map(|h| h.join(".config")))?
    };
    let name = if cfg!(target_os = "macos") {
        "openDJ"
    } else {
        "opendj"
    };
    static MOVED: std::sync::Once = std::sync::Once::new();
    MOVED.call_once(|| mixxx_engine::cache::move_old_folder(&base, name));
    Some(base.join(name))
}

fn path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("settings.json"))
}

fn load() -> Value {
    path()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| json!({}))
}

/// Whether keys are shown in Camelot notation (the default) or as notes.
pub fn camelot_keys() -> bool {
    load()["camelot_keys"].as_bool().unwrap_or(true)
}

pub fn set_camelot_keys(on: bool) {
    save(|v| v["camelot_keys"] = json!(on));
}

/// The Automix mix length and style picked last time, as indices into
/// [`crate::automix::LENGTHS`] and [`crate::automix::STYLES`].
pub fn automix_length() -> usize {
    load()["automix_length"].as_u64().unwrap_or(1) as usize
}

pub fn set_automix_length(i: usize) {
    save(|v| v["automix_length"] = json!(i));
}

pub fn automix_style() -> usize {
    load()["automix_style"].as_u64().unwrap_or(0) as usize
}

pub fn set_automix_style(i: usize) {
    save(|v| v["automix_style"] = json!(i));
}

/// The output routes chosen last time. Settings from before routes are
/// read too: the main device, and a headphone device or automatic 3/4.
pub fn audio_setup() -> AudioSetup {
    let v = load();
    if let Some(list) = v["outputs"].as_array() {
        return AudioSetup {
            routes: list
                .iter()
                .filter_map(|r| {
                    Some(Route {
                        role: Role::from_name(r["role"].as_str()?)?,
                        device: r["device"].as_str().unwrap_or_default().to_string(),
                        pair: r["pair"].as_u64().unwrap_or(0) as u16,
                    })
                })
                .collect(),
        };
    }
    let main = v["main_device"].as_str().unwrap_or_default().to_string();
    let mut routes = vec![Route {
        role: Role::Master,
        device: main.clone(),
        pair: 0,
    }];
    match v["headphones_device"].as_str() {
        Some("") => {}
        Some(d) if d != main => routes.push(Route {
            role: Role::Headphones,
            device: d.to_string(),
            pair: 0,
        }),
        _ => routes.push(Route {
            role: Role::Headphones,
            device: main,
            pair: 1,
        }),
    }
    AudioSetup { routes }
}

pub fn set_audio_setup(setup: &AudioSetup) {
    save(|v| {
        v["outputs"] = setup
            .routes
            .iter()
            .map(|r| json!({ "role": r.role.name(), "device": r.device, "pair": r.pair }))
            .collect();
    });
}

/// The sources hidden from the library rail, by name.
pub fn hidden_sources() -> Vec<String> {
    load()["hidden_sources"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_hidden_sources(names: Vec<String>) {
    save(|v| v["hidden_sources"] = json!(names));
}

/// A saved number, or `default`.
pub fn number(key: &str, default: f64) -> f64 {
    load()[key].as_f64().unwrap_or(default)
}

/// A saved switch, or `default`.
pub fn flag(key: &str, default: bool) -> bool {
    load()[key].as_bool().unwrap_or(default)
}

/// A saved path, if one was chosen.
pub fn path_of(key: &str) -> Option<PathBuf> {
    load()[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

/// A saved list of paths.
pub fn paths(key: &str) -> Vec<PathBuf> {
    load()[key]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default()
}

pub fn set(key: &str, value: Value) {
    save(|v| v[key] = value);
}

/// The mapping picked for each controller, by its device key.
pub fn controller_mappings() -> HashMap<String, MappingChoice> {
    load()["controller_mappings"]
        .as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| {
                    let choice = match v.as_str()? {
                        "off" => MappingChoice::Off,
                        file => MappingChoice::File(file.to_string()),
                    };
                    Some((k.clone(), choice))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_controller_mappings(choices: &HashMap<String, MappingChoice>) {
    save(|v| {
        v["controller_mappings"] = choices
            .iter()
            .filter_map(|(k, c)| match c {
                MappingChoice::Auto => None,
                MappingChoice::Off => Some((k.clone(), json!("off"))),
                MappingChoice::File(f) => Some((k.clone(), json!(f))),
            })
            .collect::<serde_json::Map<_, _>>()
            .into();
    });
}

/// The folder scanned for music: `OPENDJ_MUSIC`, else the one chosen in
/// settings, else `~/Music`.
pub fn music_folder() -> Option<PathBuf> {
    if std::env::var_os("OPENDJ_MUSIC").is_some() {
        return default_music_dir();
    }
    path_of("music_folder").or_else(default_music_dir)
}

/// The purchase inbox: whether it's on, the folder it watches, and what it
/// does with what arrives (see [`mixxx_plugins::inbox::Mode`]).
pub struct InboxSettings {
    pub on: bool,
    pub dir: Option<PathBuf>,
    pub mode: Mode,
    /// Seconds since 1970 when it was first switched on: files older than
    /// this were already there and are left alone.
    pub since: u64,
}

/// The inbox settings. `OPENDJ_INBOX` sets the folder (and switches it
/// on); otherwise it watches Downloads.
pub fn inbox() -> InboxSettings {
    let v = load();
    let env = std::env::var_os("OPENDJ_INBOX").map(PathBuf::from);
    let dir = env
        .clone()
        .or_else(|| v["inbox_dir"].as_str().map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Downloads")));
    let since = match v["inbox_since"].as_u64() {
        Some(s) => s,
        None => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            save(|v| v["inbox_since"] = json!(now));
            now
        }
    };
    InboxSettings {
        on: crate::browser::SERVICES_ENABLED
            && (env.is_some() || v["inbox_on"].as_bool().unwrap_or(true)),
        dir,
        mode: Mode::from_name(v["inbox_mode"].as_str().unwrap_or("move")),
        since,
    }
}

pub fn set_inbox_on(on: bool) {
    save(|v| v["inbox_on"] = json!(on));
}

pub fn set_inbox_mode(mode: Mode) {
    save(|v| v["inbox_mode"] = json!(mode.name()));
}

fn save(change: impl FnOnce(&mut Value)) {
    let Some(path) = path() else { return };
    let mut v = load();
    change(&mut v);
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    let _ = std::fs::write(path, v.to_string());
}

/// A track's key as the user wants keys written. Keys that can't be read
/// (an odd tag) are shown as they are.
pub fn show_key(key: &str, camelot: bool) -> String {
    match Key::parse(key) {
        Some(k) if camelot => k.camelot(),
        Some(k) => k.name(),
        None => key.to_string(),
    }
}
