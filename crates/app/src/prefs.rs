//! The settings window: builds the rows of each page from what is set now,
//! applies changes to the engine's controls at once and keeps them in
//! settings.json. Saved choices are applied again at start.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use crate::audio_plugins::{AudioPlugins, Found};
use crate::broadcast::{self, Broadcast};
use crate::list_picker::{Picker, Ticked};
use opendj_control::ControlRegistry;
use opendj_controllers::{is_system_port, own, Controllers, MappingChoice, Port};
use opendj_engine::inputs::{DEFAULT_DUCK_STRENGTH, DUCK_AUTO};
use opendj_engine::{
    analysis, beatthis, cache, channel_names, deck_group, engine, input_devices, key, meter,
    separation, AudioSetup, InputRole, InputRoute, RecordFormat, Recorder, BUFFER_SIZES,
    DEFAULT_AUTO_GAIN_TARGET, NUM_DECKS, SAMPLE_RATES,
};
use opendj_library::query;
use opendj_plugins::source::DOWNLOAD_FOLDER;
use opendj_plugins::{LibraryImport, Plugin, SettingKind};
use serde_json::json;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::tour::Part;
use crate::{
    beat_counter_mode, settings, short_port_name, shortcuts, AppWindow, AudioSettings, BeatCounter,
    ControllerEditor, ImportWizard, InputRow, InputSettings, KeyNotation, PluginErrors, PrefItem,
    Preferences, StoragePart, StorageUi, Tour,
};

const AUDIO: i32 = 0;
const CONTROLLERS: i32 = 1;
const LIBRARY: i32 = 2;
/// Settings › Collection's tabs.
const LIBRARY_TABS: [&str; 5] = ["General", "Stages", "Tags", "Import", "Health"];
/// The Import tab's place in LIBRARY_TABS.
const IMPORT_TAB: usize = 3;
/// The Health tab's place in LIBRARY_TABS: online lookup and clean-up.
const HEALTH_TAB: usize = 4;
/// Other DJ apps whose libraries openDJ can't bring in yet, listed on the
/// Import tab so the DJ sees what's coming.
const COMING_IMPORTS: [(&str, &str); 1] = [("Serato DJ", "Tracks, cues, loops and crates.")];
const ANALYSIS: i32 = 3;
const DECKS: i32 = 4;
const MIXER: i32 = 5;
/// Where music comes from: the rail's sources, their sign-ins, download
/// folders and the cache they share.
const SOURCES: i32 = 6;
/// Settings › Sources, for opening it from elsewhere.
pub const SOURCES_PAGE: i32 = SOURCES;
/// CLAP, VST3 and AU plugins.
const AUDIO_PLUGINS: i32 = 7;
/// Settings › Plugins' tabs.
const PLUGIN_TABS: [&str; 3] = ["Effects", "Instruments", "Folders"];
/// The Folders tab's place in PLUGIN_TABS.
const FOLDERS_TAB: usize = 2;
const BROADCAST: i32 = 8;
const KEYBOARD: i32 = 9;
const FEEDBACK: i32 = 10;

/// Bitrates offered for a broadcast, in kb/s.
const BITRATES: [u32; 7] = [64, 96, 128, 160, 192, 256, 320];

const SWITCH: i32 = 0;
const MENU: i32 = 1;
const BUTTON: i32 = 2;
const PATH: i32 = 3;
const OUTPUTS: i32 = 4;
const NOTE: i32 = 6;
const HEADING: i32 = 7;
const SEGMENTED: i32 = 8;
const TEXT: i32 = 9;
/// A controller: its mapping menu, with Edit (when `action` is set) and
/// Remove beside it.
const CONTROLLER: i32 = 10;
const SHORTCUT: i32 = 11;
/// A tick box, indented by `chosen`; `value` "some" shows a folder partly
/// ticked, and `action` its open or closed arrow, pressing id + "#fold".
const TICK: i32 = 12;
/// The inputs table (see [`Prefs::inputs_item`]).
const INPUTS_TABLE: i32 = 13;
/// Filter chips (`choices`, on by bit in `chosen`) and a search box
/// (`value`, its hint in `detail`), above a long list.
const FILTER_BAR: i32 = 16;
/// A source on Settings › Sources: its name, a status dot (`chosen` 0
/// grey, 1 green, 2 orange), what it's doing, and its In rail switch
/// (`on`). `action` is its fold arrow, empty when there's nothing to open.
const SOURCE_ROW: i32 = 17;
/// A list of folders, by name, with + and − under it (see `folder_list`).
const FOLDER_LIST: i32 = 21;
/// How track colors show: segmented buttons with a preview beside them.
const TRACK_COLOR: i32 = 22;
/// The storage bar with its Manage menu (see StorageUi in preferences.slint).
const STORAGE: i32 = 23;
/// The Manage menu's clean-ups, in its order.
const STORAGE_ACTIONS: [&str; 4] = [
    "Clear analysis cache…",
    "Clear high quality stems…",
    "Clear played tracks…",
    "Show cache folder",
];
/// A folder list's title icon in `chosen`: the sidebar's stage icons (its node
/// kinds), or 0 for a folder.
const STAGE_ICON_INBOX: i32 = 10;
const STAGE_ICON_PREP: i32 = 11;
const STAGE_ICON_READY: i32 = 19;

/// Tempo fader ranges, as fractions of normal speed.
const RATE_RANGES: [f64; 8] = [0.04, 0.06, 0.08, 0.10, 0.16, 0.24, 0.50, 1.00];
const DEFAULT_RATE_RANGE: f64 = 0.08;
/// Analysis tempo ranges, plus our default.
const BPM_RANGES: [(f64, f64); 6] = [
    (60.0, 120.0),
    (60.0, 180.0),
    (70.0, 180.0),
    (88.0, 175.0),
    (98.0, 195.0),
    (118.0, 235.0),
];
/// Mixxx's cue_mode values for the two CUE behaviours offered: Mixxx's own
/// (preview while held) and Numark's (set and return only).
const CUE_MODES: [f64; 2] = [0.0, 3.0];
/// Auto gain targets offered, in LUFS.
/// The recording formats offered, as (format, bits).
const RECORD_FORMATS: [(RecordFormat, u16, &str); 4] = [
    (RecordFormat::Wav, 24, "WAV · 24-bit"),
    (RecordFormat::Wav, 16, "WAV · 16-bit"),
    (RecordFormat::Flac, 24, "FLAC · 24-bit"),
    (RecordFormat::Flac, 16, "FLAC · 16-bit"),
];

const AUTO_GAIN_TARGETS: [f64; 7] = [-8.0, -10.0, -12.0, -14.0, -16.0, -18.0, -23.0];

/// Loudness meter targets offered, in LUFS, with what each is for.
const LOUDNESS_TARGETS: [(f64, &str); 4] = [
    (-16.0, "Internet radio · −16 LUFS"),
    (-18.0, "Podcast or talk · −18 LUFS"),
    (-14.0, "Livestream · −14 LUFS"),
    (-23.0, "Broadcast, EBU R128 · −23 LUFS"),
];

/// The master limiter's ceilings offered while recording or on air, in dBTP.
const AIR_CEILINGS: [(f64, &str); 2] = [(-1.0, "−1 dBTP"), (-2.0, "−2 dBTP · for low bitrates")];

/// How far ducking turns the music down, in dB, as offered.
const DUCK_DB: [f64; 4] = [6.0, 12.0, 18.0, 24.0];

/// `[Master],duckStrength` for ducking by `db`.
fn duck_strength(db: f64) -> f64 {
    1.0 - 10f64.powf(-db / 20.0)
}

/// Applies the saved choices to the engine's controls, at start.
pub fn apply_saved(registry: &ControlRegistry) {
    let set = |group: &str, key: &str, value: f64| {
        if let Some(c) = registry.get(group, key) {
            c.set(value);
        }
    };
    for d in 0..NUM_DECKS {
        let g = deck_group(d);
        set(&g, "cue_mode", settings::number("cue_mode", CUE_MODES[0]));
        set(
            &g,
            "rateRange",
            settings::number("rate_range", DEFAULT_RATE_RANGE),
        );
        set(&g, "rate_dir", rate_dir());
        set(&g, "quantize", f(settings::flag("quantize", true)));
        set(&g, "vinyl_mode", f(settings::flag("vinyl_mode", true)));
        if settings::flag("keylock_default", false) {
            set(&g, "keylock", 1.0);
        }
    }
    set(
        "[Controls]",
        "AllowTrackLoadToPlayingDeck",
        f(!settings::flag("protect_playing", true)),
    );
    set(
        "[Mixer Profile]",
        "xFaderShape",
        settings::number("xfader_shape", 0.0),
    );
    set(
        "[Mixer Profile]",
        "xFaderReverse",
        f(settings::flag("xfader_reverse", false)),
    );
    set(
        "[Mixer]",
        "crossfader_enabled",
        f(settings::flag("xfader_enabled", true)),
    );
    set(
        "[Master]",
        "headSplit",
        f(settings::flag("head_split", false)),
    );
    set(
        "[Master]",
        "talkover_ducking",
        settings::number("talkover_ducking", DUCK_AUTO),
    );
    set(
        "[Master]",
        "duckStrength",
        settings::number("duck_strength", DEFAULT_DUCK_STRENGTH),
    );
    set(
        "[ReplayGain]",
        "ReplayGainEnabled",
        f(settings::flag("auto_gain", true)),
    );
    set(
        "[ReplayGain]",
        "target_lufs",
        settings::number("auto_gain_target", DEFAULT_AUTO_GAIN_TARGET),
    );
    set(
        "[Master]",
        "loudness_target",
        settings::number("loudness_target", meter::DEFAULT_LOUDNESS_TARGET),
    );
    set(
        "[Master]",
        "air_ceiling",
        settings::number("air_ceiling", engine::DEFAULT_AIR_CEILING_DB),
    );
    let (lo, hi) = bpm_range();
    analysis::set_bpm_range(lo, hi);
    key::set_detector(key_detector());
    beatthis::set_enabled(settings::flag("neural_beats", true));
    separation::set_on_load(settings::flag("stems_on_load", true));
    apply_stems_cache_limit();
    key::set_mixing(key_mixing());
    key::set_notation(settings::key_notation());
    set("[Controls]", "key_match_on_load", key_match_range() as f64);
    for (id, _, _) in AUTO_RESETS {
        set("[Controls]", id, f(settings::flag(id, true)));
    }
    set(
        "[Controls]",
        "pfl_on_load",
        f(settings::flag("pfl_on_load", true)),
    );
    // The levels as the DJ left them; the first time, the engine's own
    // starting levels stand (master 50%, phones 25%, cue only).
    for (key, id) in LEVEL_KEYS {
        if let Some(value) = settings::load()[id].as_f64() {
            set("[Master]", key, value);
        }
    }
}

/// The `[Master]` levels kept between runs, with their settings keys.
const LEVEL_KEYS: [(&str, &str); 4] = [
    ("gain", "master_level"),
    ("headGain", "phones_level"),
    ("headMix", "cue_mix"),
    ("booth_gain", "booth_level"),
];

/// The `[Master]` levels as set now.
fn levels(registry: &ControlRegistry) -> Vec<Option<f64>> {
    LEVEL_KEYS
        .iter()
        .map(|(key, _)| registry.get("[Master]", key).map(|c| c.get()))
        .collect()
}

/// Keeps the master, phones, cue mix and booth levels in settings.json
/// whenever they change, so the next start puts them back. Checked every
/// couple of seconds rather than at quit, which a crash or a forced quit
/// would skip. Keep the timer alive for as long as the app runs.
pub fn remember_levels(registry: Arc<ControlRegistry>) -> slint::Timer {
    let saved = RefCell::new(levels(&registry));
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_secs(2),
        move || {
            let now = levels(&registry);
            if now == *saved.borrow() {
                return;
            }
            for ((_, id), value) in LEVEL_KEYS.iter().zip(&now) {
                if let Some(value) = value {
                    settings::set(id, json!(value));
                }
            }
            *saved.borrow_mut() = now;
        },
    );
    timer
}

/// What loading a track puts back to normal on its deck, as settings and
/// `[Controls]` keys of the same name, with their titles and help.
const AUTO_RESETS: [(&str, &str, &str); 3] = [
    (
        "auto_reset_fx",
        "Effects",
        "The deck's effects switch off. Their beats and dry/wet stay as you set them.",
    ),
    (
        "auto_reset_eq",
        "EQ and filter",
        "EQ, kills, filter and sculpt go back to the middle.",
    ),
    (
        "auto_reset_gain",
        "Gain",
        "The gain knob goes back to 0 dB; auto gain still levels each track.",
    ),
];

fn key_mixing() -> key::Mixing {
    key::Mixing::from_id(settings::number("key_mixing", 0.0) as u8)
}

/// How far tracks are shifted to match keys on load: 0 (off), 1 or 2
/// semitones.
pub fn key_match_range() -> u8 {
    settings::number("key_match_on_load", 1.0).clamp(0.0, 2.0) as u8
}

fn key_detector() -> key::Detector {
    let id = settings::number("key_detector", 0.0) as u8;
    key::Detector::from_id(id).unwrap_or_default()
}

fn f(on: bool) -> f64 {
    if on {
        1.0
    } else {
        0.0
    }
}

fn bpm_range() -> (f64, f64) {
    let lo = settings::number("bpm_min", analysis::DEFAULT_BPM_RANGE.0);
    let hi = settings::number("bpm_max", analysis::DEFAULT_BPM_RANGE.1);
    (lo, hi)
}

/// Setting up an import from another DJ app, on the Library page: the step
/// shown and the choices so far, saved when the DJ presses Import.
struct WizardState {
    importer: String,
    /// 0 finds the database, 1 picks the lists, 2 says whose tags win.
    step: usize,
    database: Option<PathBuf>,
    /// What the database holds, or why it can't be read.
    preview: String,
    /// Whether `database` could be read; Next waits for one that can.
    readable: bool,
    groups: Vec<&'static str>,
    /// The lists to tick, read when step 1 is shown.
    picker: Option<Picker>,
    /// Opened from Settings to change which lists come in: only step 1,
    /// saved by its button.
    only_lists: bool,
    theirs_win: bool,
}

impl WizardState {
    fn new(importer: &dyn LibraryImport) -> Self {
        let mut w = WizardState {
            importer: importer.id().to_string(),
            step: 0,
            database: None,
            preview: String::new(),
            readable: false,
            groups: settings::import_groups(importer),
            picker: None,
            only_lists: false,
            theirs_win: settings::import_theirs_win(importer.id()),
        };
        w.set_database(importer, settings::import_database(importer));
        w
    }

    fn set_database(&mut self, importer: &dyn LibraryImport, database: Option<PathBuf>) {
        self.preview = match &database {
            None => format!(
                "No {} library was found where it is usually kept. Choose its database file.",
                importer.title()
            ),
            Some(db) => match importer.preview(db) {
                Ok(p) if p.is_empty() => String::new(),
                Ok(p) => format!("Found {p}"),
                Err(e) => format!("Can't read it: {e}"),
            },
        };
        self.readable = !self.preview.starts_with("Can't");
        self.database = database;
        self.picker = None;
    }

    /// Reads the lists to tick, if they haven't been yet.
    fn read_lists(&mut self, importer: &dyn LibraryImport) {
        if self.picker.is_some() {
            return;
        }
        let Some(db) = &self.database else {
            return;
        };
        let choices = settings::import_list_choices(importer.id());
        match Picker::read(importer, db, &self.groups, &choices) {
            Ok(p) => self.picker = Some(p),
            Err(e) => {
                self.preview = format!("Can't read its lists: {e}");
                self.picker = Some(Picker::default());
            }
        }
    }

    /// Saves which lists come in, from the ticks.
    fn save_lists(&self, importer: &dyn LibraryImport) {
        let Some(p) = &self.picker else {
            return;
        };
        let id = importer.id();
        let groups = p.groups();
        for g in importer.list_groups() {
            settings::set(
                &settings::import_group_key(id, g.id),
                json!(groups.contains(&g.id)),
            );
        }
        settings::set(&settings::import_lists_key(id), json!(p.choices()));
    }
}

pub struct Prefs {
    app: slint::Weak<AppWindow>,
    registry: Arc<ControlRegistry>,
    controllers: Option<Rc<Controllers>>,
    /// Updates the library from the music folder.
    rescan: Box<dyn Fn()>,
    recorder: Recorder,
    /// Paths picked in the system's file dialogs, by setting.
    chosen_tx: Sender<(String, PathBuf)>,
    chosen_rx: Receiver<(String, PathBuf)>,
    /// What the last Rescan or Clear did, shown on its row.
    library_note: String,
    /// Whether the Clean-up rules on the Health tab are open.
    cleanup_open: bool,
    /// What the last import change set going, shown on the Import tab.
    import_note: String,
    /// The Manage menu's clean-up waiting for a yes, by its place in
    /// [`STORAGE_ACTIONS`].
    storage_pending: Option<usize>,
    /// What the last clean-up freed.
    storage_note: String,
    /// The devices shown on the controllers page.
    ports: Vec<Port>,
    /// Plugins with settings, shown on Settings › Sources.
    plugins: Vec<Box<dyn Plugin>>,
    /// Whether each of `plugins` plays from a cache: the DJ's own server.
    plugin_plays: Vec<bool>,
    /// The source open on Settings › Sources, by its place in the rail.
    open_source: Option<i32>,
    /// What the last Clear or Sign out on Settings › Sources did.
    sources_note: String,
    /// The rail's sources by name, in its order (see
    /// [`crate::browser::all_source_names`]).
    rail_names: Vec<String>,
    /// Tells the rest of the app a plugin's settings changed, by its id.
    plugin_changed: Box<dyn Fn(&str)>,
    /// Opens the outputs again with the saved audio settings.
    audio_changed: Box<dyn Fn()>,
    /// Opens the inputs again with the saved audio settings, and tells
    /// each input's state (see [`opendj_engine::AudioEngine::input_status`]).
    inputs: InputHooks,
    /// The inputs table's rows last shown, and what each row's menus
    /// offered: the devices ("" for None), and the first channel of each
    /// choice and whether it is mono.
    input_menus: Vec<InputMenu>,
    /// Auxiliary rows after the first that + added and that have no device
    /// yet: 0 to 2.
    aux_added: usize,
    /// The import being set up, if any.
    wizard: Option<WizardState>,
    /// The import whose Remove was pressed once, waiting for a second press.
    removing: Option<String>,
    /// What the last change of a shortcut's key did, shown on the page.
    shortcut_note: String,
    /// CLAP, VST3 and AU effects: folders, scan and the FX lists.
    audio_plugins: Rc<RefCell<AudioPlugins>>,
    /// How the plugin list on Settings › Plugins is narrowed.
    plugin_view: PluginView,
    /// The page and tab the rows were last filled for, so a refill of the
    /// same view changes rows in place and the search box keeps its focus.
    filled: Option<(i32, usize)>,
    /// Counts fills of the rows, so the tour's setup dialog, which shows
    /// some of the same rows, knows to fill its own again.
    pub generation: u64,
    /// Broadcasting, when the engine could give it the master.
    broadcast: Option<Rc<Broadcast>>,
    /// The station open for changes on Settings › Broadcast.
    editing_station: Option<usize>,
    /// What each station's Test found, by its id.
    station_notes: std::collections::HashMap<String, String>,
    tested_tx: Sender<(String, String)>,
    tested_rx: Receiver<(String, String)>,
}

/// One inputs-table row's menus: its role, the devices ("" for None), and
/// the first channel of each choice and whether it is mono.
type InputMenu = (InputRole, Vec<String>, Vec<(u16, bool)>);

/// What the settings window needs from the engine for its inputs.
pub struct InputHooks {
    pub changed: Box<dyn Fn()>,
    pub status: Box<dyn Fn() -> Vec<String>>,
}

