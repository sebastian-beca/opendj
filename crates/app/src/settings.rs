//! Preferences kept between runs, in `settings.json` in the config folder.

use std::collections::HashMap;
use std::path::PathBuf;

use mixxx_controllers::MappingChoice;
use mixxx_engine::key::Key;
use mixxx_engine::scan::default_music_dir;
use mixxx_engine::{AudioSetup, InputRole, InputRoute, RecordFormat, RecordSettings, Role, Route};
use mixxx_library::query::FieldNames;
use mixxx_plugins::inbox::Mode;
use mixxx_plugins::PluginConfig;
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

pub fn load() -> Value {
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
/// [`crate::automix::LENGTHS`] and [`crate::automix::STYLES`]. The length
/// is saved under a new name since 0 s went in first; an older
/// `automix_length` is moved up one to match.
pub fn automix_length() -> usize {
    let v = load();
    match v["automix_mix_length"].as_u64() {
        Some(i) => i as usize,
        None => v["automix_length"].as_u64().map_or(2, |i| i as usize + 1),
    }
}

pub fn set_automix_length(i: usize) {
    save(|v| v["automix_mix_length"] = json!(i));
}

pub fn automix_style() -> usize {
    load()["automix_style"].as_u64().unwrap_or(0) as usize
}

pub fn set_automix_style(i: usize) {
    save(|v| v["automix_style"] = json!(i));
}

/// Where mixes are recorded, and in what format.
pub fn recording() -> RecordSettings {
    let v = load();
    let d = RecordSettings::default();
    RecordSettings {
        dir: v["recording_folder"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or(d.dir),
        format: v["recording_format"]
            .as_str()
            .and_then(RecordFormat::from_name)
            .unwrap_or(d.format),
        bits: match v["recording_bits"].as_u64() {
            Some(16) => 16,
            _ => d.bits,
        },
    }
}

/// The output routes chosen last time, with the sample rate and buffer
/// size. Settings from before routes are read too: the main device, and a
/// headphone device or automatic 3/4.
pub fn audio_setup() -> AudioSetup {
    let v = load();
    let sample_rate = v["sample_rate"].as_u64().map(|r| r as u32);
    let buffer_frames = v["buffer_frames"].as_u64().map(|n| n as u32);
    let inputs = v["inputs"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|r| {
                    Some(InputRoute {
                        role: InputRole::from_name(r["role"].as_str()?)?,
                        device: r["device"].as_str().unwrap_or_default().to_string(),
                        channel: r["channel"].as_u64().unwrap_or(0) as u16,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    if let Some(list) = v["outputs"].as_array() {
        return AudioSetup {
            inputs,
            sample_rate,
            buffer_frames,
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
    AudioSetup {
        routes,
        inputs,
        sample_rate,
        buffer_frames,
    }
}

pub fn set_audio_setup(setup: &AudioSetup) {
    save(|v| {
        v["outputs"] = setup
            .routes
            .iter()
            .map(|r| json!({ "role": r.role.name(), "device": r.device, "pair": r.pair }))
            .collect();
        v["inputs"] = setup
            .inputs
            .iter()
            .map(|r| json!({ "role": r.role.name(), "device": r.device, "channel": r.channel }))
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

/// A plugin source's saved searches (see [`crate::sources`]), oldest first:
/// what was typed, how it was read and which copies to prefer.
/// A saved search: its text, kind, preference, and when it was added (Unix
/// milliseconds).
pub type SavedSearch = (String, String, String, u64);

/// A source's saved searches. Ones saved before searches had a time get
/// their place in the list instead, oldest first as they were kept then.
pub fn source_searches(source: &str) -> Vec<SavedSearch> {
    load()["source_searches"][source]
        .as_array()
        .map(|a| {
            a.iter()
                .enumerate()
                .filter_map(|(i, s)| {
                    let text = s["text"].as_str()?.to_string();
                    let field = |k: &str| s[k].as_str().unwrap_or_default().to_string();
                    let added = s["added"].as_u64().unwrap_or(i as u64);
                    Some((text, field("kind"), field("prefer"), added))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_source_searches(source: &str, searches: &[SavedSearch]) {
    let list: Vec<Value> = searches
        .iter()
        .map(|(text, kind, prefer, added)| {
            json!({ "text": text, "kind": kind, "prefer": prefer, "added": added })
        })
        .collect();
    save(|v| {
        if !v["source_searches"].is_object() {
            v["source_searches"] = json!({});
        }
        v["source_searches"][source] = json!(list);
    });
}

/// A saved number, or `default`.
pub fn number(key: &str, default: f64) -> f64 {
    load()[key].as_f64().unwrap_or(default)
}

/// The DJ's Discogs personal access token, which lets the cover search
/// look on Discogs; empty for none.
pub fn discogs_token() -> String {
    load()["discogs_token"].as_str().unwrap_or("").to_string()
}

pub fn set_discogs_token(token: &str) {
    save(|v| v["discogs_token"] = json!(token.trim()));
}

/// A saved switch, or `default`.
pub fn flag(key: &str, default: bool) -> bool {
    load()[key].as_bool().unwrap_or(default)
}

/// A saved text, or "".
pub fn string(key: &str) -> String {
    load()[key].as_str().unwrap_or_default().to_string()
}

/// The setting that says whether an importer runs at all; off until the DJ
/// turns it on.
pub fn import_enabled_key(importer: &str) -> String {
    format!("import:{importer}")
}

pub fn import_enabled(importer: &str) -> bool {
    flag(&import_enabled_key(importer), false)
}

/// The database an importer reads: the one the DJ picked in the import
/// wizard, else the one the app keeps where it usually does.
pub fn import_database(importer: &dyn mixxx_plugins::LibraryImport) -> Option<PathBuf> {
    path_of(&import_database_key(importer.id())).or_else(|| importer.database())
}

pub fn import_database_key(importer: &str) -> String {
    format!("import_database:{importer}")
}

/// Whether the other app's tags, ratings and colours replace the library's,
/// or only fill in what is empty; they replace them unless the DJ says.
pub fn import_theirs_win(importer: &str) -> bool {
    flag(&import_theirs_key(importer), true)
}

pub fn import_theirs_key(importer: &str) -> String {
    format!("import_theirs:{importer}")
}

/// Set when the DJ removes an import; the next library update takes it
/// out and clears it.
pub fn import_remove_key(importer: &str) -> String {
    format!("import_remove:{importer}")
}

/// The setting that says whether an importer brings in one kind of list.
pub fn import_group_key(importer: &str, group: &str) -> String {
    format!("import:{importer}:{group}")
}

/// The kinds of lists the DJ wants `importer` to bring in, by id.
pub fn import_groups(importer: &dyn mixxx_plugins::LibraryImport) -> Vec<&'static str> {
    let v = load();
    importer
        .list_groups()
        .into_iter()
        .filter(|g| {
            v[import_group_key(importer.id(), g.id).as_str()]
                .as_bool()
                .unwrap_or(g.on_by_default)
        })
        .map(|g| g.id)
        .collect()
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

/// Controllers shown in Settings › Controllers, or taken off it, by their
/// device key. A device not listed shows if openDJ has a mapping for it.
pub fn controllers_shown() -> HashMap<String, bool> {
    load()["controllers_shown"]
        .as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_bool()?)))
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_controller_shown(key: &str, shown: bool) {
    save(|v| {
        if !v["controllers_shown"].is_object() {
            v["controllers_shown"] = json!({});
        }
        v["controllers_shown"][key] = json!(shown);
    });
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

/// Where the DJ's own controller mappings are kept.
pub fn own_mappings_dir() -> Option<PathBuf> {
    config_dir().map(|d| d.join("controller-mappings"))
}

/// The DJ's own mapping used in place of a Mixxx mapping (by its file
/// name), if one was picked.
pub fn controller_version(based_on: &str) -> Option<String> {
    load()["controller_versions"][based_on]
        .as_str()
        .map(String::from)
}

pub fn set_controller_version(based_on: &str, id: Option<&str>) {
    save(|v| {
        if !v["controller_versions"].is_object() {
            v["controller_versions"] = json!({});
        }
        let versions = v["controller_versions"].as_object_mut().unwrap();
        match id {
            Some(id) => {
                versions.insert(based_on.to_string(), json!(id));
            }
            None => {
                versions.remove(based_on);
            }
        }
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

/// Whether the library is brought up to date each time openDJ starts:
/// new files in the music and stage folders, and changed imports. When off,
/// that only happens on Update library.
pub const SCAN_ON_START: &str = "scan_on_start";

pub fn scan_on_start() -> bool {
    flag(SCAN_ON_START, true)
}

/// The stage folders (see [`mixxx_library::stages`]): any number of inbox
/// folders, one triage folder and one collection folder.
pub fn stage_folders() -> mixxx_library::stages::Folders {
    mixxx_library::stages::Folders {
        inbox: paths(STAGE_INBOX),
        triage: path_of(STAGE_TRIAGE),
        collection: path_of(STAGE_COLLECTION),
    }
}

pub const STAGE_INBOX: &str = "stage_inbox";
pub const STAGE_TRIAGE: &str = "stage_triage";
pub const STAGE_COLLECTION: &str = "stage_collection";
/// Whether openDJ moves files between the stage folders. Off until the DJ
/// turns it on, as another app may still point at where they are; moves
/// wait in a queue meanwhile.
pub const MOVE_FILES: &str = "move_files";
/// Whether a file going from the inbox to triage is renamed "Artist -
/// Title (Remix)".
pub const RENAME_TO_TRIAGE: &str = "rename_to_triage";

pub fn move_files() -> bool {
    flag(MOVE_FILES, false)
}

pub fn rename_to_triage() -> bool {
    flag(RENAME_TO_TRIAGE, true)
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

/// What the DJ filled in on a plugin's page in Settings, and the plugin's
/// own folder in the config folder.
pub fn plugin_config(id: &str) -> PluginConfig {
    let values = load()["plugins"][id]
        .as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default();
    PluginConfig {
        values,
        dir: config_dir().map(|d| d.join("plugins").join(id)),
    }
}

pub fn set_plugin_value(id: &str, key: &str, value: &str) {
    save(|v| {
        if !v["plugins"].is_object() {
            v["plugins"] = json!({});
        }
        if !v["plugins"][id].is_object() {
            v["plugins"][id] = json!({});
        }
        v["plugins"][id][key] = json!(value);
    });
}

fn strings(v: &Value) -> Option<Vec<String>> {
    v.as_array().map(|a| {
        a.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect()
    })
}

/// What the DJ calls grouping and their tag fields ("Trip Stage", "Mood",
/// "Instrument"); Grouping, Tag 1 and Tag 2 until they choose.
pub fn field_names() -> FieldNames {
    let v = load();
    let d = FieldNames::default();
    FieldNames {
        grouping: v["grouping_name"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .map_or(d.grouping, String::from),
        tags: strings(&v["tag_fields"])
            .filter(|t| !t.is_empty())
            .map(|t| {
                // A name left empty goes by its number.
                t.into_iter()
                    .enumerate()
                    .map(|(i, n)| match n.trim() {
                        "" => format!("Tag {}", i + 1),
                        n => n.to_string(),
                    })
                    .collect()
            })
            .unwrap_or(d.tags),
    }
}

pub fn set_field_names(names: &FieldNames) {
    save(|v| {
        v["grouping_name"] = json!(names.grouping);
        v["tag_fields"] = json!(names.tags);
    });
}

/// The filter chips kept on the bar, by field.
pub fn filter_pinned() -> Vec<String> {
    strings(&load()["filter_pinned"]).unwrap_or_else(|| {
        crate::browser::DEFAULT_PINNED
            .iter()
            .map(|s| s.to_string())
            .collect()
    })
}

pub fn set_filter_pinned(pinned: &[String]) {
    save(|v| v["filter_pinned"] = json!(pinned));
}

/// How close a tempo must be when the BPM chip follows the playing track,
/// as an index into the tolerances offered.
pub fn filter_tolerance() -> usize {
    load()["filter_tolerance"].as_u64().unwrap_or(4) as usize
}

pub fn set_filter_tolerance(i: usize) {
    save(|v| v["filter_tolerance"] = json!(i));
}

/// The track list's columns, in order, as (column id, width); empty for
/// the default set.
pub fn columns() -> Vec<(i32, f32)> {
    load()["columns"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|c| {
                    Some((
                        c["id"].as_i64()? as i32,
                        c["width"].as_f64().unwrap_or(0.0) as f32,
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_columns(columns: &[(i32, f32)]) {
    let list: Vec<Value> = columns
        .iter()
        .map(|(id, width)| json!({ "id": id, "width": width }))
        .collect();
    save(|v| v["columns"] = json!(list));
}

/// The DJ's own keyboard shortcuts, as (key, openDJscript line) pairs in
/// the order saved. They replace the default for the same key
/// ([`crate::shortcuts::DEFAULTS`]); an empty line turns that key off.
pub fn keyboard_shortcuts() -> Vec<(String, String)> {
    load()["keyboard_shortcuts"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|s| {
                    Some((
                        s["key"].as_str()?.to_string(),
                        s["script"].as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_keyboard_shortcuts(shortcuts: &[(String, String)]) {
    let list: Vec<Value> = shortcuts
        .iter()
        .map(|(key, script)| json!({ "key": key, "script": script }))
        .collect();
    save(|v| v["keyboard_shortcuts"] = json!(list));
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