impl Prefs {
    #[allow(clippy::too_many_arguments)]
    pub fn install(
        app: &AppWindow,
        registry: Arc<ControlRegistry>,
        controllers: Option<Rc<Controllers>>,
        recorder: Recorder,
        rescan: Box<dyn Fn()>,
        plugin_changed: Box<dyn Fn(&str)>,
        audio_changed: Box<dyn Fn()>,
        inputs: InputHooks,
        audio_plugins: Rc<RefCell<AudioPlugins>>,
        broadcast: Option<Rc<Broadcast>>,
    ) -> Rc<RefCell<Prefs>> {
        let (chosen_tx, chosen_rx) = mpsc::channel();
        let (tested_tx, tested_rx) = mpsc::channel();
        let mut plugins: Vec<Box<dyn Plugin>> = crate::stores::configured()
            .into_iter()
            .filter(|p| !p.settings().is_empty())
            .collect();
        let plugin_plays = plugins
            .iter_mut()
            .map(|p| p.source().is_some_and(|s| s.plays()))
            .collect();
        let g = app.global::<Preferences>();
        let prefs = Rc::new(RefCell::new(Prefs {
            app: app.as_weak(),
            registry,
            controllers,
            rescan,
            recorder,
            chosen_tx,
            chosen_rx,
            library_note: String::new(),
            cleanup_open: false,
            import_note: String::new(),
            storage_pending: None,
            storage_note: String::new(),
            ports: Vec::new(),
            plugins,
            plugin_plays,
            open_source: None,
            sources_note: String::new(),
            rail_names: crate::browser::all_source_names(),
            plugin_changed,
            audio_changed,
            inputs,
            input_menus: Vec::new(),
            aux_added: 0,
            wizard: None,
            removing: None,
            shortcut_note: String::new(),
            audio_plugins,
            plugin_view: PluginView::default(),
            filled: None,
            generation: 0,
            broadcast,
            editing_station: None,
            station_notes: Default::default(),
            tested_tx,
            tested_rx,
        }));
        let p = prefs.clone();
        g.on_changed(move || p.borrow_mut().shown());
        let audio = app.global::<AudioSettings>();
        let p = prefs.clone();
        audio.on_set_rate(move |i| p.borrow_mut().picked("sample_rate", i.max(0) as usize));
        let p = prefs.clone();
        audio.on_set_buffer(move |i| p.borrow_mut().picked("buffer_frames", i.max(0) as usize));
        let p = prefs.clone();
        g.on_toggled(move |id, on| p.borrow_mut().toggled(&id, on));
        let p = prefs.clone();
        g.on_picked(move |id, i| p.borrow_mut().picked(&id, i.max(0) as usize));
        let p = prefs.clone();
        g.on_pressed(move |id| p.borrow_mut().pressed(&id));
        let p = prefs.clone();
        g.on_typed(move |id, text| p.borrow_mut().typed(&id, &text));
        let p = prefs.clone();
        g.on_key_captured(move |id, text, cmd, alt, shift| {
            p.borrow_mut().key_captured(&id, &text, cmd, alt, shift)
        });
        let inputs = app.global::<InputSettings>();
        let p = prefs.clone();
        inputs.on_set_device(move |row, i| {
            p.borrow_mut()
                .input_picked(row.max(0) as usize, Some(i.max(0) as usize), None)
        });
        let p = prefs.clone();
        inputs.on_set_channel(move |row, i| {
            p.borrow_mut()
                .input_picked(row.max(0) as usize, None, Some(i.max(0) as usize))
        });
        let p = prefs.clone();
        inputs.on_add(move || p.borrow_mut().input_added());
        let p = prefs.clone();
        inputs.on_remove(move |row| p.borrow_mut().input_removed(row.max(0) as usize));
        prefs
    }

    /// Called every frame: picks up chosen paths and plugged-in devices.
    pub fn poll(&mut self) {
        let mut tested = crate::storage::take_fresh();
        while let Ok((id, note)) = self.tested_rx.try_recv() {
            self.station_notes.insert(id, note);
            tested = true;
        }
        if tested {
            self.refresh();
        }
        while let Ok((id, path)) = self.chosen_rx.try_recv() {
            if id == "keyboard_set_import" {
                let file = path
                    .file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.shortcut_note = match opendj_controllers::import::import_file(&path) {
                    Ok(i) if !i.shortcuts.is_empty() => {
                        let set = shortcuts::import_set(&file, &i.shortcuts);
                        let mut note = format!("{set}: {}", i.summary());
                        if !i.skipped_shortcuts.is_empty() {
                            let keys: Vec<String> = i
                                .skipped_shortcuts
                                .iter()
                                .map(|(k, _)| shortcuts::display(k))
                                .collect();
                            note.push_str(&format!(" Not done yet: {}.", keys.join(", ")));
                        }
                        note
                    }
                    Ok(_) => format!("{file} has no keyboard shortcuts in it."),
                    Err(e) => e,
                };
                self.refresh();
                continue;
            }
            if id == "import_wizard_db" {
                if let Some(w) = &mut self.wizard {
                    if let Some(i) = importer(&w.importer) {
                        w.set_database(i.as_ref(), Some(path));
                    }
                }
                self.refresh();
                continue;
            }
            if id.starts_with("import_database:") {
                settings::set(&id, json!(path.to_string_lossy()));
                (self.rescan)();
                self.library_note = "Updating what comes in from other apps…".into();
                self.import_note = self.library_note.clone();
                self.refresh();
                continue;
            }
            if id.starts_with("plugin:") {
                self.typed(&id, &path.to_string_lossy());
                self.refresh();
                continue;
            }
            if id == "audio_plugin_folder_add" {
                let p = self.audio_plugins.borrow();
                let mut folders = p.folders();
                if !folders.contains(&path) {
                    folders.push(path);
                }
                p.set_folders(&folders);
                drop(p);
                self.refresh();
                continue;
            }
            if id == "music_folder_add" {
                let mut folders = settings::music_folders();
                if !folders.contains(&path) {
                    folders.push(path);
                }
                settings::set_music_folders(&folders);
                self.music_folders_changed();
                continue;
            }
            if id == "stage_inbox_add" {
                let mut inbox = settings::paths(settings::STAGE_INBOX);
                if !inbox.contains(&path) {
                    inbox.push(path);
                }
                let inbox: Vec<String> = inbox
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect();
                settings::set(settings::STAGE_INBOX, json!(inbox));
                self.stages_changed();
                continue;
            }
            if id == settings::STAGE_TRIAGE || id == settings::STAGE_COLLECTION {
                settings::set(&id, json!(path.to_string_lossy()));
                self.stages_changed();
                continue;
            }
            if id == settings::ALBUMS_FOLDER || id == settings::MIXED_SETS_FOLDER {
                settings::set(&id, json!(path.to_string_lossy()));
                self.recorder.configure(settings::recording());
                self.music_folders_changed();
                continue;
            }
            settings::set(&id, json!(path.to_string_lossy()));
            if id == "recording_folder" {
                self.recorder.configure(settings::recording());
            } else {
                self.library_note = "Rescan to bring in tracks from there.".into();
            }
            self.refresh();
        }
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<Preferences>();
        if self.audio_plugins.borrow_mut().poll() && g.get_open() && g.get_page() == AUDIO_PLUGINS {
            self.refresh();
        }
        // Help greys a controller's tutorial until it is plugged in.
        if g.get_open()
            && (g.get_page() == CONTROLLERS || g.get_page() == FEEDBACK)
            && g.get_query().is_empty()
        {
            let ports = self
                .controllers
                .as_ref()
                .map(|c| c.ports())
                .unwrap_or_default();
            if ports != self.ports {
                self.ports = ports;
                self.refresh();
            }
        }
    }

    /// The + or − under a list of folders, or a folder's reveal arrow. The
    /// + opens the folder picker under the id the picked folder is handled by.
    fn folder_list_pressed(&mut self, id: &str) -> bool {
        let Some((list, what)) = id.split_once('#') else {
            return false;
        };
        if let Some(path) = what.strip_prefix("reveal:") {
            crate::files::reveal(Path::new(path));
            return true;
        }
        let remove = what
            .strip_prefix("remove:")
            .and_then(|i| i.parse::<usize>().ok());
        if what != "add" && remove.is_none() {
            return false;
        }
        let single = [
            settings::ALBUMS_FOLDER,
            settings::MIXED_SETS_FOLDER,
            settings::STAGE_TRIAGE,
            settings::STAGE_COLLECTION,
        ];
        if list.starts_with("import_database:") {
            match remove {
                None => {
                    self.pressed(list);
                }
                Some(_) => {
                    settings::set(list, json!(""));
                    self.refresh();
                }
            }
            return true;
        }
        match (list, remove) {
            (MUSIC_FOLDERS_LIST, None) => self.pressed("music_folder_add"),
            (MUSIC_FOLDERS_LIST, Some(i)) => self.pressed(&format!("music_folder_remove:{i}")),
            (settings::STAGE_INBOX, None) => self.pressed("stage_inbox_add"),
            (settings::STAGE_INBOX, Some(i)) => self.pressed(&format!("stage_inbox_remove:{i}")),
            (PLUGIN_FOLDERS_LIST, None) => self.pressed("audio_plugin_folder_add"),
            (PLUGIN_FOLDERS_LIST, Some(i)) => {
                self.pressed(&format!("audio_plugin_folder_remove:{i}"))
            }
            (list, None) if single.contains(&list) => self.pressed(list),
            (list, Some(_)) if single.contains(&list) => {
                settings::set(list, json!(""));
                if list == settings::STAGE_TRIAGE || list == settings::STAGE_COLLECTION {
                    self.stages_changed();
                } else {
                    self.recorder.configure(settings::recording());
                    self.music_folders_changed();
                }
            }
            _ => return false,
        }
        true
    }

    /// A music folder was added or removed: the library is updated, which
    /// brings in what is there and finds what is now outside.
    fn music_folders_changed(&mut self) {
        (self.rescan)();
        crate::storage::forget();
        // Plugins are told the music folders with their settings.
        for plugin in &mut self.plugins {
            let id = plugin.manifest().id;
            plugin.configure(&settings::plugin_config(id));
            (self.plugin_changed)(id);
        }
        self.library_note =
            "Updating… new tracks appear in your collection as they are found.".into();
        self.refresh();
    }

    /// The stage folders changed: the library is updated, which finds what
    /// is in them and gives tracks their stage.
    fn stages_changed(&mut self) {
        (self.rescan)();
        self.library_note = "Updating… tracks in the stage folders get their stage.".into();
        self.refresh();
    }

    /// A page was opened or the search changed.
    fn shown(&mut self) {
        if let Some(app) = self.app.upgrade() {
            app.global::<AudioSettings>().invoke_refresh();
        }
        self.storage_pending = None;
        self.storage_note.clear();
        self.sources_note.clear();
        self.shortcut_note.clear();
        self.refresh();
    }

    /// Fills the rows again for what is shown.
    fn refresh(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<Preferences>();
        let query = g.get_query().to_lowercase();
        let pages = g.get_pages();
        let searching = !query.trim().is_empty();
        let tabs = if searching {
            Vec::new()
        } else {
            self.tabs(g.get_page())
        };
        let tab = (g.get_tab().max(0) as usize).min(tabs.len().saturating_sub(1));
        g.set_tab(tab as i32);
        g.set_tabs(strings(&tabs));
        let items: Vec<PrefItem> = if !searching {
            self.page(&app, g.get_page(), Some(tab))
        } else {
            let words: Vec<&str> = query.split_whitespace().collect();
            (AUDIO..FEEDBACK + 1)
                .flat_map(|p| {
                    let name = slint::Model::row_data(&pages, p as usize).unwrap_or_default();
                    self.page(&app, p, None).into_iter().map(move |mut item| {
                        item.page_name = name.to_uppercase().into();
                        item
                    })
                })
                .filter(|item| item.kind != NOTE && item.kind != HEADING && !item.title.is_empty())
                .filter(|item| {
                    let text =
                        format!("{} {} {}", item.title, item.detail, item.page_name).to_lowercase();
                    words.iter().all(|w| text.contains(w))
                })
                .collect()
        };
        let view = (!searching).then_some((g.get_page(), tab));
        // On the plugin list, typing in its search box fills the rows
        // again: change them in place so the box isn't made anew.
        let in_place = view.is_some()
            && view == self.filled
            && matches!(view, Some((AUDIO_PLUGINS, t)) if t != FOLDERS_TAB);
        if !in_place || !patch_rows(&g.get_items(), &items) {
            g.set_items(ModelRc::new(VecModel::from(items)));
        }
        self.filled = view;
        show_wizard(&app, self.wizard.as_ref());
        self.generation += 1;
    }

    /// The rows of a part of the tour's setup dialog: the settings it asks
    /// about, as on their pages. `heard` says what the controller check
    /// last heard ("" for nothing yet).
    pub fn tour_items(&mut self, part: Part, heard: &str) -> Vec<PrefItem> {
        let Some(app) = self.app.upgrade() else {
            return Vec::new();
        };
        match part {
            Part::Audio => {
                let mut items: Vec<PrefItem> = self
                    .page(&app, AUDIO, None)
                    .into_iter()
                    .filter(|i| i.id == "outputs" || i.id == "levels")
                    .collect();
                items.push(PrefItem {
                    action: "Play".into(),
                    ..item(
                        "tour_tone:1",
                        BUTTON,
                        "Test the speakers",
                        "A short beep in the master.",
                    )
                });
                items.push(PrefItem {
                    action: "Play".into(),
                    ..item(
                        "tour_tone:2",
                        BUTTON,
                        "Test the headphones",
                        "A short beep in the headphones.",
                    )
                });
                items.push(item(
                    "",
                    NOTE,
                    "",
                    "Controller with a sound card, such as the DDJ-FLX4: pick it for both, on \
                     channels 1 & 2 and 3 & 4.",
                ));
                items
            }
            Part::Controller => {
                let mut items = vec![item(
                    "",
                    NOTE,
                    "",
                    if heard.is_empty() {
                        "Press a button or move a knob to check it's working."
                    } else {
                        heard
                    },
                )];
                items.extend(self.controllers_page());
                items
            }
            Part::Library => {
                let mut items = self.music_folder_items();
                items.push(item("", HEADING, "IMPORT", ""));
                let mut found = false;
                let importers = crate::stores::importers();
                for i in &importers {
                    if settings::import_database(i.as_ref()).is_none() {
                        continue;
                    }
                    found = true;
                    let title = i.title();
                    if settings::import_enabled(i.id()) {
                        items.push(item(
                            "",
                            NOTE,
                            &format!("Importing from {title}"),
                            "Kept up to date.",
                        ));
                    } else {
                        items.push(PrefItem {
                            action: "Set up…".into(),
                            ..item(
                                &format!("import_start:{}", i.id()),
                                BUTTON,
                                &format!("Import from {title}"),
                                "Tracks, cues, grids and lists.",
                            )
                        });
                    }
                }
                for app_name in crate::tour::other_dj_apps() {
                    if importers.iter().any(|i| i.title() == app_name) {
                        continue;
                    }
                    found = true;
                    items.push(PrefItem {
                        value: "Coming soon".into(),
                        ..item(
                            "",
                            BUTTON,
                            &format!("Import from {app_name}"),
                            "Found on this computer.",
                        )
                    });
                }
                if !found {
                    items.push(item(
                        "",
                        NOTE,
                        "",
                        "No other DJ app found. Settings › Collection › Import has them later.",
                    ));
                }
                if !self.library_note.is_empty() {
                    items.push(item("", NOTE, "", &self.library_note));
                }
                items
            }
            Part::Mixing => Vec::new(),
        }
    }

    /// A page's rows: of one of its tabs, or of all of them for a search
    /// (`tab` None).
    fn page(&mut self, app: &AppWindow, page: i32, tab: Option<usize>) -> Vec<PrefItem> {
        match page {
            AUDIO => vec![
                {
                    // Sample rate and buffer show as columns of the outputs
                    // table; their notes go under it.
                    let rate = self.sample_rate_item();
                    let buffer = self.buffer_item();
                    let g = app.global::<AudioSettings>();
                    g.set_rates(rate.choices.clone());
                    g.set_rate(rate.chosen);
                    g.set_buffers(buffer.choices.clone());
                    g.set_buffer(buffer.chosen);
                    item(
                        "outputs",
                        OUTPUTS,
                        "Outputs",
                        &format!("{} {}", rate.detail, self.audio_health()),
                    )
                },
                item("inputs", HEADING, "INPUTS", ""),
                self.inputs_item(),
                PrefItem {
                    choices: strings(&["Off", "Auto", "Manual"]),
                    chosen: settings::number("talkover_ducking", DUCK_AUTO) as i32,
                    ..item(
                        "talkover_ducking",
                        SEGMENTED,
                        "Talkover ducking",
                        "Auto turns the music down while you talk into the mic; \
                         Manual whenever the mic is on.",
                    )
                },
                {
                    let strength = settings::number("duck_strength", DEFAULT_DUCK_STRENGTH);
                    let names: Vec<String> = DUCK_DB.iter().map(|d| format!("-{d} dB")).collect();
                    PrefItem {
                        choices: strings(&names),
                        chosen: DUCK_DB
                            .iter()
                            .position(|&d| (duck_strength(d) - strength).abs() < 1e-3)
                            .unwrap_or(1) as i32,
                        ..item(
                            "duck_strength",
                            MENU,
                            "Ducking depth",
                            "How far the music goes down under the mic.",
                        )
                    }
                },
                item("recording", HEADING, "RECORDING", ""),
                {
                    let r = settings::recording();
                    PrefItem {
                        choices: strings(&RECORD_FORMATS.map(|f| f.2)),
                        chosen: RECORD_FORMATS
                            .iter()
                            .position(|f| (f.0, f.1) == (r.format, r.bits))
                            .unwrap_or(0) as i32,
                        ..item(
                            "recording_format",
                            MENU,
                            "Format",
                            "FLAC is lossless and about half the size of WAV.",
                        )
                    }
                },
                // The same folder as Recordings and Mixed Sets in Settings ›
                // Collection: recordings go in a folder inside it.
                folder_list(
                    settings::MIXED_SETS_FOLDER,
                    "Recordings",
                    &format!(
                        "Saved in the \"{}\" folder inside your Recordings and Mixed Sets \
                         folder, the same one as in Settings › Collection. Each mix comes with \
                         a .cue tracklist.",
                        settings::RECORDINGS_SUBFOLDER
                    ),
                    &settings::mixed_sets_folder().into_iter().collect::<Vec<_>>(),
                    true,
                    false,
                ),
            ],
            CONTROLLERS => self.controllers_page(),
            LIBRARY => self.library_page(tab),
            ANALYSIS => {
                let (lo, hi) = bpm_range();
                let names: Vec<String> = BPM_RANGES
                    .iter()
                    .map(|&(a, b)| {
                        let default = (a, b) == analysis::DEFAULT_BPM_RANGE;
                        format!("{a} – {b} BPM{}", if default { " (default)" } else { "" })
                    })
                    .collect();
                let chosen = BPM_RANGES.iter().position(|&r| r == (lo, hi)).unwrap_or(1);
                let camelot = app.global::<KeyNotation>().get_camelot();
                vec![
                    PrefItem {
                        choices: strings(&names),
                        chosen: chosen as i32,
                        ..item(
                            "bpm_range",
                            MENU,
                            "BPM range",
                            "Where tempos are looked for in newly analysed tracks.",
                        )
                    },
                    PrefItem {
                        choices: strings(&["Standard", "Beat This!"]),
                        chosen: i32::from(settings::flag("neural_beats", true)),
                        ..item(
                            "beat_detector",
                            SEGMENTED,
                            "Beat detection",
                            "Beat This! (an open-source neural network) finds downbeats, \
                             so bar 1 starts on one. \
                             It takes a few seconds more per track, after the waveform. \
                             Edited and imported grids stay.",
                        )
                    },
                    PrefItem {
                        choices: strings(&["Standard", "Neural"]),
                        chosen: key_detector().id() as i32,
                        ..item(
                            "key_detector",
                            SEGMENTED,
                            "Key detection",
                            "Neural is more accurate, mostly on electronic music, but slower. \
                             Keys from tags stay.",
                        )
                    },
                    PrefItem {
                        choices: strings(&[
                            "Camelot · 8A",
                            "Open Key · 1m",
                            "Sharps · A#m",
                            "Flats · Bbm",
                        ]),
                        chosen: match (camelot, key::notation()) {
                            (true, key::Notation::OpenKey) => 1,
                            (true, _) => 0,
                            (false, key::Notation::Flats) => 3,
                            (false, _) => 2,
                        },
                        ..item(
                            "key_notation",
                            MENU,
                            "Key notation",
                            "How keys are written on the decks and in the track list. \
                             Clicking a deck's key switches between the wheel and notes. \
                             Keys in your files' tags stay as they are.",
                        )
                    },
                ]
                .into_iter()
                .chain(self.stems_items())
                .collect()
            }
            DECKS => {
                let range = settings::number("rate_range", DEFAULT_RATE_RANGE);
                let names: Vec<String> = RATE_RANGES
                    .iter()
                    .map(|r| format!("±{}%", (r * 100.0).round()))
                    .collect();
                let cue = settings::number("cue_mode", CUE_MODES[0]);
                vec![
                    PrefItem {
                        choices: strings(&["Hold to preview", "Set and return"]),
                        chosen: CUE_MODES.iter().position(|&m| m == cue).unwrap_or(0) as i32,
                        ..item(
                            "cue_mode",
                            SEGMENTED,
                            "CUE button",
                            "Hold to preview plays while CUE is held, as on Pioneer players.",
                        )
                    },
                    PrefItem {
                        choices: strings(&names),
                        chosen: RATE_RANGES
                            .iter()
                            .position(|&r| (r - range).abs() < 1e-6)
                            .unwrap_or(2) as i32,
                        ..item(
                            "rate_range",
                            MENU,
                            "Tempo fader range",
                            "How far the tempo fader changes the speed, either way.",
                        )
                    },
                    PrefItem {
                        choices: strings(&["Up slows down", "Up speeds up"]),
                        chosen: if settings::flag("tempo_up_faster", false) { 1 } else { 0 },
                        ..item(
                            "tempo_direction",
                            SEGMENTED,
                            "Tempo fader direction",
                            "Which way the tempo faders, on screen and on controllers, speed \
                             the deck up. Pioneer players slow down as the fader goes up.",
                        )
                    },
                    switch(
                        "keylock_default",
                        "Key lock on at start",
                        "Tempo changes keep the key; each deck's lock button still switches it.",
                        settings::flag("keylock_default", false),
                    ),
                    PrefItem {
                        choices: strings(&["Harmonic · 4 keys", "Fuzzy · 6 keys"]),
                        chosen: key_mixing().id() as i32,
                        ..item(
                            "key_mixing",
                            SEGMENTED,
                            "Key mixing",
                            "Which keys the track list marks in lilac as mixing with the track \
                             playing. Harmonic: the same number, or the next one with the same \
                             letter (8A: 8A, 8B, 7A, 9A). Fuzzy ignores the letter, so 7B and 9B \
                             mix too.",
                        )
                    },
                    PrefItem {
                        choices: strings(&["Off", "±1 semitone", "±2 semitones"]),
                        chosen: key_match_range() as i32,
                        ..item(
                            "key_match_on_load",
                            SEGMENTED,
                            "Match key on load",
                            "A track loaded while another deck plays is shifted up or down to \
                             mix with it, with key lock on. The track list marks those tracks with \
                             the shift they'd get.",
                        )
                    },
                    PrefItem {
                        choices: strings(&["Off", "Beats", "Beats and phrases"]),
                        chosen: beat_counter_mode(),
                        ..item(
                            "beat_counter",
                            SEGMENTED,
                            "Beat counter",
                            "Squares in each deck header light up beat by beat, the first of \
                             the bar brighter, with the bars of the phrase under them. With \
                             phrases, the header also counts down the bars to the next phrase \
                             and names it.",
                        )
                    },
                    switch(
                        "quantize",
                        "Quantize",
                        "Decks start with quantize on: cues, loops, play and jumps land on the beat. \
                         Right-click a deck's SYNC to switch it for that deck.",
                        settings::flag("quantize", true),
                    ),
                    switch(
                        "vinyl_mode",
                        "Vinyl mode",
                        "Touching the top of a controller's jog grabs the record to scratch. \
                         Off, the top bends the tempo like the rim, as CDJ mode does.",
                        settings::flag("vinyl_mode", true),
                    ),
                    switch(
                        "protect_playing",
                        "Protect playing decks",
                        "A track can't be loaded onto a deck while it plays.",
                        settings::flag("protect_playing", true),
                    ),
                    switch(
                        "pfl_on_load",
                        "Headphones follow the loaded deck",
                        "Loading a track cues its deck in the headphones and takes the other decks out of the cue, like VirtualDJ. What the room hears doesn't change, and automix leaves the cue alone.",
                        settings::flag("pfl_on_load", true),
                    ),
                    item("auto_reset", HEADING, "AUTO RESET ON LOAD", ""),
                ]
                .into_iter()
                .chain(AUTO_RESETS.map(|(id, title, detail)| {
                    switch(id, title, detail, settings::flag(id, true))
                }))
                .collect()
            }
            MIXER => self.mixer_general(),
            SOURCES => self.sources_page(tab.is_none()),
            AUDIO_PLUGINS => self.audio_plugin_page(tab),
            BROADCAST => {
                let mut items = self.broadcast_page();
                items.extend(loudness_items());
                items
            }
            KEYBOARD => self.keyboard_page(),
            FEEDBACK => vec![
                item("", HEADING, "GUIDED TOUR", ""),
                PrefItem {
                    action: "Start".into(),
                    ..item(
                        "tour_start:-1",
                        BUTTON,
                        "Take the tour",
                        "Sets up your sound, controller and music, then mixes two tracks with \
                         you, step by step.",
                    )
                },
                PrefItem {
                    choices: strings(&crate::tour::PART_NAMES),
                    value: "Choose…".into(),
                    ..item(
                        "tour_part",
                        MENU,
                        "Just one part",
                        "Run one part of the tour again.",
                    )
                },
            ]
            .into_iter()
            .chain(self.tutorial_items())
            .chain([
                item("", HEADING, "FEEDBACK", ""),
                item(
                    "",
                    NOTE,
                    "",
                    &format!(
                        "openDJ {} is a beta. Problems, ideas and comments all help; \
                         each link opens a form on GitHub with your version filled in.",
                        crate::diagnostics::VERSION
                    ),
                ),
                PrefItem {
                    action: "Report…".into(),
                    ..item(
                        "report_bug",
                        BUTTON,
                        "Report a problem",
                        "Something broke, crashed or didn't do what you expected.",
                    )
                },
                PrefItem {
                    action: "Suggest…".into(),
                    ..item(
                        "suggest_feature",
                        BUTTON,
                        "Suggest a feature",
                        "Something you'd like openDJ to do, or do differently.",
                    )
                },
                switch(
                    "update_check",
                    "Check for updates",
                    "Looks for a new version on GitHub when openDJ starts. Nothing is sent but the request.",
                    settings::flag("update_check", true),
                ),
                PrefItem {
                    action: "Show".into(),
                    ..item(
                        "show_logs",
                        BUTTON,
                        "Logs and crash reports",
                        "Attach these to a problem report. They stay on your computer unless you do.",
                    )
                },
            ])
            .collect(),
            _ => Vec::new(),
        }
    }

    /// Help's tutorials: Start for each, ✓ once done, and a controller
    /// tutorial greyed until its controller is plugged in.
    fn tutorial_items(&self) -> Vec<PrefItem> {
        let ports: Vec<String> = self
            .controllers
            .as_ref()
            .map(|c| c.ports().into_iter().map(|p| p.name).collect())
            .unwrap_or_default();
        std::iter::once(item("", HEADING, "TUTORIALS", ""))
            .chain(crate::tutorials::TUTORIALS.iter().map(|t| {
                let plugged = t
                    .controller
                    .is_none_or(|name| ports.iter().any(|p| p.contains(name)));
                let done = crate::tutorials::done(t.id);
                let title = format!("{}{}", t.name, if done { "  ✓" } else { "" });
                let detail = format!("{} About {} minutes.", t.about, t.minutes);
                PrefItem {
                    action: if plugged { "Start" } else { "" }.into(),
                    value: match t.controller {
                        Some(name) if !plugged => format!("Plug in a {name} to start"),
                        _ => String::new(),
                    }
                    .into(),
                    ..item(&format!("tutorial_start:{}", t.id), BUTTON, &title, &detail)
                }
            }))
            .collect()
    }

    fn mixer_general(&self) -> Vec<PrefItem> {
        let target = settings::number("auto_gain_target", DEFAULT_AUTO_GAIN_TARGET);
        let targets: Vec<String> = AUTO_GAIN_TARGETS
            .iter()
            .map(|&t| {
                let note = match t {
                    t if t == DEFAULT_AUTO_GAIN_TARGET => " (default)",
                    -8.0 => " · club master",
                    -23.0 => " · broadcast",
                    _ => "",
                };
                format!("{t} LUFS{note}")
            })
            .collect();
        vec![
            switch(
                "auto_gain",
                "Auto gain",
                "Levels every track to the same loudness, measured when it is analysed. The gain knob still trims on top.",
                settings::flag("auto_gain", true),
            ),
            PrefItem {
                choices: strings(&targets),
                chosen: AUTO_GAIN_TARGETS
                    .iter()
                    .position(|&t| (t - target).abs() < 1e-6)
                    .unwrap_or(3) as i32,
                ..item(
                    "auto_gain_target",
                    MENU,
                    "Auto gain level",
                    "Louder levels leave less headroom before the master limiter.",
                )
            },
            PrefItem {
                choices: strings(&["Smooth", "Linear", "Scratch"]),
                chosen: settings::number("xfader_shape", 0.0) as i32,
                ..item(
                    "xfader_shape",
                    SEGMENTED,
                    "Crossfader curve",
                    "Smooth for blends; Scratch cuts in sharply at the ends.",
                )
            },
            switch(
                "xfader_enabled",
                "Crossfader",
                "Off: the crossfader does nothing and every deck plays through. Right-click the crossfader to switch it too.",
                settings::flag("xfader_enabled", true),
            ),
            switch(
                "xfader_reverse",
                "Reverse crossfader",
                "Hamster style: the left side plays the right deck.",
                settings::flag("xfader_reverse", false),
            ),
            switch(
                "head_split",
                "Split cue",
                "Cue in the left ear and master in the right, both in mono.",
                settings::flag("head_split", false),
            ),
            ]
    }

    /// Settings › Plugins. Effects and Instruments: Scan, then the plugins
    /// by maker, narrowed by format chips and a search, each with a tick.
    /// Folders: where plugins are looked for, Scan, and files that couldn't
    /// be read, with Retry. `tab` None is everything, one row per plugin,
    /// for a search of all of Settings.
    fn audio_plugin_page(&self, tab: Option<usize>) -> Vec<PrefItem> {
        let p = self.audio_plugins.borrow();
        if !p.available() {
            return vec![item(
                "",
                NOTE,
                "Plugins",
                "This copy of openDJ came without its plugin helper (opendj-plugin-host), so CLAP, VST3 and Audio Unit plugins can't run.",
            )];
        }
        let effects = p.effects();
        let instruments = p.instruments();
        let failures = p.failures();
        let shown = p.shown();
        let note = if !p.note.is_empty() {
            p.note.clone()
        } else if effects.is_empty() && instruments.is_empty() {
            "Nothing is looked at until you press Scan.".to_string()
        } else {
            format!(
                "{} effects and {} instruments known from the last scan.",
                effects.len(),
                instruments.len()
            )
        };
        let scan = PrefItem {
            action: if p.scanning() { "" } else { "Scan" }.into(),
            value: if p.scanning() {
                // Just the count: the file's name is in the line below.
                note.split(':').next().unwrap_or_default().into()
            } else {
                "".into()
            },
            ..item("audio_plugins_scan", BUTTON, "Scan for plugins", &note)
        };
        let mut items = Vec::new();
        if tab.is_none_or(|t| t == FOLDERS_TAB) {
            items.push(folder_list(
                PLUGIN_FOLDERS_LIST,
                "Plugin folders",
                "Where openDJ looks for .clap and .vst3 plugins, besides the system's own plugin \
                 folders.",
                &p.folders(),
                false,
                false,
            ));
            items.push(scan.clone());
            if !failures.is_empty() {
                let n = failures.len();
                items.push(PrefItem {
                    action: "Details…".into(),
                    ..item(
                        "audio_plugin_errors",
                        BUTTON,
                        &format!(
                            "{n} {} couldn't be loaded",
                            if n == 1 { "plugin" } else { "plugins" }
                        ),
                        "",
                    )
                });
            }
        }
        let Some(tab) = tab else {
            for e in effects.iter().chain(&instruments) {
                items.push(PrefItem {
                    on: shown.contains(&e.key),
                    ..item(
                        &format!("audio_plugin_show:{}", e.key),
                        TICK,
                        &format!("{} · {} · {}", e.name, e.vendor, e.format),
                        "",
                    )
                });
            }
            return items;
        };
        if tab == FOLDERS_TAB {
            return items;
        }
        let instrument_tab = tab == 1;
        let list = if instrument_tab {
            &instruments
        } else {
            &effects
        };
        items.push(PrefItem {
            detail: if p.scanning() {
                note.clone()
            } else if list.is_empty() {
                format!("{note} Plugin folders are on the Folders tab.")
            } else if instrument_tab {
                "Ticked instruments are offered by the Instrument input in the MIC panel.".into()
            } else {
                "Ticked effects are offered in every FX list: decks, Beat FX, Mic and Aux.".into()
            }
            .into(),
            ..scan
        });
        if list.is_empty() {
            return items;
        }
        let v = &self.plugin_view;
        let formats = plugin_formats(list);
        items.push(PrefItem {
            choices: strings(&formats),
            chosen: formats
                .iter()
                .enumerate()
                .filter(|(_, f)| v.formats.contains(f))
                .map(|(i, _)| 1 << i)
                .sum(),
            value: v.search.as_str().into(),
            ..item(
                "audio_plugin_filter",
                FILTER_BAR,
                "",
                if instrument_tab {
                    "Search instruments and makers"
                } else {
                    "Search effects and makers"
                },
            )
        });
        let groups = plugin_groups(list, &v.formats, &v.search);
        if groups.is_empty() {
            items.push(item("", NOTE, "", "Nothing matches."));
        }
        let searching = !v.search.trim().is_empty();
        for (maker, plugins) in &groups {
            let ticked = plugins.iter().filter(|f| shown.contains(&f.key)).count();
            let open = searching || v.open.contains(maker);
            items.push(PrefItem {
                on: ticked == plugins.len(),
                value: if ticked > 0 && ticked < plugins.len() {
                    "some".into()
                } else {
                    "".into()
                },
                chosen: 1,
                action: if open { "▾" } else { "▸" }.into(),
                ..item(
                    &format!("audio_plugin_maker:{maker}"),
                    TICK,
                    maker,
                    &if ticked > 0 {
                        format!("{ticked} of {} ticked", plugins.len())
                    } else {
                        plugins.len().to_string()
                    },
                )
            });
            if !open {
                continue;
            }
            for f in plugins {
                items.push(PrefItem {
                    on: shown.contains(&f.key),
                    chosen: 2,
                    ..item(
                        &format!("audio_plugin_show:{}", f.key),
                        TICK,
                        &f.name,
                        f.format,
                    )
                });
            }
        }
        items
    }

    /// The plugin list shown: instruments on Settings › Plugins'
    /// Instruments tab, otherwise effects.
    fn plugin_list(&self) -> Vec<Found> {
        let tab = self
            .app
            .upgrade()
            .map_or(0, |a| a.global::<Preferences>().get_tab());
        let p = self.audio_plugins.borrow();
        if tab == 1 {
            p.instruments()
        } else {
            p.effects()
        }
    }

    fn running_rate(&self) -> f64 {
        self.registry
            .get("[App]", "samplerate")
            .map_or(44_100.0, |c| c.get())
    }

    fn sample_rate_item(&self) -> PrefItem {
        let running = self.running_rate();
        let chosen = settings::audio_setup().sample_rate;
        let mut names = vec!["Device default".to_string()];
        names.extend(
            SAMPLE_RATES
                .iter()
                .map(|&r| format!("{} kHz", r as f64 / 1000.0)),
        );
        let detail = match chosen {
            Some(r) if r as f64 != running => format!(
                "Runs at {} kHz now; {} kHz from the next start, if the device plays it.",
                running / 1000.0,
                r as f64 / 1000.0
            ),
            _ => format!(
                "Runs at {} kHz now. A change applies the next time openDJ starts.",
                running / 1000.0
            ),
        };
        PrefItem {
            choices: strings(&names),
            chosen: chosen
                .and_then(|r| SAMPLE_RATES.iter().position(|&x| x == r))
                .map_or(0, |i| i + 1) as i32,
            ..item("sample_rate", MENU, "Sample rate", &detail)
        }
    }

    /// The auxiliary rows after the first that the inputs table shows: those
    /// set up, and those + added.
    fn extra_aux_rows(&self, setup: &AudioSetup) -> usize {
        let has = |role| setup.inputs.iter().any(|r| r.role == role);
        let set_up = if has(InputRole::Auxiliary3) {
            2
        } else if has(InputRole::Auxiliary2) {
            1
        } else {
            0
        };
        set_up.max(self.aux_added).min(2)
    }

    /// The inputs table: fills its rows (see InputsTable in mixer.slint)
    /// and returns the settings row that shows it. The microphone and the
    /// first auxiliary input are always there; + adds up to two more.
    fn inputs_item(&mut self) -> PrefItem {
        let setup = settings::audio_setup();
        let status = (self.inputs.status)();
        let devices = input_devices();
        let extra = self.extra_aux_rows(&setup);
        let roles = &InputRole::ALL[..2 + extra];
        self.input_menus.clear();
        let mut rows = Vec::new();
        for (n, &role) in roles.iter().enumerate() {
            let saved = setup.inputs.iter().find(|r| r.role == role);
            // "None", then each input device.
            let mut names = vec![String::new()];
            let mut shown = vec!["None".to_string()];
            for d in devices.iter().filter(|d| d.channels >= 1) {
                names.push(d.name.clone());
                shown.push(d.name.clone());
            }
            let mut device = 0;
            // Each choice's first channel and whether it is mono: the
            // microphone takes one channel; an auxiliary input a pair, or
            // one.
            let mut starts: Vec<(u16, bool)> = Vec::new();
            if let Some(r) = saved {
                match devices.iter().find(|d| d.name == r.device) {
                    Some(d) => {
                        device = names.iter().position(|n| *n == r.device).unwrap_or(0);
                        starts = channel_choices(role, d.channels);
                    }
                    None => {
                        let name = if r.device.is_empty() {
                            "System default"
                        } else {
                            &r.device
                        };
                        names.push(r.device.clone());
                        shown.push(format!("{name} (not connected)"));
                        device = names.len() - 1;
                        starts = vec![(r.channel, r.mono)];
                    }
                }
            }
            let channel = saved
                .and_then(|r| {
                    starts
                        .iter()
                        .position(|&(c, mono)| c == r.channel && mono == (r.channels() == 1))
                })
                .unwrap_or(0);
            let channels: Vec<String> = if starts.is_empty() {
                vec!["–".to_string()]
            } else {
                starts
                    .iter()
                    .map(|&(c, mono)| {
                        if mono && role.can_be_stereo() {
                            format!("{} · mono", channel_names(c, 1))
                        } else {
                            channel_names(c, if mono { 1 } else { 2 })
                        }
                    })
                    .collect()
            };
            let state = status.get(role.index()).cloned().unwrap_or_default();
            self.input_menus.push((role, names, starts));
            rows.push(InputRow {
                name: match n {
                    0 => "Microphone".to_string(),
                    _ if extra == 0 => "Auxiliary".to_string(),
                    _ => format!("Auxiliary {n}"),
                }
                .into(),
                devices: strings(&shown),
                device: device as i32,
                channels: strings(&channels),
                channel: channel as i32,
                ok: !state.contains("can't") && !state.contains("not "),
                status: if saved.is_some() {
                    state
                } else {
                    String::new()
                }
                .into(),
                removable: n >= 2,
            });
        }
        if let Some(app) = self.app.upgrade() {
            let g = app.global::<InputSettings>();
            g.set_rows(ModelRc::new(VecModel::from(rows)));
            g.set_can_add(extra < 2);
        }
        item(
            "inputs_table",
            INPUTS_TABLE,
            "Microphone and auxiliary",
            "The microphone turns on with MIC; auxiliary inputs mix in with the decks.",
        )
    }

    /// An input's device (`device`) or channels (`channel`) picked in the
    /// inputs table, by their place in its menus.
    fn input_picked(&mut self, row: usize, device: Option<usize>, channel: Option<usize>) {
        let Some((role, names, starts)) = self.input_menus.get(row) else {
            return;
        };
        let role = *role;
        let mut setup = settings::audio_setup();
        let old = setup.inputs.iter().find(|r| r.role == role).cloned();
        let route = match (device, channel) {
            (Some(0), _) => None,
            (Some(d), _) => names.get(d).map(|name| {
                // A one-channel device can only be mono.
                let width = input_devices()
                    .iter()
                    .find(|x| x.name == *name)
                    .map_or(2, |x| x.channels);
                InputRoute {
                    role,
                    device: name.clone(),
                    channel: 0,
                    mono: width < 2,
                }
            }),
            (None, Some(c)) => match (old, starts.get(c)) {
                (Some(r), Some(&(channel, mono))) => Some(InputRoute { channel, mono, ..r }),
                (old, _) => old,
            },
            (None, None) => return,
        };
        setup.inputs.retain(|r| r.role != role);
        setup.inputs.extend(route);
        settings::set_audio_setup(&setup);
        (self.inputs.changed)();
        self.refresh();
    }

    /// + under the inputs table: one more auxiliary row, up to three.
    fn input_added(&mut self) {
        let setup = settings::audio_setup();
        self.aux_added = (self.extra_aux_rows(&setup) + 1).min(2);
        self.refresh();
    }

    /// × on an auxiliary row after the first: forgets that input. The ones
    /// after it move up, so they keep being Auxiliary 2 and 3 in order.
    fn input_removed(&mut self, row: usize) {
        let Some(&(role, ..)) = self.input_menus.get(row) else {
            return;
        };
        if !matches!(role, InputRole::Auxiliary2 | InputRole::Auxiliary3) {
            return;
        }
        let mut setup = settings::audio_setup();
        let extra = self.extra_aux_rows(&setup);
        setup.inputs.retain(|r| r.role != role);
        if role == InputRole::Auxiliary2 {
            for r in &mut setup.inputs {
                if r.role == InputRole::Auxiliary3 {
                    r.role = InputRole::Auxiliary2;
                }
            }
        }
        self.aux_added = extra.saturating_sub(1);
        settings::set_audio_setup(&setup);
        (self.inputs.changed)();
        self.refresh();
    }

    fn buffer_item(&self) -> PrefItem {
        let running = self.running_rate();
        let mut names = vec!["Device default".to_string()];
        names.extend(
            BUFFER_SIZES
                .iter()
                .map(|&n| format!("{n} samples · {:.1} ms", n as f64 / running * 1000.0)),
        );
        let chosen = settings::audio_setup().buffer_frames;
        PrefItem {
            choices: strings(&names),
            chosen: chosen
                .and_then(|n| BUFFER_SIZES.iter().position(|&x| x == n))
                .map_or(0, |i| i + 1) as i32,
            ..item(
                "buffer_frames",
                MENU,
                "Buffer size",
                &format!(
                    "Smaller reacts faster to the controls; larger is safer from dropouts \
                     on a busy computer. {}",
                    self.audio_health()
                ),
            )
        }
    }

    /// How the audio keeps up, from the engine's `[App],audio_latency_*`
    /// controls: the share of each buffer it takes and the dropouts so far.
    fn audio_health(&self) -> String {
        let get = |k: &str| self.registry.get("[App]", k).map_or(0.0, |c| c.get());
        let dropouts = get("audio_latency_overload_count") as u64;
        format!(
            "Audio load {:.0}% now, {:.0}% at most; {}.",
            get("audio_latency_usage"),
            get("audio_latency_peak"),
            match dropouts {
                0 => "no dropouts".to_string(),
                1 => "1 dropout".to_string(),
                n => format!("{n} dropouts"),
            }
        )
    }

    /// Settings › Controllers: the controllers openDJ has a mapping for,
    /// and any others added; the rest wait under Add a controller.
    fn controllers_page(&mut self) -> Vec<PrefItem> {
        let Some(c) = &self.controllers else {
            return vec![item(
                "",
                NOTE,
                "",
                "Controller mappings were not found, so MIDI controllers are off.",
            )];
        };
        self.ports = c.ports();
        let mappings = c.mappings();
        // Every controller plugged in, the computer's own ports (macOS's
        // Network Session) last.
        let mut ports: Vec<&Port> = self.ports.iter().collect();
        ports.sort_by_key(|p| is_system_port(&p.name));
        let quick = quick_maps();
        let mut items: Vec<PrefItem> = ports
            .iter()
            .map(|port| {
                let choice = c.choice(&port.name);
                let matched = c.auto_mapping(&port.name);
                let mut choices = vec![match &matched {
                    Some(n) => format!("Automatic · {n}"),
                    None => "Automatic · none found".to_string(),
                }];
                // An HID controller runs its HID mapping or nothing: MIDI
                // mappings and Quick map don't apply to it.
                if !port.hid {
                    choices.extend(mappings.iter().map(|(n, _)| n.clone()));
                    // Then the ones made with Quick map.
                    choices.extend(quick.iter().map(|m| m.name.clone()));
                }
                let chosen = match &choice {
                    MappingChoice::Auto | MappingChoice::Off => 0,
                    MappingChoice::File(f) => mappings
                        .iter()
                        .position(|(_, m)| m == f)
                        .map_or(0, |i| i + 1),
                    MappingChoice::Own(p) => quick
                        .iter()
                        .position(|m| p.file_stem().is_some_and(|s| *s == *m.id))
                        .map_or(0, |i| i + 1 + mappings.len()),
                };
                let on = choice != MappingChoice::Off;
                // Only what needs the DJ: a problem, or no mapping.
                let detail = match (&port.running, &port.mapping_file) {
                    _ if !on => String::new(),
                    (Some(_), _) => String::new(),
                    _ if port.problem.is_some() => port.problem.clone().unwrap_or_default(),
                    (None, None) => "No mapping found".to_string(),
                    (None, Some(_)) => String::new(),
                };
                // The cog opens the Controller Editor on the mapping it
                // uses, or without one, the mapping wizard.
                let editable = edit_file(c, port).is_some() && on;
                let action = match (editable, &port.mapping_file) {
                    (true, _) => "Edit",
                    (false, None) if on && !port.hid => "Map it",
                    _ => "",
                };
                let file = port.mapping_file.clone().or_else(|| {
                    mappings
                        .iter()
                        .find(|(n, _)| Some(n) == port.running.as_ref())
                        .map(|(_, f)| f.clone())
                });
                PrefItem {
                    choices: strings(&choices),
                    chosen: chosen as i32,
                    on,
                    action: action.into(),
                    value: controller_outline(file.as_deref()).into(),
                    ..item(
                        &format!("controller:{}", port.name),
                        CONTROLLER,
                        short_port_name(&port.name),
                        &detail,
                    )
                }
            })
            .collect();
        if items.is_empty() {
            items.push(item(
                "",
                NOTE,
                "No controllers",
                "Plug in a MIDI or HID controller and it shows up here within a second or so.",
            ));
        }
        items
    }

    /// Settings › Collection: the music folders, as a list with + and −.
    fn music_folder_items(&self) -> Vec<PrefItem> {
        let env = std::env::var_os("OPENDJ_MUSIC").is_some();
        vec![folder_list(
            MUSIC_FOLDERS_LIST,
            "Music Collection",
            if env {
                "Set by OPENDJ_MUSIC for now."
            } else {
                ""
            },
            &settings::music_folders(),
            false,
            env,
        )]
    }

    /// Settings › Collection › Stages: the inbox folders, the triage and
    /// collection folders, and what happens to files between them.
    fn stage_items(&self) -> Vec<PrefItem> {
        let folders = settings::stage_folders();
        let mut items = vec![item(
            "",
            NOTE,
            "",
            "Stages follow a track from new to ready to play. New music lands in the Inbox, \
             you prepare it in In Prep (tags, cover, rating, tempo and key), and it ends up in \
             Play Ready.",
        )];
        items.push(PrefItem {
            chosen: STAGE_ICON_INBOX,
            ..folder_list(
                settings::STAGE_INBOX,
                "Inbox",
                "",
                &folders.inbox,
                true,
                false,
            )
        });
        items.push(PrefItem {
            chosen: STAGE_ICON_PREP,
            ..folder_list(
                settings::STAGE_TRIAGE,
                "In Prep",
                "Its subfolders are prep crates.",
                &folders.triage.into_iter().collect::<Vec<_>>(),
                true,
                false,
            )
        });
        items.push(PrefItem {
            chosen: STAGE_ICON_READY,
            ..folder_list(
                settings::STAGE_COLLECTION,
                "Play Ready",
                "",
                &folders.collection.into_iter().collect::<Vec<_>>(),
                true,
                false,
            )
        });
        items.push(switch(
            settings::MOVE_FILES,
            "Move files",
            "Allow openDJ to move files between stage folders.",
            settings::move_files(),
        ));
        items.push(switch(
            settings::RENAME_TO_TRIAGE,
            "Rename files",
            "Allow openDJ to rename files when moving them to the In Prep folder.",
            settings::rename_to_triage(),
        ));
        // With the Prep layout (v0.6).
        if crate::PREP_OFFERED {
            items.push(item("", HEADING, "PREP CHECKLIST", ""));
            items.push(item(
                "",
                NOTE,
                "",
                "What makes a track ready, in the Prep layout's Checklist tab. Play Ready lights up \
                 when every line is ticked. The app ticks these itself when the track has them:",
            ));
            for (key, name) in settings::PREP_APP_LINES {
                items.push(PrefItem {
                    on: settings::flag(&settings::prep_check_key(key), true),
                    ..item(&settings::prep_check_key(key), TICK, name, "")
                });
            }
            items.push(item(
                "",
                NOTE,
                "",
                "Your own lines, ticked by hand (Option+1–9 in Prep). Clear a line to take it away.",
            ));
            for (i, line) in settings::prep_lines_typed().iter().enumerate() {
                items.push(PrefItem {
                    value: line.as_str().into(),
                    on: false,
                    ..item(
                        &format!("prep_line:{i}"),
                        TEXT,
                        &format!("Line {}", i + 1),
                        "",
                    )
                });
            }
            items.push(PrefItem {
                action: "Add a line".into(),
                ..item(
                    "prep_line_add",
                    BUTTON,
                    "",
                    "Such as \"Vocals checked\" or \"Intro is clean\".",
                )
            });
        }
        items
    }

    /// Settings › Collection › Health: looking tracks up online, and what
    /// Collection Health's clean-up looks for.
    fn health_items(&self, searching: bool) -> Vec<PrefItem> {
        let mut items = vec![
            item("cover_art", HEADING, "ONLINE LOOKUP", ""),
            PrefItem {
                value: settings::discogs_token().into(),
                on: true,
                ..item(
                    "discogs_token",
                    TEXT,
                    "Discogs token",
                    "Adds Discogs to cover search and Look up online. Make one on \
                     discogs.com under Settings › Developers.",
                )
            },
            PrefItem {
                value: settings::acoustid_key().into(),
                on: true,
                ..item(
                    "acoustid_key",
                    TEXT,
                    "AcoustID key",
                    "Recognizes tracks by their sound. A free key from acoustid.org under \
                     My applications.",
                )
            },
        ];
        items.push(PrefItem {
            action: if self.cleanup_open { "open" } else { "closed" }.into(),
            ..item(CLEANUP_FOLD, HEADING, "CLEAN-UP", "")
        });
        if !self.cleanup_open && !searching {
            return items;
        }
        items.push(item(
            "",
            NOTE,
            "",
            "What Collection Health's \"Junk in names\" looks for. Clean up shows every change before \
             it is made.",
        ));
        for (key, name, about, default) in opendj_library::cleanup::Rules::ALL {
            items.push(switch(key, name, about, settings::flag(key, default)));
        }
        items
    }

    /// The tabs a page has; most have none.
    fn tabs(&self, page: i32) -> Vec<String> {
        match page {
            LIBRARY => LIBRARY_TABS.iter().map(|t| t.to_string()).collect(),
            AUDIO_PLUGINS => PLUGIN_TABS.iter().map(|t| t.to_string()).collect(),
            _ => Vec::new(),
        }
    }

    /// Settings › Collection: General, Stages, Fields, Health and Import. `tab` None
    /// is every tab, for a search.
    fn library_page(&mut self, tab: Option<usize>) -> Vec<PrefItem> {
        let shows = |t: usize| tab.is_none_or(|x| x == t);
        let mut items = Vec::new();
        if shows(0) {
            items.extend(self.library_general());
        }
        if shows(1) {
            items.extend(self.stage_items());
        }
        if shows(2) {
            items.extend(self.field_items());
        }
        if shows(IMPORT_TAB) {
            items.extend(self.import_items());
        }
        if shows(HEALTH_TAB) {
            items.extend(self.health_items(tab.is_none()));
        }
        items
    }

    /// Settings › Collection › Import: each DJ app openDJ can bring a library
    /// in from. One not set up offers Set up…; once it's on, its own
    /// settings open up under its name. Apps still to come say so.
    fn import_items(&self) -> Vec<PrefItem> {
        let mut items = vec![item(
            "",
            NOTE,
            "",
            "Bring in another DJ app's library: its tracks, cues, grids and lists join your collection. openDJ \
             keeps them up to date while you still use that app.",
        )];
        if !self.import_note.is_empty() {
            items.push(item("", NOTE, "", &self.import_note));
        }
        for importer in crate::stores::importers() {
            let title = importer.title();
            items.push(item("", HEADING, &title.to_uppercase(), ""));
            if settings::import_enabled(importer.id()) {
                items.extend(self.import_rows(importer.as_ref()));
            } else {
                items.push(PrefItem {
                    action: "Set up…".into(),
                    ..item(
                        &format!("import_start:{}", importer.id()),
                        BUTTON,
                        &format!("Import from {title}"),
                        "",
                    )
                });
            }
        }
        items.push(item("", HEADING, "COMING SOON", ""));
        for (title, _) in COMING_IMPORTS {
            items.push(PrefItem {
                value: "Coming soon".into(),
                ..item("", BUTTON, &format!("Import from {title}"), "")
            });
        }
        items
    }

    /// Settings › Analysis › Stems: stems on load, the high-quality stems
    /// kept and the models.
    fn stems_items(&self) -> Vec<PrefItem> {
        let (count, bytes) = cache::stems_usage();
        let limit = settings::number("stems_cache_gb", DEFAULT_STEMS_CACHE_GB);
        let limits: Vec<String> = STEMS_CACHE_GB
            .iter()
            .map(|&gb| match gb {
                0.0 => "No limit".to_string(),
                gb if gb == DEFAULT_STEMS_CACHE_GB => format!("{gb} GB (default)"),
                gb => format!("{gb} GB"),
            })
            .collect();
        let have = |name: &str| separation::model(name).is_some();
        let state = |name: &str| {
            if have(name) {
                "downloaded"
            } else {
                "downloads when first needed"
            }
        };
        vec![
            item("stems", HEADING, "STEMS", ""),
            switch(
                "stems_on_load",
                "Stems on load",
                "Each track gets quick stems a second or two after it loads, from the cue point, \
                 so the stem pads and knobs work at once. Off saves CPU on older computers; \
                 Prepare High Quality Stems still works.",
                settings::flag("stems_on_load", true),
            ),
            PrefItem {
                choices: strings(&limits),
                chosen: STEMS_CACHE_GB
                    .iter()
                    .position(|&gb| gb == limit)
                    .unwrap_or(2) as i32,
                ..item(
                    "stems_cache_gb",
                    MENU,
                    "High quality stems kept",
                    &format!(
                        "{count} {} · {}. The stems played longest ago make room for new ones; \
                         Manage under Collection › Storage clears them.",
                        if count == 1 { "track" } else { "tracks" },
                        size(bytes),
                    ),
                )
            },
            item(
                "stem_models",
                NOTE,
                "",
                &format!(
                    "Quick stems model: {}. High quality model: {}.",
                    state(separation::SPLEETER),
                    state(separation::HTDEMUCS),
                ),
            ),
        ]
    }

    /// Settings › Collection › General: the music folders, the cache and
    /// cover art.
    fn library_general(&self) -> Vec<PrefItem> {
        let mut items = self.music_folder_items();
        items.extend([
            folder_list(
                settings::ALBUMS_FOLDER,
                "Albums",
                "One folder per album, or Artist › Album folders. Albums are in your collection \
                 and under Albums in the rail, where they play gapless.",
                &settings::albums_folder().into_iter().collect::<Vec<_>>(),
                true,
                false,
            ),
            folder_list(
                settings::MIXED_SETS_FOLDER,
                "Recordings and Mixed Sets",
                &format!(
                    "Whole DJ mixes, yours and others'. They stay out of your collection and show \
                     under Mixed Sets in the rail. Your recordings are saved in its \
                     \"{}\" folder.",
                    settings::RECORDINGS_SUBFOLDER
                ),
                &settings::mixed_sets_folder()
                    .into_iter()
                    .collect::<Vec<_>>(),
                true,
                false,
            ),
        ]);
        if !self.library_note.is_empty() {
            items.push(item("", NOTE, "", &self.library_note));
        }
        items.extend([
            PrefItem {
                choices: strings(&["Dot", "Strip", "Tint", "Off"]),
                chosen: settings::track_color().unwrap_or(0) as i32,
                ..item("track_color", TRACK_COLOR, "Track color", "")
            },
            self.storage_item(),
        ]);
        items
    }

    /// Settings › Collection › General › Storage: fills StorageUi with the
    /// disk's parts and returns the row that shows them.
    fn storage_item(&self) -> PrefItem {
        let mut folders = settings::music_folders();
        folders.extend(settings::albums_folder());
        folders.extend(settings::mixed_sets_folder());
        let music = crate::storage::music_bytes(folders.clone());
        let (_, analysis) = cache::analysis_usage();
        let (_, stems) = cache::stems_usage();
        let (played, _) = crate::sources::cache_usage();
        let disk = folders
            .first()
            .cloned()
            .or_else(cache::cache_dir)
            .and_then(|p| crate::storage::disk(&p));
        let ours = music.unwrap_or(0) + analysis + stems + played;
        let whole = disk.map_or(ours.max(1), |(total, _)| total.max(1)) as f32;
        let part = |name: &str, bytes: u64, tint: slint::Color| StoragePart {
            name: name.into(),
            size: size(bytes).into(),
            share: bytes as f32 / whole,
            tint,
        };
        let rgb = slint::Color::from_rgb_u8;
        let mut parts = vec![
            match music {
                Some(bytes) => part("Music", bytes, rgb(0xb6, 0x9c, 0xff)),
                None => StoragePart {
                    size: "measuring…".into(),
                    ..part("Music", 0, rgb(0xb6, 0x9c, 0xff))
                },
            },
            part("Analysis", analysis, rgb(0x5a, 0xc8, 0xfa)),
            part("Stems", stems, rgb(0xff, 0x9f, 0x0a)),
            part("Played tracks", played, rgb(0x30, 0xd1, 0x58)),
        ];
        let mut summary = String::new();
        if let Some((total, free)) = disk {
            let other = total.saturating_sub(free).saturating_sub(ours);
            parts.push(part("Other", other, rgb(0x63, 0x63, 0x66)));
            summary = format!("{} free of {}", size(free), size(total));
        }
        if let Some(app) = self.app.upgrade() {
            let g = app.global::<StorageUi>();
            g.set_free_share((1.0 - parts.iter().map(|p| p.share).sum::<f32>()).max(0.0));
            g.set_parts(ModelRc::new(VecModel::from(parts)));
            g.set_summary(summary.into());
            g.set_actions(strings(&STORAGE_ACTIONS));
            let (confirm, warning, label) = match self.storage_pending {
                Some(0) => (
                    format!("Clear the analysis cache ({})?", size(analysis)),
                    "Every track is analyzed again when it next loads: its waveform, tempo \
                     and key. Cues and tags stay in your collection.",
                    "Clear",
                ),
                Some(1) => (
                    format!("Clear high quality stems ({})?", size(stems)),
                    "Preparing them again takes a while for each track. Tracks still get \
                     quick stems when they load.",
                    "Clear",
                ),
                Some(2) => (
                    format!("Clear played tracks ({})?", size(played)),
                    "Tracks played from your servers download again when next played. \
                     Tracks you added to your collection stay.",
                    "Clear",
                ),
                _ => (String::new(), "", ""),
            };
            g.set_confirm(confirm.into());
            g.set_warning(warning.into());
            g.set_confirm_label(label.into());
            g.set_note(self.storage_note.clone().into());
        }
        item(
            "storage_manage",
            STORAGE,
            "Storage",
            "Clear the analysis cache, stems and played tracks",
        )
    }

    /// Manage › a clean-up: asks first, but Show cache folder just opens it.
    fn storage_picked(&mut self, index: usize) {
        self.storage_note.clear();
        if index == 3 {
            self.storage_pending = None;
            if let Some(dir) = cache::cache_dir() {
                crate::files::reveal(&dir);
            }
        } else {
            self.storage_pending = Some(index);
        }
        self.refresh();
    }

    /// The yes to the clean-up picked from Manage.
    fn storage_confirmed(&mut self) {
        let Some(index) = self.storage_pending.take() else {
            return;
        };
        let (_, before) = match index {
            0 => cache::analysis_usage(),
            1 => cache::stems_usage(),
            _ => (0, 0),
        };
        self.storage_note = match index {
            0 => {
                let n = cache::clear_analysis();
                format!(
                    "Cleared {} from {n} {}.",
                    size(before),
                    if n == 1 { "analysis" } else { "analyses" }
                )
            }
            1 => {
                let n = cache::clear_stems();
                format!(
                    "Cleared {} of stems from {n} {}.",
                    size(before),
                    if n == 1 { "track" } else { "tracks" }
                )
            }
            _ => match crate::sources::clear_cache() {
                0 => "There were no played tracks to clear.".into(),
                freed => format!("Cleared {} of played tracks.", size(freed)),
            },
        };
        self.refresh();
    }

    /// Settings › Collection › Tags: names for grouping and the tag fields.
    fn field_items(&self) -> Vec<PrefItem> {
        let mut items = vec![item(
            "",
            NOTE,
            "",
            "Name grouping and your tag fields for what you use them for, such as Trip Stage, \
             Mood or Instrument. Each name is also a search word (mood:uplifting) and a \
             filter on the bar above the track list.",
        )];
        let names = settings::field_names();
        items.push(PrefItem {
            value: names.grouping.clone().into(),
            ..item("grouping_name", TEXT, "Grouping", "")
        });
        for (i, name) in names.tags.iter().enumerate() {
            items.push(PrefItem {
                value: name.clone().into(),
                ..item(
                    &format!("tag_name:{i}"),
                    TEXT,
                    &format!("Tag {}", i + 1),
                    &format!("Search with tag{}: or {}:", i + 1, query::keyword(name)),
                )
            });
        }
        items.push(PrefItem {
            action: "Add".into(),
            ..item(
                "add_tag_field",
                BUTTON,
                "Add a tag field",
                "Another set of words to tag tracks with.",
            )
        });
        if names.tags.len() > 1 {
            items.push(PrefItem {
                action: "Remove".into(),
                ..item(
                    "remove_tag_field",
                    BUTTON,
                    &format!("Remove {}", names.tags.last().map_or("", String::as_str)),
                    "Its tags stay on your tracks, and come back if you add the field again.",
                )
            });
        }
        items
    }

    /// What an import that is on shows: where it reads from, which lists
    /// come in, whose tags win, and ways to stop or remove it.
    fn import_rows(&self, importer: &dyn LibraryImport) -> Vec<PrefItem> {
        let (id, title) = (importer.id(), importer.title());
        // The library's file, shown by the folder it's in (VirtualDJ's
        // database.xml as "VirtualDJ"); + picks another.
        let db = settings::import_database(importer);
        let mut rows = vec![PrefItem {
            choices: strings(&db.iter().map(|p| library_name(p)).collect::<Vec<_>>()),
            ..folder_list(
                &settings::import_database_key(id),
                &format!("{title} library"),
                "",
                &db.into_iter().collect::<Vec<_>>(),
                true,
                false,
            )
        }];
        if !importer.list_groups().is_empty() {
            rows.push(PrefItem {
                action: "Choose…".into(),
                ..item(&format!("import_lists:{id}"), BUTTON, "Lists", "")
            });
        }
        rows.push(theirs_row(
            &settings::import_theirs_key(id),
            title,
            settings::import_theirs_win(id),
        ));
        // Mixxx has no streamed tracks.
        if id != "mixxx" {
            rows.push(switch(
                &settings::import_wants_streamed_key(id),
                "Add streamed tracks to Wanted",
                "",
                settings::import_wants_streamed(id),
            ));
        }
        rows.push(PrefItem {
            action: "Stop".into(),
            ..item(&format!("import_stop:{id}"), BUTTON, "Stop importing", "")
        });
        let confirm = self.removing.as_deref() == Some(id);
        rows.push(PrefItem {
            action: if confirm { "Remove" } else { "Remove…" }.into(),
            ..item(
                &format!("import_remove:{id}"),
                BUTTON,
                "Remove import",
                &if confirm {
                    "Press again to remove. Tracks in your lists or music folder stay.".to_string()
                } else {
                    String::new()
                },
            )
        });
        rows
    }

    /// A button or menu in the import wizard, or one of an import's own.
    /// Returns false for ids that aren't the importers'.
    fn import_pressed(&mut self, id: &str) -> bool {
        if let Some(i) = id.strip_prefix("import_start:").and_then(importer) {
            self.wizard = Some(WizardState::new(i.as_ref()));
        } else if let Some(i) = id.strip_prefix("import_stop:").and_then(importer) {
            settings::set(&settings::import_enabled_key(i.id()), json!(false));
        } else if let Some(i) = id.strip_prefix("import_remove:").and_then(importer) {
            if self.removing.as_deref() != Some(i.id()) {
                self.removing = Some(i.id().to_string());
            } else {
                self.removing = None;
                settings::set(&settings::import_enabled_key(i.id()), json!(false));
                settings::set(&settings::import_remove_key(i.id()), json!(true));
                (self.rescan)();
                self.library_note = format!("Removing what came in from {}…", i.title());
                self.import_note = self.library_note.clone();
            }
        } else if let Some(i) = id.strip_prefix("import_database:").and_then(importer) {
            choose(
                id,
                &format!("Choose {}'s database", i.title()),
                false,
                self.chosen_tx.clone(),
            );
            return true;
        } else if id == "import_wizard_db" {
            let title = self.wizard_importer().map(|i| i.title().to_string());
            choose(
                id,
                &format!("Choose {}'s database", title.unwrap_or_default()),
                false,
                self.chosen_tx.clone(),
            );
            return true;
        } else if let Some(i) = id.strip_prefix("import_lists:").and_then(importer) {
            let mut w = WizardState::new(i.as_ref());
            w.step = 1;
            w.only_lists = true;
            w.read_lists(i.as_ref());
            self.wizard = Some(w);
        } else if let Some(node) = id
            .strip_prefix("import_pick:")
            .and_then(|n| n.strip_suffix("#fold"))
            .and_then(|n| n.parse().ok())
        {
            if let Some(p) = self.wizard.as_mut().and_then(|w| w.picker.as_mut()) {
                p.toggle_open(node);
            }
        } else if id == "import_wizard_cancel" {
            self.wizard = None;
        } else if id == "import_wizard_back" {
            if let Some(w) = &mut self.wizard {
                w.step = w.step.saturating_sub(1);
            }
        } else if id == "import_wizard_next" {
            let Some(w) = &mut self.wizard else {
                return true;
            };
            if w.database.is_none() || !w.readable {
                return true;
            }
            if w.only_lists {
                if let Some(w) = self.wizard.take() {
                    if let Some(i) = importer(&w.importer) {
                        w.save_lists(i.as_ref());
                        (self.rescan)();
                        self.library_note = "Updating what comes in from other apps…".into();
                        self.import_note = self.library_note.clone();
                    }
                }
            } else if w.step < 2 {
                w.step += 1;
                if w.step == 1 {
                    if let Some(i) = importer(&w.importer) {
                        w.read_lists(i.as_ref());
                    }
                }
            } else if let Some(w) = self.wizard.take() {
                self.finish_wizard(w);
            }
        } else {
            return false;
        }
        self.refresh();
        true
    }

    /// Switches the page shown to its tab `tab`; the caller fills it.
    fn show_tab(&self, tab: usize) {
        if let Some(app) = self.app.upgrade() {
            app.global::<Preferences>().set_tab(tab as i32);
        }
    }

    fn wizard_importer(&self) -> Option<Box<dyn LibraryImport>> {
        self.wizard.as_ref().and_then(|w| importer(&w.importer))
    }

    /// Saves the wizard's choices and brings the library in.
    fn finish_wizard(&mut self, w: WizardState) {
        let Some(i) = importer(&w.importer) else {
            return;
        };
        let id = i.id();
        // Only a database the DJ picked is saved, so the usual place is
        // followed if VirtualDJ moves it.
        let picked = w
            .database
            .clone()
            .filter(|d| Some(d) != i.database().as_ref());
        settings::set(
            &settings::import_database_key(id),
            json!(picked.map_or(String::new(), |d| d.to_string_lossy().into_owned())),
        );
        if w.picker.is_some() {
            w.save_lists(i.as_ref());
        } else {
            for g in i.list_groups() {
                settings::set(
                    &settings::import_group_key(id, g.id),
                    json!(w.groups.contains(&g.id)),
                );
            }
        }
        settings::set(&settings::import_theirs_key(id), json!(w.theirs_win));
        settings::set(&settings::import_remove_key(id), json!(false));
        settings::set(&settings::import_enabled_key(id), json!(true));
        // Its settings open up on the Import tab, shown now.
        self.show_tab(IMPORT_TAB);
        (self.rescan)();
        self.library_note = format!(
            "Bringing in {}… tracks appear in your collection when it is done.",
            i.title()
        );
        self.import_note = self.library_note.clone();
    }

    fn toggled(&mut self, id: &str, on: bool) {
        // A controller's switch: off stops it, on runs its automatic
        // mapping again (or the one picked in its menu after).
        if let Some(port) = id.strip_prefix("controller:") {
            if let Some(c) = &self.controllers {
                c.set_choice(
                    port,
                    if on {
                        MappingChoice::Auto
                    } else {
                        MappingChoice::Off
                    },
                );
                settings::set_controller_mappings(&c.choices());
            }
            self.refresh();
            return;
        }
        if let Some(i) = id.strip_prefix("source:").and_then(|i| i.parse().ok()) {
            self.set_rail_shown(i, on);
            self.refresh();
            return;
        }
        if let Some(key) = id.strip_prefix("audio_plugin_show:") {
            self.audio_plugins.borrow_mut().set_shown(key, on);
            self.refresh();
            return;
        }
        if let Some(maker) = id.strip_prefix("audio_plugin_maker:") {
            let list = self.plugin_list();
            let v = &self.plugin_view;
            let keys: Vec<String> = plugin_groups(&list, &v.formats, &v.search)
                .into_iter()
                .filter(|(m, _)| m == maker)
                .flat_map(|(_, fs)| fs.into_iter().map(|f| f.key))
                .collect();
            self.audio_plugins.borrow_mut().set_shown_all(&keys, on);
            self.refresh();
            return;
        }
        if let Some(node) = id.strip_prefix("import_pick:").and_then(|n| n.parse().ok()) {
            if let Some(p) = self.wizard.as_mut().and_then(|w| w.picker.as_mut()) {
                p.set(node, on);
            }
            self.refresh();
            return;
        }
        if let Some(group) = id.strip_prefix("import_wizard_group:") {
            if let Some(w) = &mut self.wizard {
                w.groups.retain(|g| *g != group);
                if on {
                    if let Some(g) = importer(&w.importer)
                        .and_then(|i| i.list_groups().into_iter().find(|g| g.id == group))
                    {
                        w.groups.push(g.id);
                    }
                }
            }
            self.refresh();
            return;
        }
        if id.starts_with("plugin:") {
            self.set_plugin_value(id, if on { "on" } else { "off" });
            self.refresh();
            return;
        }
        if let Some((i, field)) = station_field(id) {
            self.edit_station(i, |s| match field {
                "enabled" => s.enabled = on,
                "tls" => s.tls = on,
                "public" => s.public = on,
                "opus_titles" => s.opus_titles = on,
                _ => {}
            });
            self.refresh();
            return;
        }
        settings::set(id, json!(on));
        if id == settings::MOVE_FILES && on {
            // The library update makes the moves that were waiting.
            (self.rescan)();
            self.library_note = "Moving files that were waiting…".into();
        }
        if id.starts_with("import:") {
            // Brings the library and its lists in again with the new choice.
            (self.rescan)();
            self.library_note = "Updating what comes in from other apps…".into();
            self.import_note = self.library_note.clone();
        }
        match id {
            "keylock_default" if on => self.each_deck("keylock", 1.0),
            "quantize" => self.each_deck("quantize", f(on)),
            "vinyl_mode" => self.each_deck("vinyl_mode", f(on)),
            "protect_playing" => self.set("[Controls]", "AllowTrackLoadToPlayingDeck", f(!on)),
            "xfader_reverse" => self.set("[Mixer Profile]", "xFaderReverse", f(on)),
            "xfader_enabled" => self.set("[Mixer]", "crossfader_enabled", f(on)),
            "head_split" => self.set("[Master]", "headSplit", f(on)),
            "pfl_on_load" => self.set("[Controls]", "pfl_on_load", f(on)),
            "auto_gain" => self.set("[ReplayGain]", "ReplayGainEnabled", f(on)),
            "stems_on_load" => separation::set_on_load(on),
            "auto_reset_fx" | "auto_reset_eq" | "auto_reset_gain" => {
                self.set("[Controls]", id, f(on))
            }
            _ => {}
        }
        self.refresh();
    }

    fn picked(&mut self, id: &str, index: usize) {
        if id == "storage_manage" {
            return self.storage_picked(index);
        }
        if id == "audio_plugin_filter" {
            if let Some(f) = plugin_formats(&self.plugin_list()).get(index) {
                let formats = &mut self.plugin_view.formats;
                if let Some(i) = formats.iter().position(|x| x == f) {
                    formats.remove(i);
                } else {
                    formats.push(f.clone());
                }
            }
            self.refresh();
            return;
        }
        if let Some((i, field)) = station_field(id) {
            self.edit_station(i, |s| match field {
                "server" => {
                    s.server = if index == 1 {
                        opendj_broadcast::ServerKind::Shoutcast
                    } else {
                        opendj_broadcast::ServerKind::Icecast
                    }
                }
                "format" => {
                    s.format = if index == 1 {
                        opendj_broadcast::Format::Opus
                    } else {
                        opendj_broadcast::Format::Mp3
                    }
                }
                "bitrate" => s.bitrate = BITRATES.get(index).copied().unwrap_or(s.bitrate),
                _ => {}
            });
            self.refresh();
            return;
        }
        if id == "tour_part" {
            self.pressed(&format!("tour_start:{index}"));
            return;
        }
        if id == "keyboard_set" {
            let sets = settings::keyboard_sets();
            match index.checked_sub(sets.len()) {
                None => settings::select_keyboard_set(&sets[index]),
                Some(0) => {
                    let name = format!("{} copy", settings::keyboard_set());
                    settings::add_keyboard_set(&name, &settings::keyboard_shortcuts());
                }
                Some(1) => choose(
                    "keyboard_set_import",
                    "Choose a VirtualDJ keyboard mapping",
                    false,
                    self.chosen_tx.clone(),
                ),
                Some(_) => settings::delete_keyboard_set(),
            }
            self.shortcut_note.clear();
            self.refresh();
            return;
        }
        if let Some(key) = id.strip_prefix("shortcut:") {
            let Some(verb) = opendj_script::VERBS.get(index) else {
                return;
            };
            let mut own = settings::keyboard_shortcuts();
            let now = shortcuts::rows(&own)
                .into_iter()
                .find(|r| r.key == key)
                .map(|r| r.script)
                .unwrap_or_default();
            // An empty line takes the verb; otherwise it runs after.
            let script = match now.trim() {
                "" => shortcuts::sample(verb),
                now => format!("{now} & {}", shortcuts::sample(verb)),
            };
            shortcuts::set_script(&mut own, key, &script);
            settings::set_keyboard_shortcuts(&own);
            self.refresh();
            return;
        }
        if id == "import_wizard_theirs" {
            if let Some(w) = &mut self.wizard {
                w.theirs_win = index == 0;
            }
            self.refresh();
            return;
        }
        if id.starts_with("import_theirs:") {
            settings::set(id, json!(index == 0));
            (self.rescan)();
            self.library_note = "Updating what comes in from other apps…".into();
            self.refresh();
            return;
        }
        if let Some(port) = id.strip_prefix("controller:") {
            if let Some(c) = &self.controllers {
                let choice = match index {
                    0 => MappingChoice::Auto,
                    i => {
                        let mappings = c.mappings();
                        match mappings.get(i - 1) {
                            Some((_, file)) => MappingChoice::File(file.clone()),
                            None => match (
                                quick_maps().get(i - 1 - mappings.len()),
                                settings::own_mappings_dir(),
                            ) {
                                (Some(m), Some(d)) => MappingChoice::Own(own::path(&d, &m.id)),
                                _ => return,
                            },
                        }
                    }
                };
                c.set_choice(port, choice);
                settings::set_controller_mappings(&c.choices());
            }
            self.refresh();
            return;
        }
        match id {
            "bpm_range" => {
                let Some(&(lo, hi)) = BPM_RANGES.get(index) else {
                    return;
                };
                settings::set("bpm_min", json!(lo));
                settings::set("bpm_max", json!(hi));
                analysis::set_bpm_range(lo, hi);
            }
            "sources_cache_gb" => {
                let Some(&gb) = SOURCES_CACHE_GB.get(index) else {
                    return;
                };
                settings::set(crate::sources::CACHE_GB, json!(gb));
            }
            "stems_cache_gb" => {
                let Some(&gb) = STEMS_CACHE_GB.get(index) else {
                    return;
                };
                settings::set("stems_cache_gb", json!(gb));
                apply_stems_cache_limit();
            }
            "beat_detector" => {
                let on = index == 1;
                settings::set("neural_beats", json!(on));
                beatthis::set_enabled(on);
            }
            "key_detector" => {
                let Some(&d) = key::Detector::ALL.get(index) else {
                    return;
                };
                settings::set("key_detector", json!(d.id()));
                key::set_detector(d);
            }
            "key_mixing" => {
                let m = key::Mixing::from_id(index as u8);
                settings::set("key_mixing", json!(m.id()));
                key::set_mixing(m);
            }
            "beat_counter" => {
                settings::set("beat_counter", json!(index.min(2)));
                if let Some(app) = self.app.upgrade() {
                    app.global::<BeatCounter>().set_mode(beat_counter_mode());
                }
            }
            "key_match_on_load" => {
                settings::set("key_match_on_load", json!(index.min(2)));
                self.set("[Controls]", "key_match_on_load", index.min(2) as f64);
            }
            "track_color" => {
                if let Some(app) = self.app.upgrade() {
                    app.global::<crate::ColumnUi>()
                        .invoke_set_color_mode(index.min(3) as i32);
                }
            }
            "key_notation" => {
                let n = key::Notation::from_id(index as u8);
                settings::set_key_notation(n);
                key::set_notation(n);
                if let Some(app) = self.app.upgrade() {
                    app.global::<KeyNotation>().set_camelot(n.wheel());
                }
            }
            "cue_mode" => {
                let Some(&mode) = CUE_MODES.get(index) else {
                    return;
                };
                settings::set("cue_mode", json!(mode));
                self.each_deck("cue_mode", mode);
            }
            "tempo_direction" => {
                settings::set("tempo_up_faster", json!(index == 1));
                self.each_deck("rate_dir", rate_dir());
            }
            "rate_range" => {
                let Some(&range) = RATE_RANGES.get(index) else {
                    return;
                };
                settings::set("rate_range", json!(range));
                self.each_deck("rateRange", range);
            }
            "auto_gain_target" => {
                let Some(&target) = AUTO_GAIN_TARGETS.get(index) else {
                    return;
                };
                settings::set("auto_gain_target", json!(target));
                self.set("[ReplayGain]", "target_lufs", target);
            }
            "loudness_target" => {
                let Some(&(target, _)) = LOUDNESS_TARGETS.get(index) else {
                    return;
                };
                settings::set("loudness_target", json!(target));
                self.set("[Master]", "loudness_target", target);
            }
            "air_ceiling" => {
                let Some(&(ceiling, _)) = AIR_CEILINGS.get(index) else {
                    return;
                };
                settings::set("air_ceiling", json!(ceiling));
                self.set("[Master]", "air_ceiling", ceiling);
            }
            "recording_format" => {
                let Some(&(format, bits, _)) = RECORD_FORMATS.get(index) else {
                    return;
                };
                settings::set("recording_format", json!(format.name()));
                settings::set("recording_bits", json!(bits));
                self.recorder.configure(settings::recording());
            }
            "xfader_shape" => {
                settings::set("xfader_shape", json!(index));
                self.set("[Mixer Profile]", "xFaderShape", index as f64);
            }
            "talkover_ducking" => {
                settings::set("talkover_ducking", json!(index));
                self.set("[Master]", "talkover_ducking", index as f64);
            }
            "duck_strength" => {
                let Some(&db) = DUCK_DB.get(index) else {
                    return;
                };
                let strength = duck_strength(db);
                settings::set("duck_strength", json!(strength));
                self.set("[Master]", "duckStrength", strength);
            }
            // The first choice, Device default, clears the setting.
            "sample_rate" => {
                let rate = index.checked_sub(1).and_then(|i| SAMPLE_RATES.get(i));
                settings::set("sample_rate", json!(rate));
            }
            "buffer_frames" => {
                let frames = index.checked_sub(1).and_then(|i| BUFFER_SIZES.get(i));
                settings::set("buffer_frames", json!(frames));
                (self.audio_changed)();
                if let Some(app) = self.app.upgrade() {
                    app.global::<AudioSettings>().invoke_refresh();
                }
            }
            _ => return,
        }
        self.refresh();
    }

    /// Keyboard shortcuts, each an openDJscript line (see shortcuts.rs).
    fn audio_plugin_pressed(&mut self, id: &str) -> bool {
        if let Some(maker) = id
            .strip_prefix("audio_plugin_maker:")
            .and_then(|m| m.strip_suffix("#fold"))
        {
            let open = &mut self.plugin_view.open;
            if !open.remove(maker) {
                open.insert(maker.to_string());
            }
            self.refresh();
            return true;
        }
        let mut p = self.audio_plugins.borrow_mut();
        if id == "audio_plugins_scan" {
            p.start_scan();
        } else if id == "audio_plugin_folder_add" {
            choose(id, "Choose a plugin folder", true, self.chosen_tx.clone());
        } else if let Some(i) = id
            .strip_prefix("audio_plugin_folder_remove:")
            .and_then(|i| i.parse::<usize>().ok())
        {
            let mut folders = p.folders();
            if i < folders.len() {
                folders.remove(i);
            }
            p.set_folders(&folders);
        } else if id == "audio_plugin_errors" {
            let failures = p.failures();
            let text: Vec<String> = failures
                .iter()
                .map(|(path, why)| format!("{}\n    {why}", path.display()))
                .collect();
            if let Some(app) = self.app.upgrade() {
                let g = app.global::<PluginErrors>();
                g.set_title(
                    format!(
                        "{} {} couldn't be loaded",
                        failures.len(),
                        if failures.len() == 1 {
                            "plugin"
                        } else {
                            "plugins"
                        }
                    )
                    .into(),
                );
                g.set_text(text.join("\n\n").into());
                g.set_open(true);
            }
        } else if id == "audio_plugin_errors_close" {
            if let Some(app) = self.app.upgrade() {
                app.global::<PluginErrors>().set_open(false);
            }
        } else if id == "audio_plugin_retry_all" {
            for (path, _) in p.failures() {
                p.retry(&path);
            }
            if let Some(app) = self.app.upgrade() {
                app.global::<PluginErrors>().set_open(false);
            }
        } else {
            return false;
        }
        drop(p);
        self.refresh();
        true
    }

    fn keyboard_page(&self) -> Vec<PrefItem> {
        let fixed = if cfg!(target_os = "macos") {
            "Space plays or pauses the master deck, ⌘, opens Settings and Esc closes them; those stay as they are."
        } else {
            "Space plays or pauses the master deck, Ctrl+, opens Settings and Esc closes them; those stay as they are."
        };
        let mut items = vec![
            item(
                "",
                NOTE,
                "",
                &format!(
                    "Each key runs a line of openDJscript, such as “rating 3” or “deck 2 sync & play”. \
                     Click a key to change it, type the action, or pick one from Actions. {fixed}"
                ),
            ),
            item("", HEADING, "SET", ""),
        ];
        let sets = settings::keyboard_sets();
        let current = settings::keyboard_set();
        let mut choices = sets.clone();
        choices.push("New set from this one".into());
        choices.push("Import…".into());
        let default = current == settings::DEFAULT_KEYBOARD_SET;
        if !default {
            choices.push("Delete this set".into());
        }
        items.push(PrefItem {
            choices: strings(&choices),
            chosen: sets.iter().position(|n| *n == current).unwrap_or(0) as i32,
            ..item(
                "keyboard_set",
                MENU,
                "Shortcut set",
                "Keep several sets of shortcuts and switch between them. Import reads a VirtualDJ keyboard mapping.",
            )
        });
        if !default {
            items.push(PrefItem {
                value: current.clone().into(),
                ..item("keyboard_set_name", TEXT, "Name", "")
            });
        }
        items.push(item("", HEADING, "SHORTCUTS", ""));
        if !self.shortcut_note.is_empty() {
            items.push(item("", NOTE, "", &self.shortcut_note));
        }
        let own = settings::keyboard_shortcuts();
        items.extend(shortcuts::rows(&own).iter().map(shortcut_item));
        items.push(PrefItem {
            detail: "Click Add a key, then press the keys for a new shortcut.".into(),
            ..item("shortcut_new", SHORTCUT, "", "")
        });
        items.push(PrefItem {
            action: "Reset All".into(),
            ..item(
                "shortcuts_reset_all",
                BUTTON,
                "Reset all shortcuts",
                &format!(
                    "Back to openDJ's own: {} to {} rate the picked tracks.",
                    shortcuts::display("cmd+1"),
                    shortcuts::display("cmd+5")
                ),
            )
        });
        items.push(item("", HEADING, "ACTIONS", ""));
        items.push(item(
            "",
            NOTE,
            "",
            "A verb alone acts on the master deck; “deck 2 play” on deck 2. “&” runs one action after \
             another, “play ? pause : play” asks first, and “toggle 'slip_enabled'” reaches any control by its Mixxx name.",
        ));
        items.extend(opendj_script::VERBS.iter().map(|v| {
            item(
                &format!("verb:{}", v.name),
                NOTE,
                &shortcuts::usage(v),
                v.help,
            )
        }));
        items
    }

    /// The DJ typed in a shortcut: its row is updated where it is, so the
    /// field keeps its focus.
    fn update_shortcut_row(&self, id: &str) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let own = settings::keyboard_shortcuts();
        let Some(row) = shortcuts::rows(&own)
            .into_iter()
            .find(|r| id.strip_prefix("shortcut:") == Some(r.key.as_str()))
        else {
            return;
        };
        let items = app.global::<Preferences>().get_items();
        for i in 0..slint::Model::row_count(&items) {
            let Some(old) = slint::Model::row_data(&items, i) else {
                continue;
            };
            if old.id == id {
                let mut new = shortcut_item(&row);
                // The field shows what was typed, not a saved copy.
                new.value = old.value;
                slint::Model::set_row_data(&items, i, new);
            }
        }
    }

    /// A key pressed on a shortcut's key button. False to keep listening.
    fn key_captured(&mut self, id: &str, text: &str, cmd: bool, alt: bool, shift: bool) -> bool {
        let Some(key) = shortcuts::key_name(text, cmd, alt, shift) else {
            return false;
        };
        let shown = shortcuts::display(&key);
        let mut own = settings::keyboard_shortcuts();
        if shortcuts::FIXED.contains(&key.as_str())
            || shortcuts::normalize(&key).as_deref() != Some(&key)
        {
            self.shortcut_note = format!("{shown} can't be a shortcut.");
        } else if let Some(old) = id.strip_prefix("shortcut:") {
            let script = |key: &str| {
                shortcuts::rows(&own)
                    .into_iter()
                    .find(|r| r.key == key)
                    .map(|r| r.script)
                    .unwrap_or_default()
            };
            // Moving a key that does nothing onto one in use would only
            // turn that one off.
            let taken = script(&key);
            if script(old).trim().is_empty() && !taken.trim().is_empty() && old != key {
                self.shortcut_note = format!("{shown} already runs “{taken}”.");
                self.refresh();
                return true;
            }
            self.shortcut_note = match shortcuts::rebind(&mut own, old, &key) {
                Some(was) => format!("{shown} ran “{was}”; it now runs this one instead."),
                None => String::new(),
            };
        } else if !shortcuts::add(&mut own, &key) {
            self.shortcut_note = format!("{shown} is already on the list.");
        } else {
            self.shortcut_note.clear();
        }
        settings::set_keyboard_shortcuts(&own);
        self.refresh();
        true
    }

    /// Settings › Sources: the cache every source shares, then each source
    /// by kind with its status and In rail switch. The open source shows
    /// its settings under it; a search (`all`) shows every source's.
    fn sources_page(&self, all: bool) -> Vec<PrefItem> {
        let (used, each) = crate::sources::cache_usage();
        let limit = settings::number(crate::sources::CACHE_GB, crate::sources::DEFAULT_CACHE_GB);
        let limits: Vec<String> = SOURCES_CACHE_GB
            .iter()
            .map(|&gb| {
                if gb == crate::sources::DEFAULT_CACHE_GB {
                    format!("{gb} GB (default)")
                } else {
                    format!("{gb} GB")
                }
            })
            .collect();
        let mut cache_rows = vec![PrefItem {
            choices: strings(&limits),
            chosen: SOURCES_CACHE_GB
                .iter()
                .position(|&gb| gb == limit)
                .unwrap_or(2) as i32,
            ..item(
                "sources_cache_gb",
                MENU,
                "Cache for played tracks",
                &format!(
                    "{} used by tracks played from your servers; the oldest go first. \
                         Manage under Collection › Storage clears them.",
                    if used == 0 {
                        "Nothing".to_string()
                    } else {
                        size(used)
                    },
                ),
            )
        }];
        if !self.sources_note.is_empty() {
            cache_rows.push(item("", NOTE, "", &self.sources_note));
        }
        let rail = self.rail_shown();
        let shown = |i: i32| rail.get(i as usize).copied().unwrap_or(true);
        let place = |name: &str| {
            self.rail_names
                .iter()
                .position(|n| n == name)
                .map(|i| i as i32)
        };
        let open = |i: i32| all || self.open_source == Some(i);
        let fold = |i: i32| if open(i) { "▾" } else { "▸" };
        let row = |i: i32, title: &str, status: i32, detail: &str, arrow: &str| PrefItem {
            on: shown(i),
            chosen: status,
            action: arrow.into(),
            ..item(&format!("source:{i}"), SOURCE_ROW, title, detail)
        };

        let mut items = Vec::new();
        items.push(item("", HEADING, "BUILT IN", ""));
        for name in ["Collection", "Files", "Albums", "Mixed Sets", "Instruments"] {
            if let Some(i) = place(name) {
                items.push(row(i, name, 0, "", ""));
            }
        }
        // Everything online in one section: the streaming services, the
        // DJ's own servers and free music (the stores come later). Add-ons after.
        items.push(item("", HEADING, "STREAMING", ""));
        if crate::browser::STREAMING_ENABLED {
            for &(source, name, id) in crate::browser::STREAMS.iter() {
                let signed_in = crate::spotify::Saved::load(&format!("{id}.json"))
                    .is_some_and(|s| !s.refresh_token.is_empty());
                let (status, detail) = if signed_in {
                    (1, "Signed in".to_string())
                } else {
                    (0, "Not signed in".to_string())
                };
                items.push(row(source, name, status, &detail, fold(source)));
                if open(source) {
                    items.push(item(
                        "",
                        NOTE,
                        "",
                        &format!(
                            "Sign in from {name} in the rail. Its playlists show there, and \
                             tracks also in your collection can be played. {name} is browse \
                             only: nothing downloads and nothing is cached."
                        ),
                    ));
                    if signed_in {
                        items.push(PrefItem {
                            action: "Sign out".into(),
                            ..item(
                                &format!("stream_sign_out:{source}"),
                                BUTTON,
                                &format!("Sign out of {name}"),
                                "Your playlists there are forgotten here until you sign in again.",
                            )
                        });
                    }
                }
            }
        }

        // Plugin sources, by kind: the DJ's own servers, built-in sources,
        // then add-ons.
        let builtin: Vec<&str> = opendj_plugins::builtin()
            .iter()
            .map(|p| p.manifest().id)
            .collect();
        let kind = |n: usize| {
            if self.plugin_plays[n] {
                0
            } else if builtin.contains(&self.plugins[n].manifest().id) {
                1
            } else {
                2
            }
        };
        let mut addons = Vec::new();
        for k in 0..3 {
            let members: Vec<usize> = (0..self.plugins.len()).filter(|&n| kind(n) == k).collect();
            if members.is_empty() {
                continue;
            }
            // Servers and free music join the streaming section; add-ons
            // get their own, last.
            let items = if k == 2 {
                addons.push(item("", HEADING, "ADD-ONS", ""));
                &mut addons
            } else {
                &mut items
            };
            for n in members {
                let plugin = &self.plugins[n];
                let name = plugin.manifest().name;
                let config = settings::plugin_config(plugin.manifest().id);
                let set_up = !crate::sources::needs_setup(&plugin.settings(), &config);
                let mut detail = if !set_up {
                    "Not set up".to_string()
                } else if self.plugin_plays[n] {
                    "Set up".to_string()
                } else {
                    "Ready".to_string()
                };
                let cached = each
                    .get(&crate::sources::cache_key(name))
                    .copied()
                    .unwrap_or(0);
                if cached > 0 {
                    detail.push_str(&format!(" · {} cached", size(cached)));
                }
                let folder = match config.get(DOWNLOAD_FOLDER).trim() {
                    "" => format!("Downloaded/{name}"),
                    set => set.to_string(),
                };
                if plugin.settings().iter().any(|s| s.key == DOWNLOAD_FOLDER) {
                    // A server's files reach the collection with Add to
                    // Collection; other sources' with Download.
                    let verb = if self.plugin_plays[n] {
                        "adds to"
                    } else {
                        "downloads to"
                    };
                    detail.push_str(&format!(" · {verb} {folder}"));
                }
                match place(name) {
                    Some(i) => {
                        items.push(row(i, name, set_up as i32, &detail, fold(i)));
                        if open(i) {
                            items.extend(self.plugin_page(n));
                        }
                    }
                    // Settings with no source in the rail: always open.
                    None => {
                        items.push(item("", HEADING, &name.to_uppercase(), ""));
                        items.extend(self.plugin_page(n));
                    }
                }
            }
        }

        items.extend(cache_rows);
        items.extend(addons);
        items
    }

    /// Whether each source shows in the rail, in the rail's order.
    fn rail_shown(&self) -> Vec<bool> {
        self.app
            .upgrade()
            .map(|app| slint::Model::iter(&app.get_sources_shown()).collect())
            .unwrap_or_default()
    }

    /// Shows or hides source `i` in the rail, as its right-click menu does.
    fn set_rail_shown(&self, i: i32, on: bool) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let shown = app.get_sources_shown();
        if i < 0 || i as usize >= slint::Model::row_count(&shown) {
            return;
        }
        slint::Model::set_row_data(&shown, i as usize, on);
        app.invoke_source_shown_changed(i, on);
    }

    /// Plugin `index`'s page: its own rows, ids `plugin:<index>:<key>`.
    fn plugin_page(&self, index: usize) -> Vec<PrefItem> {
        let Some(plugin) = self.plugins.get(index) else {
            return Vec::new();
        };
        let config = settings::plugin_config(plugin.manifest().id);
        plugin
            .settings()
            .into_iter()
            .map(|s| {
                let id = format!("plugin:{index}:{}", s.key);
                match s.kind {
                    SettingKind::Note => item(&id, NOTE, &s.title, &s.detail),
                    SettingKind::Switch(default) => PrefItem {
                        on: config.is_on(s.key, default),
                        ..item(&id, SWITCH, &s.title, &s.detail)
                    },
                    SettingKind::Folder => PrefItem {
                        value: match config.get(s.key) {
                            "" => "Default".into(),
                            set => set.into(),
                        },
                        action: "Choose…".into(),
                        ..item(&id, PATH, &s.title, &s.detail)
                    },
                    kind => PrefItem {
                        value: config.get(s.key).into(),
                        on: kind == SettingKind::Secret,
                        ..item(&id, TEXT, &s.title, &s.detail)
                    },
                }
            })
            .collect()
    }

    /// The DJ typed in a plugin's text field. The page isn't filled again,
    /// so the field keeps its place while typing.
    fn typed(&mut self, id: &str, text: &str) {
        if let Some(i) = id
            .strip_prefix("prep_line:")
            .and_then(|i| i.parse::<usize>().ok())
        {
            let mut lines = settings::prep_lines_typed();
            if let Some(line) = lines.get_mut(i) {
                *line = text.to_string();
                settings::set_prep_lines(&lines);
            }
            return;
        }
        if id == "audio_plugin_filter" {
            self.plugin_view.search = text.to_string();
            self.refresh();
            return;
        }
        if let Some((i, field)) = station_field(id) {
            if field == "password" {
                let station = broadcast::stations().into_iter().nth(i);
                if let (Some(b), Some(s)) = (&self.broadcast, station) {
                    b.set_password(&s.id, text);
                }
                return;
            }
            let text = text.to_string();
            self.edit_station(i, |s| match field {
                "name" => s.name = text,
                "host" => s.host = text.trim().to_string(),
                "port" => s.port = text.trim().parse().unwrap_or(s.port),
                "mount" => s.mount = text.trim().to_string(),
                "user" => s.user = text.trim().to_string(),
                "stream_name" => s.stream_name = text,
                "genre" => s.genre = text,
                "website" => s.website = text.trim().to_string(),
                "description" => s.description = text,
                _ => {}
            });
            return;
        }
        if id == broadcast::TITLE_TEMPLATE || id == broadcast::DJ_NAME {
            settings::set(id, json!(text));
            if let Some(b) = &self.broadcast {
                b.configure();
            }
            return;
        }
        if id == "keyboard_set_name" {
            settings::rename_keyboard_set(text);
            return;
        }
        if let Some(key) = id.strip_prefix("shortcut:") {
            let mut own = settings::keyboard_shortcuts();
            shortcuts::set_script(&mut own, key, text);
            settings::set_keyboard_shortcuts(&own);
            self.update_shortcut_row(id);
            return;
        }
        if id == "discogs_token" {
            settings::set_discogs_token(text);
            return;
        }
        if id == "acoustid_key" {
            settings::set_acoustid_key(text);
            return;
        }
        if id == "grouping_name" || id.starts_with("tag_name:") {
            let mut names = settings::field_names();
            match id
                .strip_prefix("tag_name:")
                .and_then(|i| i.parse::<usize>().ok())
            {
                Some(i) if i < names.tags.len() => names.tags[i] = text.to_string(),
                Some(_) => return,
                None => names.grouping = text.to_string(),
            }
            set_field_names(names);
            return;
        }
        self.set_plugin_value(id, text);
    }

    /// A value on a plugin's page (`plugin:<index>:<key>`) changed: it's
    /// saved, and the plugin hears of it.
    fn set_plugin_value(&mut self, id: &str, value: &str) {
        let mut parts = id.splitn(3, ':');
        let (Some("plugin"), Some(index), Some(key)) = (parts.next(), parts.next(), parts.next())
        else {
            return;
        };
        let Some(plugin) = index
            .parse()
            .ok()
            .and_then(|i: usize| self.plugins.get_mut(i))
        else {
            return;
        };
        let plugin_id = plugin.manifest().id;
        settings::set_plugin_value(plugin_id, key, value);
        plugin.configure(&settings::plugin_config(plugin_id));
        (self.plugin_changed)(plugin_id);
    }

    fn pressed(&mut self, id: &str) {
        if self.broadcast_pressed(id) {
            return;
        }
        if let Some(tutorial) = id.strip_prefix("tutorial_start:") {
            if let Some(app) = self.app.upgrade() {
                app.global::<Preferences>().set_open(false);
                app.global::<Tour>().invoke_start_tutorial(tutorial.into());
            }
            return;
        }
        if let Some(i) = id
            .strip_prefix("source:")
            .and_then(|i| i.strip_suffix("#fold"))
            .and_then(|i| i.parse().ok())
        {
            self.open_source = (self.open_source != Some(i)).then_some(i);
            self.refresh();
            return;
        }
        if let Some(source) = id
            .strip_prefix("stream_sign_out:")
            .and_then(|i| i.parse().ok())
        {
            if let Some(app) = self.app.upgrade() {
                app.invoke_stream_sign_out(source);
            }
            self.sources_note = "Signed out.".into();
            self.refresh();
            return;
        }
        if id.starts_with("tour_") {
            if let Some(app) = self.app.upgrade() {
                if let Some(part) = id.strip_prefix("tour_start:") {
                    app.global::<Preferences>().set_open(false);
                    app.global::<Tour>()
                        .invoke_start(part.parse().unwrap_or(-1));
                } else {
                    app.global::<Tour>().invoke_pressed(id.into());
                }
            }
            return;
        }
        if id == "shortcuts_reset_all" {
            settings::set_keyboard_shortcuts(&[]);
            self.shortcut_note = "Every shortcut is back to its default.".into();
            self.refresh();
            return;
        }
        if let Some(key) = id.strip_prefix("shortcut:") {
            let mut own = settings::keyboard_shortcuts();
            match shortcuts::rows(&own).into_iter().find(|r| r.key == key) {
                Some(r) if r.default && r.changed => shortcuts::reset(&mut own, key),
                _ => shortcuts::remove(&mut own, key),
            }
            settings::set_keyboard_shortcuts(&own);
            self.shortcut_note.clear();
            self.refresh();
            return;
        }
        if self.import_pressed(id) {
            return;
        }
        if id == CLEANUP_FOLD {
            self.cleanup_open = !self.cleanup_open;
            self.refresh();
            return;
        }
        if self.folder_list_pressed(id) {
            return;
        }
        if self.audio_plugin_pressed(id) {
            return;
        }
        if id.starts_with("plugin:") {
            choose(id, "Choose a folder", true, self.chosen_tx.clone());
            return;
        }
        if let Some(i) = id
            .strip_prefix("music_folder_remove:")
            .and_then(|i| i.parse::<usize>().ok())
        {
            let mut folders = settings::music_folders();
            if i < folders.len() {
                folders.remove(i);
            }
            settings::set_music_folders(&folders);
            self.music_folders_changed();
            self.library_note = "Its tracks leave your collection on the next update, except ones \
                                 you rated, played, staged or put in a list: Collection Health shows \
                                 those under Outside folders. The files stay."
                .into();
            self.refresh();
            return;
        }
        if let Some(i) = id
            .strip_prefix("stage_inbox_remove:")
            .and_then(|i| i.parse::<usize>().ok())
        {
            let mut inbox = settings::paths(settings::STAGE_INBOX);
            if i < inbox.len() {
                inbox.remove(i);
            }
            let inbox: Vec<String> = inbox
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            settings::set(settings::STAGE_INBOX, json!(inbox));
            self.stages_changed();
            return;
        }
        if let Some((port, what)) = id
            .strip_prefix("controller:")
            .and_then(|p| p.rsplit_once('#'))
        {
            match what {
                "edit" => {
                    let port = self.ports.iter().find(|p| p.name == port);
                    let file = match (&self.controllers, port) {
                        (Some(c), Some(port)) => edit_file(c, port).unwrap_or_default(),
                        _ => String::new(),
                    };
                    if let Some(app) = self.app.upgrade() {
                        app.global::<ControllerEditor>()
                            .invoke_show_mapping(file.into());
                    }
                }
                "quick" => {
                    if let Some(app) = self.app.upgrade() {
                        app.global::<crate::MapMode>().invoke_quick_map(port.into());
                    }
                }
                _ => {}
            }
            return;
        }
        match id {
            // A new checklist line; empty ones left from before go.
            "prep_line_add" => {
                let mut lines = settings::prep_lines();
                lines.push(String::new());
                settings::set_prep_lines(&lines);
            }
            "stage_inbox_add" => choose(
                id,
                "Choose a folder to watch for new music",
                true,
                self.chosen_tx.clone(),
            ),
            settings::STAGE_TRIAGE => choose(
                id,
                "Choose the In Prep folder",
                true,
                self.chosen_tx.clone(),
            ),
            settings::STAGE_COLLECTION => choose(
                id,
                "Choose the Play Ready folder",
                true,
                self.chosen_tx.clone(),
            ),
            "music_folder_add" => choose(id, "Choose a music folder", true, self.chosen_tx.clone()),
            settings::ALBUMS_FOLDER => choose(
                id,
                "Choose your albums folder",
                true,
                self.chosen_tx.clone(),
            ),
            settings::MIXED_SETS_FOLDER => choose(
                id,
                "Choose your mixed sets folder",
                true,
                self.chosen_tx.clone(),
            ),
            "recording_folder" => choose(
                id,
                "Choose where to save recordings",
                true,
                self.chosen_tx.clone(),
            ),
            "show_recordings" => {
                let dir = settings::recording().dir;
                let _ = std::fs::create_dir_all(&dir);
                match self.recorder.file().filter(|f| f.exists()) {
                    Some(file) => crate::files::reveal(&file),
                    None => crate::files::reveal(&dir),
                };
            }
            "rescan" => {
                (self.rescan)();
                self.library_note =
                    "Updating… new tracks appear in your collection as they are found.".into();
            }
            "report_bug" => {
                crate::spotify::open_in_browser(&crate::diagnostics::issue_url("bug_report.yml"));
            }
            "suggest_feature" => {
                crate::spotify::open_in_browser(&crate::diagnostics::issue_url(
                    "feature_request.yml",
                ));
            }
            "show_logs" => {
                if let Some(dir) = crate::diagnostics::logs_dir() {
                    let _ = std::fs::create_dir_all(&dir);
                    crate::files::reveal(&dir);
                }
            }
            "add_tag_field" | "remove_tag_field" => {
                let mut names = settings::field_names();
                if id == "add_tag_field" {
                    names.tags.push(format!("Tag {}", names.tags.len() + 1));
                } else if names.tags.len() > 1 {
                    names.tags.pop();
                }
                set_field_names(names);
            }
            "storage_confirm" => return self.storage_confirmed(),
            "storage_cancel" => self.storage_pending = None,
            _ => return,
        }
        self.refresh();
    }

    fn set(&self, group: &str, key: &str, value: f64) {
        if let Some(c) = self.registry.get(group, key) {
            c.set(value);
        }
    }

    fn each_deck(&self, key: &str, value: f64) {
        for d in 0..NUM_DECKS {
            self.set(&deck_group(d), key, value);
        }
    }
}

/// The decks' `rate_dir`: -1 unless the tempo faders are set to speed up
/// as they go up.
fn rate_dir() -> f64 {
    if settings::flag("tempo_up_faster", false) {
        1.0
    } else {
        -1.0
    }
}

/// Keeps what the DJ calls their fields; the browser picks it up.
fn set_field_names(names: query::FieldNames) {
    settings::set_field_names(&names);
    query::set_field_names(settings::field_names());
}

/// The DJ's own mappings that stand alone, made with Quick map.
fn quick_maps() -> Vec<own::OwnMapping> {
    settings::own_mappings_dir()
        .map(|d| own::all(&d))
        .unwrap_or_default()
        .into_iter()
        .filter(|m| m.based_on.is_empty())
        .collect()
}

/// The mapping file a controller's Edit opens: the one it uses, or, for a
/// stand-in device, the one named as running.
fn edit_file(c: &Controllers, port: &Port) -> Option<String> {
    // HID mappings are all script: the editor has nothing to show.
    if port.hid {
        return None;
    }
    port.mapping_file.clone().or_else(|| {
        let running = port.running.as_ref()?;
        c.mappings()
            .into_iter()
            .find(|(name, _)| name == running)
            .map(|(_, file)| file)
    })
}

/// A station's row on Settings › Broadcast: `bc:<index>:<field>`.
fn station_field(id: &str) -> Option<(usize, &str)> {
    let rest = id.strip_prefix("bc:")?;
    let (i, field) = rest.split_once(':')?;
    Some((i.parse().ok()?, field))
}

impl Prefs {
    /// Settings › Broadcast: the stations to stream to, the now-playing
    /// text, and what a public station needs.
    fn broadcast_page(&mut self) -> Vec<PrefItem> {
        use opendj_broadcast::{Format, ServerKind};
        let Some(b) = self.broadcast.clone() else {
            return vec![item(
                "",
                NOTE,
                "",
                "Broadcasting isn't available: the audio engine didn't start.",
            )];
        };
        let stations = broadcast::stations();
        let mut items = vec![
            switch(
                broadcast::ENABLED,
                "Broadcasting",
                "Shows ON AIR in the top bar, once a station below is set up and switched on.",
                settings::flag(broadcast::ENABLED, false),
            ),
            item("broadcast", HEADING, "STATIONS", ""),
        ];
        if stations.is_empty() {
            items.push(item(
                "",
                NOTE,
                "",
                "Stream your mix to an internet radio: add the Icecast or Shoutcast server your \
                 station or host gave you, then press ON AIR in the top bar.",
            ));
        }
        for (i, s) in stations.iter().enumerate() {
            let id = |field: &str| format!("bc:{i}:{field}");
            let set_up = !s.host.trim().is_empty();
            items.push(switch(
                &id("enabled"),
                &s.name,
                &if set_up {
                    format!("{}. Streams when you go on air.", s.summary())
                } else {
                    "Not set up yet: fill in its server.".to_string()
                },
                s.enabled,
            ));
            if self.editing_station != Some(i) {
                items.push(PrefItem {
                    action: "Edit".into(),
                    ..item(
                        &id("edit"),
                        BUTTON,
                        "Server and stream",
                        self.station_notes.get(&s.id).map_or("", |n| n.as_str()),
                    )
                });
                continue;
            }
            let text = |field: &str, title: &str, detail: &str, value: &str| PrefItem {
                value: value.into(),
                ..item(&id(field), TEXT, title, detail)
            };
            let icecast = s.server == ServerKind::Icecast;
            items.push(text("name", "Name", "What openDJ calls it.", &s.name));
            items.push(PrefItem {
                choices: strings(&["Icecast 2", "Shoutcast 1"]),
                chosen: if icecast { 0 } else { 1 },
                ..item(
                    &id("server"),
                    SEGMENTED,
                    "Server",
                    "Most hosts run Icecast. Shoutcast 2 servers take Shoutcast 1 streams.",
                )
            });
            items.push(text(
                "host",
                "Address",
                "The server's host name or IP address, such as radio.example.org.",
                &s.host,
            ));
            items.push(text("port", "Port", "", &s.port.to_string()));
            if icecast {
                items.push(text(
                    "mount",
                    "Mount point",
                    "Where listeners find the stream, such as /live.",
                    &s.mount,
                ));
                items.push(text("user", "User", "Usually source.", &s.user));
            }
            items.push(PrefItem {
                value: b.password(&s.id).into(),
                on: true,
                ..item(
                    &id("password"),
                    TEXT,
                    "Password",
                    "The source password. Kept in your system's keychain, not in openDJ's \
                     settings.",
                )
            });
            items.push(switch(
                &id("tls"),
                "Secure connection (TLS)",
                "Only if the server takes source connections over https.",
                s.tls,
            ));
            if icecast {
                items.push(PrefItem {
                    choices: strings(&["MP3", "Opus"]),
                    chosen: if s.format == Format::Opus { 1 } else { 0 },
                    ..item(
                        &id("format"),
                        SEGMENTED,
                        "Format",
                        "MP3 plays everywhere and shows each track's title. Opus sounds \
                         better at low bitrates.",
                    )
                });
            }
            let rates: Vec<String> = BITRATES.iter().map(|r| format!("{r} kb/s")).collect();
            items.push(PrefItem {
                choices: strings(&rates),
                chosen: BITRATES.iter().position(|&r| r == s.bitrate).unwrap_or(2) as i32,
                ..item(
                    &id("bitrate"),
                    MENU,
                    "Bitrate",
                    "Higher sounds better and needs a faster upload. 128 kb/s MP3 or 96 kb/s \
                     Opus suit most radios.",
                )
            });
            if s.effective_format() == Format::Opus {
                items.push(switch(
                    &id("opus_titles"),
                    "Send each new title in the stream",
                    "Opus carries titles in the stream itself, by starting it again at each \
                     track. Browsers and VLC follow; some players stop. Off, listeners see the \
                     first title only.",
                    s.opus_titles,
                ));
            }
            items.push(text(
                "stream_name",
                "Stream name",
                "Shown to listeners; the station's name when empty.",
                &s.stream_name,
            ));
            items.push(text("genre", "Genre", "", &s.genre));
            items.push(text("website", "Website", "", &s.website));
            items.push(text("description", "Description", "", &s.description));
            items.push(switch(
                &id("public"),
                "List in the directory",
                "Asks the server to list the stream in its public directory.",
                s.public,
            ));
            items.push(PrefItem {
                action: "Test".into(),
                ..item(
                    &id("test"),
                    BUTTON,
                    "Test the connection",
                    self.station_notes.get(&s.id).map_or(
                        "Connects with these settings and hangs up, without going on air.",
                        |n| n.as_str(),
                    ),
                )
            });
            let confirm = self.removing.as_deref() == Some(id("remove").as_str());
            items.push(PrefItem {
                action: if confirm { "Remove" } else { "Remove…" }.into(),
                ..item(
                    &id("remove"),
                    BUTTON,
                    "Remove this station",
                    if confirm {
                        "Press Remove again to remove it and its password."
                    } else {
                        ""
                    },
                )
            });
            items.push(PrefItem {
                action: "Done".into(),
                ..item(&id("edit"), BUTTON, "", "")
            });
        }
        items.push(PrefItem {
            action: "Add".into(),
            ..item("bc_add", BUTTON, "Add a station", "")
        });
        items.push(item("", HEADING, "NOW PLAYING", ""));
        items.push(PrefItem {
            value: settings::string(broadcast::TITLE_TEMPLATE).into(),
            ..item(
                broadcast::TITLE_TEMPLATE,
                TEXT,
                "Title sent to listeners",
                "The track heard most, a couple of seconds after it takes over. {artist}, \
                 {title} and {dj} are filled in; plain text is sent as it is. Empty is \
                 {artist} - {title}.",
            )
        });
        items.push(PrefItem {
            value: settings::string(broadcast::DJ_NAME).into(),
            ..item(
                broadcast::DJ_NAME,
                TEXT,
                "DJ name",
                "For {dj} in the title.",
            )
        });
        items.push(item("", HEADING, "BEFORE YOU GO ON AIR", ""));
        items.push(item(
            "",
            NOTE,
            "",
            "While on air, the limiter keeps peaks under the ceiling set in Settings › Audio › \
             Loudness, and the ON AIR panel's loudness meter shows how loud the stream is \
             against your target.",
        ));
        items.push(item(
            "",
            NOTE,
            "",
            "Playing music to the public needs licences in most countries, from bodies such as \
             PRS, GEMA, SACEM, ASCAP, BMI and SoundExchange. Many hosts include them; check \
             yours before you stream other people's music.",
        ));
        if let Some(trouble) = b.keychain_trouble() {
            items.push(item(
                "",
                NOTE,
                "",
                &format!(
                    "Passwords can't be kept in your system's keychain ({trouble}), so openDJ \
                     remembers them until it quits."
                ),
            ));
        }
        items
    }

    /// Changes station `i` and tells the broadcaster.
    fn edit_station(&mut self, i: usize, f: impl FnOnce(&mut opendj_broadcast::Station)) {
        if let Some(b) = &self.broadcast {
            b.edit(i, f);
        }
    }

    /// Settings › Broadcast's buttons; false if `id` isn't one of them.
    fn broadcast_pressed(&mut self, id: &str) -> bool {
        let Some(b) = self.broadcast.clone() else {
            return false;
        };
        if id == "bc_add" {
            self.editing_station = Some(b.add());
            self.refresh();
            return true;
        }
        let Some((i, field)) = station_field(id) else {
            return false;
        };
        match field {
            "edit" => {
                self.editing_station = (self.editing_station != Some(i)).then_some(i);
                self.removing = None;
            }
            "remove" => {
                if self.removing.as_deref() == Some(id) {
                    b.remove(i);
                    self.removing = None;
                    self.editing_station = None;
                } else {
                    self.removing = Some(id.to_string());
                }
            }
            "test" => {
                let Some(station) = broadcast::stations().into_iter().nth(i) else {
                    return true;
                };
                self.station_notes
                    .insert(station.id.clone(), "Connecting…".into());
                let tx = self.tested_tx.clone();
                let sid = station.id.clone();
                b.test(i, move |result| {
                    let note = match result {
                        Ok(()) => {
                            "Connected: the server took the stream. Ready to go on air.".to_string()
                        }
                        Err(e) => e,
                    };
                    let _ = tx.send((sid, note));
                });
            }
            _ => return false,
        }
        self.refresh();
        true
    }
}

/// How the plugin list is narrowed; kept while Settings is open.
#[derive(Default)]
struct PluginView {
    /// The format chips that are on ("CLAP", "VST3", "AU"); none is all.
    formats: Vec<String>,
    search: String,
    /// Makers opened in the list.
    open: std::collections::HashSet<String>,
}

/// The formats in `list`, in the chips' order.
fn plugin_formats(list: &[Found]) -> Vec<String> {
    ["CLAP", "VST3", "AU"]
        .into_iter()
        .filter(|f| list.iter().any(|p| p.format == *f))
        .map(String::from)
        .collect()
}

/// `list` narrowed to `formats` (none is all) and to plugins whose name or
/// maker has every word of `search`, by maker, makers and plugins A to Z.
fn plugin_groups(list: &[Found], formats: &[String], search: &str) -> Vec<(String, Vec<Found>)> {
    let words: Vec<String> = search.split_whitespace().map(str::to_lowercase).collect();
    // By the maker's name in lower case, so "FabFilter" and "fabfilter"
    // are one maker, shown as first spelt.
    let mut groups: std::collections::BTreeMap<String, (String, Vec<Found>)> = Default::default();
    for f in list {
        if !formats.is_empty() && !formats.iter().any(|x| x == f.format) {
            continue;
        }
        let maker = match f.vendor.trim() {
            "" => "Other".to_string(),
            v => v.to_string(),
        };
        let text = format!("{} {}", f.name, maker).to_lowercase();
        if !words.iter().all(|w| text.contains(w.as_str())) {
            continue;
        }
        groups
            .entry(maker.to_lowercase())
            .or_insert_with(|| (maker, Vec::new()))
            .1
            .push(f.clone());
    }
    groups
        .into_values()
        .map(|(maker, mut fs)| {
            fs.sort_by(|a, b| {
                (a.name.to_lowercase(), a.format).cmp(&(b.name.to_lowercase(), b.format))
            });
            (maker, fs)
        })
        .collect()
}

/// Makes the rows in `model` match `items`, changing only the rows that
/// differ, so the rows that stay keep their state (a search box its
/// focus). False when `model` isn't a list that can be changed in place.
fn patch_rows(model: &ModelRc<PrefItem>, items: &[PrefItem]) -> bool {
    use slint::Model;
    let Some(rows) = model.as_any().downcast_ref::<VecModel<PrefItem>>() else {
        return false;
    };
    for (i, it) in items.iter().enumerate() {
        match rows.row_data(i) {
            Some(old) if old == *it => {}
            Some(_) => rows.set_row_data(i, it.clone()),
            None => rows.push(it.clone()),
        }
    }
    while rows.row_count() > items.len() {
        rows.remove(rows.row_count() - 1);
    }
    true
}

fn item(id: &str, kind: i32, title: &str, detail: &str) -> PrefItem {
    PrefItem {
        id: id.into(),
        kind,
        title: title.into(),
        detail: detail.into(),
        ..Default::default()
    }
}

/// Settings › Broadcast: the loudness meter's target and the limiter.
fn loudness_items() -> Vec<PrefItem> {
    vec![
        item("loudness", HEADING, "LOUDNESS", ""),
        {
            let target = settings::number("loudness_target", meter::DEFAULT_LOUDNESS_TARGET);
            PrefItem {
                choices: strings(&LOUDNESS_TARGETS.map(|t| t.1)),
                chosen: LOUDNESS_TARGETS
                    .iter()
                    .position(|t| (t.0 - target).abs() < 1e-6)
                    .unwrap_or(0) as i32,
                ..item(
                    "loudness_target",
                    MENU,
                    "Loudness target",
                    "Where the loudness meter in the ON AIR panel shows green. It doesn't change any level.",
                )
            }
        },
        {
            let ceiling = settings::number("air_ceiling", engine::DEFAULT_AIR_CEILING_DB);
            PrefItem {
                choices: strings(&AIR_CEILINGS.map(|t| t.1)),
                chosen: AIR_CEILINGS
                    .iter()
                    .position(|t| (t.0 - ceiling).abs() < 1e-6)
                    .unwrap_or(0) as i32,
                ..item(
                    "air_ceiling",
                    MENU,
                    "Limiter while recording or on air",
                    "Keeps peaks, also those between samples, under this level so MP3, AAC and Opus files and streams don't clip.",
                )
            }
        },
    ]
}

/// The Clean-up heading on Settings › Collection › Health, which folds.
const CLEANUP_FOLD: &str = "cleanup#fold";

/// The music folders' list on Settings › Collection.
const MUSIC_FOLDERS_LIST: &str = "music_folders";
/// The plugin folders' list on Settings › Plugins › Folders.
const PLUGIN_FOLDERS_LIST: &str = "audio_plugin_folders";

/// A list of folders shown by name, as in Finder, with + (pick one in the
/// system's folder picker; on a `single` list, in place of the one there)
/// and − (take the selected one off) under it.
/// `info` goes in the ⓘ beside the title (none when empty). A `single`
/// list holds one folder; a `fixed` one can't be changed here. The row
/// presses `id#add`, `id#remove:<index>` and `id#reveal:<path>`.
fn folder_list(
    id: &str,
    title: &str,
    info: &str,
    folders: &[PathBuf],
    single: bool,
    fixed: bool,
) -> PrefItem {
    let names: Vec<String> = folders.iter().map(|p| folder_name(p)).collect();
    let paths: Vec<String> = folders
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    PrefItem {
        choices: strings(&names),
        paths: strings(&paths),
        on: single,
        action: if fixed { "fixed".into() } else { "".into() },
        ..item(id, FOLDER_LIST, title, info)
    }
}

/// A small outline of a controller for its row on Settings › Controllers,
/// as Path commands in a 100 × 60 box: its picture's panels and controls
/// when openDJ has one for the mapping, else a generic two-deck shape.
fn controller_outline(mapping_file: Option<&str>) -> String {
    use std::fmt::Write;
    let Some(layout) = mapping_file.and_then(opendj_controllers::layout::Layout::for_file) else {
        return "M 6 10 L 94 10 L 94 50 L 6 50 Z M 20 18 A 12 12 0 1 1 19.99 18 Z \
                M 80 18 A 12 12 0 1 1 79.99 18 Z M 46 18 L 46 42 M 54 18 L 54 42"
            .to_string();
    };
    let (w, h) = (layout.width.max(1.0), layout.height.max(1.0));
    let scale = (100.0 / w).min(60.0 / h);
    let (ox, oy) = ((100.0 - w * scale) / 2.0, (60.0 - h * scale) / 2.0);
    let mut out = String::new();
    let rect = |out: &mut String, x: f32, y: f32, rw: f32, rh: f32| {
        let (x0, y0) = (ox + x * scale, oy + y * scale);
        let (x1, y1) = (x0 + rw * scale, y0 + rh * scale);
        let _ = write!(
            out,
            "M {x0:.1} {y0:.1} L {x1:.1} {y0:.1} L {x1:.1} {y1:.1} L {x0:.1} {y1:.1} Z "
        );
    };
    for p in &layout.panels {
        rect(&mut out, p[0], p[1], p[2], p[3]);
    }
    for p in &layout.parts {
        if p.round {
            let r = p.w.min(p.h) * scale / 2.0;
            let (cx, cy) = (
                ox + (p.x + p.w / 2.0) * scale,
                oy + (p.y + p.h / 2.0) * scale,
            );
            let _ = write!(
                out,
                "M {:.1} {:.1} A {r:.1} {r:.1} 0 1 1 {:.1} {:.1} Z ",
                cx,
                cy - r,
                cx - 0.01,
                cy - r
            );
        } else {
            rect(&mut out, p.x, p.y, p.w, p.h);
        }
    }
    out.trim_end().to_string()
}

/// What an imported library is called in its list: the folder its file is
/// in, as the app's own folder ("VirtualDJ"), or the file's name.
fn library_name(p: &Path) -> String {
    p.parent()
        .filter(|d| d.file_name().is_some())
        .map(folder_name)
        .unwrap_or_else(|| folder_name(p))
}

/// What a folder is called in a list: its own name, not its whole path.
fn folder_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string_lossy().into_owned())
}

/// A shortcut's row on the Keyboard page.
fn shortcut_item(r: &shortcuts::Row) -> PrefItem {
    let (detail, wrong) = shortcuts::describe(&r.script);
    let verbs: Vec<&str> = opendj_script::VERBS.iter().map(|v| v.name).collect();
    PrefItem {
        value: r.script.as_str().into(),
        on: wrong,
        choices: strings(&verbs),
        action: match (r.default, r.changed) {
            (true, true) => "Reset",
            (true, false) => "Turn Off",
            (false, _) => "Remove",
        }
        .into(),
        ..item(
            &format!("shortcut:{}", r.key),
            SHORTCUT,
            &shortcuts::display(&r.key),
            &detail,
        )
    }
}

fn switch(id: &str, title: &str, detail: &str, on: bool) -> PrefItem {
    PrefItem {
        on,
        ..item(id, SWITCH, title, detail)
    }
}

fn strings<S: AsRef<str>>(v: &[S]) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        v.iter()
            .map(|s| s.as_ref().into())
            .collect::<Vec<SharedString>>(),
    ))
}

/// How big the sources' cache may grow, in GB.
const SOURCES_CACHE_GB: [f64; 6] = [2.0, 5.0, 10.0, 20.0, 50.0, 100.0];

/// How many GB of high-quality stems may be kept; 0 is no limit.
const STEMS_CACHE_GB: [f64; 6] = [2.0, 5.0, 10.0, 20.0, 50.0, 0.0];
const DEFAULT_STEMS_CACHE_GB: f64 = 10.0;

/// Gives the engine the stems cache limit and trims the cache to it in
/// the background.
fn apply_stems_cache_limit() {
    let gb = settings::number("stems_cache_gb", DEFAULT_STEMS_CACHE_GB);
    separation::set_cache_limit((gb * (1u64 << 30) as f64) as u64);
    let _ = std::thread::Builder::new()
        .name("stems-cache".into())
        .spawn(|| separation::trim_cache(&[]));
}

fn size(bytes: u64) -> String {
    match bytes {
        b if b >= 1 << 30 => format!("{:.1} GB", b as f64 / (1u64 << 30) as f64),
        b if b >= 1 << 20 => format!("{:.0} MB", b as f64 / (1u64 << 20) as f64),
        b => format!("{:.0} KB", b as f64 / 1024.0),
    }
}

/// A path as a row shows it, or `none`.
fn show(p: Option<PathBuf>, none: &str) -> String {
    p.map_or(none.to_string(), |p| p.to_string_lossy().into_owned())
}

/// The importer with this id.
fn importer(id: &str) -> Option<Box<dyn LibraryImport>> {
    crate::stores::importers()
        .into_iter()
        .find(|i| i.id() == id)
}

/// Whose tags win when a track's differ.
fn theirs_row(id: &str, title: &str, theirs_win: bool) -> PrefItem {
    PrefItem {
        choices: strings(&[title, "openDJ"]),
        chosen: if theirs_win { 0 } else { 1 },
        ..item(
            id,
            SEGMENTED,
            "When tags differ",
            "Whose titles, ratings and colors a track keeps.",
        )
    }
}

/// The step of the import wizard shown, with its buttons.
fn wizard_rows(importer: &dyn LibraryImport, w: &WizardState) -> Vec<PrefItem> {
    let title = importer.title();
    match w.step {
        0 => vec![
            PrefItem {
                value: show(w.database.clone(), "Not found").into(),
                action: "Choose…".into(),
                ..item("import_wizard_db", PATH, &format!("{title} database"), "")
            },
            item("", NOTE, "", &w.preview),
        ],
        1 => match &w.picker {
            Some(p) if !p.nodes[0].children.is_empty() => pick_rows(p),
            Some(_) => vec![item(
                "",
                NOTE,
                "",
                &format!("{title} has no lists to bring in, only tracks."),
            )],
            None => importer
                .list_groups()
                .into_iter()
                .map(|g| {
                    switch(
                        &format!("import_wizard_group:{}", g.id),
                        g.title,
                        g.detail,
                        w.groups.contains(&g.id),
                    )
                })
                .collect(),
        },
        _ => vec![
            theirs_row("import_wizard_theirs", title, w.theirs_win),
            item(
                "",
                NOTE,
                "",
                &format!(
                    "Grids and cues you edit in openDJ stay either way. Import brings it in \
                     now. openDJ checks {title}'s files each time it starts and brings in \
                     what changed; you can change these choices in Settings › Collection."
                ),
            ),
        ],
    }
}

/// The tree of lists to tick: a folder's tick box does all of it, its
/// arrow opens it.
fn pick_rows(p: &Picker) -> Vec<PrefItem> {
    p.visible()
        .into_iter()
        .map(|i| {
            let n = &p.nodes[i];
            let ticked = if n.list.is_some() {
                if n.on {
                    Ticked::All
                } else {
                    Ticked::None
                }
            } else {
                p.ticked(i)
            };
            let detail = if i == 0 {
                p.summary()
            } else {
                n.detail.clone()
            };
            PrefItem {
                on: ticked == Ticked::All,
                value: if ticked == Ticked::Some { "some" } else { "" }.into(),
                chosen: n.depth as i32,
                action: match (n.list.is_some(), n.open) {
                    (true, _) => "",
                    (false, true) => "▾",
                    (false, false) => "▸",
                }
                .into(),
                ..item(&format!("import_pick:{i}"), TICK, &n.title, &detail)
            }
        })
        .collect()
}

/// Shows the import wizard's dialog, or hides it.
fn show_wizard(app: &AppWindow, w: Option<&WizardState>) {
    let g = app.global::<ImportWizard>();
    let Some((w, importer)) = w.and_then(|w| Some((w, importer(&w.importer)?))) else {
        g.set_open(false);
        return;
    };
    let title = importer.title();
    let step = match w.step {
        0 => format!("Find your {title} library"),
        1 => "Choose which lists come across".to_string(),
        _ => "Choose whose tags win when they differ".to_string(),
    };
    if w.only_lists {
        g.set_title(format!("{title} lists").into());
        g.set_step("Tick the lists to bring in. A folder's tick box does everything in it.".into());
    } else {
        g.set_title(format!("Import from {title}").into());
        g.set_step(format!("Step {} of 3 · {step}", w.step + 1).into());
    }
    g.set_items(ModelRc::new(VecModel::from(wizard_rows(
        importer.as_ref(),
        w,
    ))));
    g.set_next_label(
        if w.only_lists {
            "Save"
        } else if w.step < 2 {
            "Next"
        } else {
            "Import"
        }
        .into(),
    );
    g.set_can_next(w.database.is_some() && w.readable);
    g.set_can_back(w.step > 0 && !w.only_lists);
    g.set_open(true);
}

/// Opens the system's dialog for picking a folder or a file, on its own
/// thread; the path comes back through `tx`. macOS uses AppleScript's,
/// Windows its own Open dialog, Linux zenity's.
fn choose(id: &str, prompt: &str, folder: bool, tx: Sender<(String, PathBuf)>) {
    let id = id.to_string();
    open_dialog(prompt, folder, move |path| {
        let _ = tx.send((id, path));
    });
}

/// Opens the system's dialog for picking a file; `picked` gets the path on
/// the dialog's thread, unless it was cancelled.
pub fn choose_file(prompt: &str, picked: impl FnOnce(PathBuf) + Send + 'static) {
    open_dialog(prompt, false, picked);
}

/// Opens the system's dialog for picking a folder; `picked` gets the path
/// on the dialog's thread, unless it was cancelled.
pub fn choose_folder(prompt: &str, picked: impl FnOnce(PathBuf) + Send + 'static) {
    open_dialog(prompt, true, picked);
}

fn open_dialog(prompt: &str, folder: bool, picked: impl FnOnce(PathBuf) + Send + 'static) {
    let prompt = prompt.to_string();
    std::thread::spawn(move || {
        if let Some(path) = system_dialog(&prompt, folder) {
            picked(path);
        }
    });
}

/// The path picked in the system's dialog, or `None` if it was cancelled.
#[cfg(windows)]
fn system_dialog(prompt: &str, folder: bool) -> Option<PathBuf> {
    let dialog = rfd::FileDialog::new().set_title(prompt);
    if folder {
        dialog.pick_folder()
    } else {
        dialog.pick_file()
    }
}

/// The path picked in the system's dialog, or `None` if it was cancelled.
#[cfg(not(windows))]
fn system_dialog(prompt: &str, folder: bool) -> Option<PathBuf> {
    let output = if cfg!(target_os = "macos") {
        let what = if folder { "folder" } else { "file" };
        std::process::Command::new("osascript")
            .arg("-e")
            .arg(format!(
                "POSIX path of (choose {what} with prompt \"{prompt}\")"
            ))
            .output()
    } else {
        let mut cmd = std::process::Command::new("zenity");
        cmd.arg("--file-selection").arg(format!("--title={prompt}"));
        if folder {
            cmd.arg("--directory");
        }
        cmd.output()
    };
    match output {
        Ok(out) if out.status.success() => {
            let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            (!path.is_empty()).then(|| PathBuf::from(path))
        }
        Ok(_) => None, // Cancelled.
        Err(e) => {
            eprintln!("can't open a file dialog: {e}");
            None
        }
    }
}

/// The channels an input can take from a device with `channels` of them:
/// each one for the microphone; each pair, then each one (mono), for an
/// auxiliary input.
fn channel_choices(role: InputRole, channels: u16) -> Vec<(u16, bool)> {
    let mut out = Vec::new();
    if role.can_be_stereo() {
        out.extend(
            (0..channels.saturating_sub(1))
                .step_by(2)
                .map(|c| (c, false)),
        );
    }
    out.extend((0..channels).map(|c| (c, true)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendj_engine::plugins::Plugin;

    #[test]
    fn auxiliary_inputs_offer_pairs_then_mono_channels() {
        assert_eq!(
            channel_choices(InputRole::Auxiliary2, 4),
            [
                (0, false),
                (2, false),
                (0, true),
                (1, true),
                (2, true),
                (3, true)
            ]
        );
        assert_eq!(channel_choices(InputRole::Auxiliary, 1), [(0, true)]);
        assert_eq!(
            channel_choices(InputRole::Microphone, 2),
            [(0, true), (1, true)]
        );
    }

    fn found(name: &str, vendor: &str, format: &'static str) -> Found {
        Found {
            key: format!("{format}:{name}"),
            name: name.into(),
            vendor: vendor.into(),
            format,
            plugin: Plugin::Au { id: name.into() },
            features: Vec::new(),
        }
    }

    #[test]
    fn plugins_are_grouped_by_maker_and_narrowed() {
        let list = vec![
            found("Pro-Q 3", "FabFilter", "VST3"),
            found("Pro-Q 3", "FabFilter", "CLAP"),
            found("Diva", "u-he", "CLAP"),
            found("Thing", "", "AU"),
            found("Saturn 2", "fabfilter", "AU"),
        ];
        assert_eq!(plugin_formats(&list), ["CLAP", "VST3", "AU"]);
        let all = plugin_groups(&list, &[], "");
        let makers: Vec<&str> = all.iter().map(|(m, _)| m.as_str()).collect();
        // A to Z whatever the case; a maker spelt two ways is one maker.
        assert_eq!(makers, ["FabFilter", "Other", "u-he"]);
        assert_eq!(all[0].1.len(), 3);
        let clap = plugin_groups(&list, &["CLAP".into()], "");
        assert_eq!(clap.iter().map(|(_, f)| f.len()).sum::<usize>(), 2);
        let searched = plugin_groups(&list, &[], "fab pro");
        assert_eq!(searched.len(), 1);
        assert_eq!(searched[0].1.len(), 2);
        assert_eq!(plugin_groups(&list, &[], "SATURN").len(), 1);
        assert!(plugin_groups(&list, &["AU".into()], "diva").is_empty());
    }

    #[test]
    fn rows_change_in_place() {
        let rows = ModelRc::new(VecModel::from(vec![
            item("a", NOTE, "A", ""),
            item("b", NOTE, "B", ""),
            item("c", NOTE, "C", ""),
        ]));
        let new = [item("a", NOTE, "A", ""), item("x", TICK, "X", "")];
        assert!(patch_rows(&rows, &new));
        let got: Vec<PrefItem> = slint::Model::iter(&rows).collect();
        assert_eq!(got, new);
        let other: ModelRc<PrefItem> = ModelRc::default();
        assert!(!patch_rows(&other, &new));
    }
}
