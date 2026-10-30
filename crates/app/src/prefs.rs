//! The settings window: builds the rows of each page from what is set now,
//! applies changes to the engine's controls at once and keeps them in
//! settings.json. Saved choices are applied again at start.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use crate::list_picker::{Picker, Ticked};
use opendj_control::ControlRegistry;
use opendj_controllers::{device_key, support, Controllers, MappingChoice, Port};
use opendj_engine::inputs::{DEFAULT_DUCK_STRENGTH, DUCK_AUTO};
use opendj_engine::{
    analysis, cache, channel_names, deck_group, input_devices, key, InputRole, InputRoute,
    RecordFormat, Recorder, BUFFER_SIZES, DEFAULT_AUTO_GAIN_TARGET, NUM_DECKS, SAMPLE_RATES,
};
use opendj_library::query;
use opendj_plugins::{LibraryImport, Plugin, SettingKind};
use serde_json::json;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::tour::Part;
use crate::{
    settings, short_port_name, shortcuts, AppWindow, AudioSettings, ControllerEditor, ImportWizard,
    KeyNotation, PrefItem, Preferences, Tour,
};

const AUDIO: i32 = 0;
const CONTROLLERS: i32 = 1;
const LIBRARY: i32 = 2;
/// Settings › Library's tabs.
const LIBRARY_TABS: [&str; 4] = ["General", "Stages", "Fields", "Import"];
/// The Import tab's place in LIBRARY_TABS.
const IMPORT_TAB: usize = 3;
/// Other DJ apps whose libraries openDJ can't bring in yet, listed on the
/// Import tab so the DJ sees what's coming.
const COMING_IMPORTS: [(&str, &str); 2] = [
    ("Serato DJ", "Tracks, cues, loops and crates."),
    (
        "Traktor",
        "Tracks, cues, loops and playlists from collection.nml.",
    ),
];
const ANALYSIS: i32 = 3;
const DECKS: i32 = 4;
const MIXER: i32 = 5;
const KEYBOARD: i32 = 6;
const FEEDBACK: i32 = 7;

const SWITCH: i32 = 0;
const MENU: i32 = 1;
const BUTTON: i32 = 2;
const PATH: i32 = 3;
const OUTPUTS: i32 = 4;
const LEVELS: i32 = 5;
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

/// Tempo fader ranges, as fractions of normal speed.
const RATE_RANGES: [f64; 8] = [0.04, 0.06, 0.08, 0.10, 0.16, 0.24, 0.50, 1.00];
const DEFAULT_RATE_RANGE: f64 = 0.08;
/// Analysis tempo ranges, plus our default.
const BPM_RANGES: [(f64, f64); 5] = [
    (60.0, 120.0),
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
    let (lo, hi) = bpm_range();
    analysis::set_bpm_range(lo, hi);
    key::set_detector(key_detector());
    key::set_mixing(key_mixing());
    set("[Controls]", "key_match_on_load", key_match_range() as f64);
    for (id, _, _) in AUTO_RESETS {
        set("[Controls]", id, f(settings::flag(id, true)));
    }
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
    /// What the last import change set going, shown on the Import tab.
    import_note: String,
    cache_note: String,
    /// The devices shown on the controllers page.
    ports: Vec<Port>,
    /// The devices under Add a controller, in its menu's order.
    hidden_ports: Vec<String>,
    /// Plugins with a page of their own, after the built-in pages.
    plugins: Vec<Box<dyn Plugin>>,
    /// Tells the rest of the app a plugin's settings changed, by its id.
    plugin_changed: Box<dyn Fn(&str)>,
    /// Opens the outputs again with the saved audio settings.
    audio_changed: Box<dyn Fn()>,
    /// Opens the inputs again with the saved audio settings, and tells
    /// each input's state (see [`opendj_engine::AudioEngine::input_status`]).
    inputs: InputHooks,
    /// What each input's menu offered last, by [`InputRole::index`].
    input_choices: [Vec<Option<InputRoute>>; 2],
    /// The import being set up, if any.
    wizard: Option<WizardState>,
    /// The import whose Remove was pressed once, waiting for a second press.
    removing: Option<String>,
    /// What the last change of a shortcut's key did, shown on the page.
    shortcut_note: String,
    /// Counts fills of the rows, so the tour's setup dialog, which shows
    /// some of the same rows, knows to fill its own again.
    pub generation: u64,
}

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
    ) -> Rc<RefCell<Prefs>> {
        let (chosen_tx, chosen_rx) = mpsc::channel();
        let plugins: Vec<Box<dyn Plugin>> = crate::stores::configured()
            .into_iter()
            .filter(|p| !p.settings().is_empty())
            .collect();
        let g = app.global::<Preferences>();
        let mut pages: Vec<SharedString> = slint::Model::iter(&g.get_pages()).collect();
        pages.extend(
            plugins
                .iter()
                .map(|p| SharedString::from(p.manifest().name)),
        );
        g.set_pages(ModelRc::new(VecModel::from(pages)));
        let prefs = Rc::new(RefCell::new(Prefs {
            app: app.as_weak(),
            registry,
            controllers,
            rescan,
            recorder,
            chosen_tx,
            chosen_rx,
            library_note: String::new(),
            import_note: String::new(),
            cache_note: String::new(),
            ports: Vec::new(),
            hidden_ports: Vec::new(),
            plugins,
            plugin_changed,
            audio_changed,
            inputs,
            input_choices: Default::default(),
            wizard: None,
            removing: None,
            shortcut_note: String::new(),
            generation: 0,
        }));
        let p = prefs.clone();
        g.on_changed(move || p.borrow_mut().shown());
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
        prefs
    }

    /// Called every frame: picks up chosen paths and plugged-in devices.
    pub fn poll(&mut self) {
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
        if g.get_open() && g.get_page() == CONTROLLERS && g.get_query().is_empty() {
            let ports = self
                .controllers
                .as_ref()
                .map(|c| c.ports())
                .unwrap_or_default();
            if ports != self.ports {
                self.refresh();
            }
        }
    }

    /// A music folder was added or removed: the library is updated, which
    /// brings in what is there and finds what is now outside.
    fn music_folders_changed(&mut self) {
        (self.rescan)();
        self.library_note = "Updating… new tracks appear in the library as they are found.".into();
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
        self.cache_note.clear();
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
            (AUDIO..FEEDBACK + 1 + self.plugins.len() as i32)
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
        g.set_items(ModelRc::new(VecModel::from(items)));
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
                        "Beeps for a few seconds in the master output.",
                    )
                });
                items.push(PrefItem {
                    action: "Play".into(),
                    ..item(
                        "tour_tone:2",
                        BUTTON,
                        "Test the headphones",
                        "Beeps for a few seconds in the headphones only.",
                    )
                });
                items.push(item(
                    "",
                    NOTE,
                    "",
                    "A controller with its own sound card, such as the DDJ-FLX4, is its own \
                     device: pick it for the master and headphones, on channels 1 & 2 and 3 & 4.",
                ));
                items
            }
            Part::Controller => {
                let mut items = vec![item(
                    "",
                    NOTE,
                    "",
                    if heard.is_empty() {
                        "Press a button or move a knob on your controller to check it's \
                         heard."
                    } else {
                        heard
                    },
                )];
                items.extend(self.controllers_page());
                items
            }
            Part::Library => {
                let mut items = self.music_folder_items();
                items.push(item("", HEADING, "FROM YOUR OTHER DJ APPS", ""));
                let mut found = false;
                let importers = crate::stores::importers();
                for i in &importers {
                    let Some(db) = settings::import_database(i.as_ref()) else {
                        continue;
                    };
                    found = true;
                    let title = i.title();
                    if settings::import_enabled(i.id()) {
                        items.push(item(
                            "",
                            NOTE,
                            &format!("Importing from {title}"),
                            &format!("openDJ brings in {title}'s library and keeps it up to date."),
                        ));
                    } else {
                        items.push(PrefItem {
                            action: "Set up…".into(),
                            ..item(
                                &format!("import_start:{}", i.id()),
                                BUTTON,
                                &format!("Import from {title}"),
                                &format!(
                                    "Found its library at {}. Brings in tracks, cues, grids \
                                     and lists.",
                                    db.display()
                                ),
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
                            "Found it on this computer. Its import is on the way.",
                        )
                    });
                }
                if !found {
                    items.push(item(
                        "",
                        NOTE,
                        "",
                        "No other DJ app's library turned up. You can import one later in \
                         Settings › Library › Import.",
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
                item(
                    "outputs",
                    OUTPUTS,
                    "Outputs",
                    "A device and channel pair for the master, headphone and booth mixes.",
                ),
                item(
                    "levels",
                    LEVELS,
                    "Headphones and booth",
                    "Cue mix blends the cue with the master in the headphones.",
                ),
                self.sample_rate_item(),
                self.buffer_item(),
                item("inputs", HEADING, "INPUTS", ""),
                self.input_item(InputRole::Microphone),
                self.input_item(InputRole::Auxiliary),
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
                {
                    // With a mixed sets folder, recordings go in it.
                    let in_sets = settings::mixed_sets_folder().is_some();
                    PrefItem {
                        value: settings::recording().dir.display().to_string().into(),
                        action: if in_sets { "" } else { "Choose…" }.into(),
                        ..item(
                            "recording_folder",
                            PATH,
                            "Recordings folder",
                            if in_sets {
                                "In your Mixed Sets folder (Settings › Library). Each mix is \
                                 saved with a .cue tracklist and a log of every control move."
                            } else {
                                "Each mix is saved with a .cue tracklist and a log of every \
                                 control move. Choose a Mixed Sets folder in Settings › Library \
                                 to keep them with your other sets."
                            },
                        )
                    }
                },
                PrefItem {
                    action: "Show".into(),
                    ..item(
                        "show_recordings",
                        BUTTON,
                        "Show recordings",
                        "Opens the folder, with the last recording selected.",
                    )
                },
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
                        choices: strings(&["Camelot · 8A", "Musical · Am"]),
                        chosen: if camelot { 0 } else { 1 },
                        ..item(
                            "key_notation",
                            SEGMENTED,
                            "Key notation",
                            "How keys are written on the decks and in the library.",
                        )
                    },
                ]
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
                            "Which keys the library marks in lilac as mixing with the track \
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
                             mix with it, with key lock on. The library marks those tracks with \
                             the shift they'd get.",
                        )
                    },
                    switch(
                        "quantize",
                        "Quantize",
                        "Each deck's Q button starts on: cues and loops snap to the beat, and play, CUE, hot cues and jumps land in step with the other playing deck.",
                        settings::flag("quantize", true),
                    ),
                    switch(
                        "protect_playing",
                        "Protect playing decks",
                        "A track can't be loaded onto a deck while it plays.",
                        settings::flag("protect_playing", true),
                    ),
                    item("auto_reset", HEADING, "AUTO RESET ON LOAD", ""),
                ]
                .into_iter()
                .chain(AUTO_RESETS.map(|(id, title, detail)| {
                    switch(id, title, detail, settings::flag(id, true))
                }))
                .collect()
            }
            MIXER => {
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
            p if p > FEEDBACK => self.plugin_page((p - FEEDBACK - 1) as usize),
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
            ],
            _ => Vec::new(),
        }
    }

    /// The rate the engine runs at now.
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

    /// The menu choosing `role`'s device and channels: None, then each
    /// input device's channels (pairs for the auxiliary input).
    fn input_item(&mut self, role: InputRole) -> PrefItem {
        let saved = settings::audio_setup()
            .inputs
            .into_iter()
            .find(|r| r.role == role);
        let width = role.channels();
        let mut names = vec!["None".to_string()];
        let mut choices = vec![None];
        for d in input_devices() {
            for channel in (0..d.channels).step_by(width as usize) {
                if channel + width > d.channels {
                    break;
                }
                names.push(format!("{} · {}", d.name, channel_names(channel, width)));
                choices.push(Some(InputRoute {
                    role,
                    device: d.name.clone(),
                    channel,
                }));
            }
        }
        if let Some(r) = &saved {
            if !choices.contains(&Some(r.clone())) {
                let device = if r.device.is_empty() {
                    "System default"
                } else {
                    &r.device
                };
                names.push(format!(
                    "{device} · {} (not connected)",
                    channel_names(r.channel, width)
                ));
                choices.push(Some(r.clone()));
            }
        }
        let chosen = choices.iter().position(|c| *c == saved).unwrap_or(0);
        self.input_choices[role.index()] = choices;
        let status = (self.inputs.status)()
            .get(role.index())
            .cloned()
            .unwrap_or_default();
        let (id, title, detail) = match role {
            InputRole::Microphone => (
                "microphone_input",
                "Microphone",
                "One channel of an input device. Turn it on with MIC; its effects are on the mixer's MIC page.",
            ),
            InputRole::Auxiliary => (
                "auxiliary_input",
                "Auxiliary",
                "A pair of channels, such as a turntable, a phone or another mixer, mixed in with the decks.",
            ),
        };
        // While a device is chosen, the row says how it is going.
        let detail = if saved.is_some() && !status.is_empty() {
            status
        } else {
            detail.to_string()
        };
        PrefItem {
            choices: strings(&names),
            chosen: chosen as i32,
            ..item(id, MENU, title, &detail)
        }
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
                "Smaller reacts faster to the controls; larger is safer from dropouts on a busy computer.",
            )
        }
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
        let shown = settings::controllers_shown();
        let (visible, hidden): (Vec<&Port>, Vec<&Port>) = self.ports.iter().partition(|port| {
            shown
                .get(&device_key(&port.name))
                .copied()
                .unwrap_or_else(|| {
                    port.running.is_some()
                        || c.auto_mapping(&port.name).is_some()
                        || matches!(c.choice(&port.name), MappingChoice::File(_))
                })
        });
        let mut items: Vec<PrefItem> = visible
            .iter()
            .map(|port| {
                let choice = c.choice(&port.name);
                let matched = c.auto_mapping(&port.name);
                let mut choices = vec![
                    match &matched {
                        Some(n) => format!("Automatic · {n}"),
                        None => "Automatic · none found".to_string(),
                    },
                    "Off".to_string(),
                ];
                choices.extend(mappings.iter().map(|(n, _)| n.clone()));
                let chosen = match &choice {
                    MappingChoice::Auto => 0,
                    MappingChoice::Off => 1,
                    MappingChoice::File(f) => mappings
                        .iter()
                        .position(|(_, m)| m == f)
                        .map_or(0, |i| i + 2),
                };
                let detail = match (&port.running, &choice, &port.mapping_file) {
                    (Some(m), _, _) => match port
                        .mapping_file
                        .as_deref()
                        .or_else(|| {
                            mappings
                                .iter()
                                .find(|(n, _)| n == m)
                                .map(|(_, f)| f.as_str())
                        })
                        .and_then(support::status)
                    {
                        Some(s) => format!("Running {m} · {}", s.label()),
                        None => format!("Running {m}"),
                    },
                    (None, MappingChoice::Off, _) => "Not used".to_string(),
                    (None, _, None) => {
                        "No Mixxx mapping matches this device: pick one.".to_string()
                    }
                    (None, _, Some(_)) => "Starting, or its mapping failed to load".to_string(),
                };
                // Edit opens the Controller Editor on the mapping it uses.
                let editable = edit_file(c, port).is_some() && choice != MappingChoice::Off;
                PrefItem {
                    choices: strings(&choices),
                    chosen: chosen as i32,
                    action: if editable { "Edit" } else { "" }.into(),
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
                "Plug in a controller openDJ has a mapping for and it shows up here within a \
                 second or so.",
            ));
        }
        self.hidden_ports = hidden.iter().map(|p| p.name.clone()).collect();
        if !self.hidden_ports.is_empty() {
            let names: Vec<&str> = self
                .hidden_ports
                .iter()
                .map(|p| short_port_name(p))
                .collect();
            items.push(PrefItem {
                choices: strings(&names),
                value: "Choose…".into(),
                ..item(
                    "controller_add",
                    MENU,
                    "Add a controller",
                    "Other MIDI devices plugged in, such as one without a mapping yet.",
                )
            });
        }
        items.push(PrefItem {
            action: "Refresh".into(),
            ..item(
                "controllers_refresh",
                BUTTON,
                "Find controllers",
                "Looks for controllers again and reconnects them, for one that is plugged in \
                 but not showing or not answering.",
            )
        });
        items.push(item(
            "",
            NOTE,
            "",
            "Controllers run Mixxx's own mappings. Automatic picks the one named after the device.",
        ));
        items
    }

    /// Settings › Library: the music folders, each with Remove, then Add.
    fn music_folder_items(&self) -> Vec<PrefItem> {
        let env = std::env::var_os("OPENDJ_MUSIC").is_some();
        let folders = settings::music_folders();
        let mut items = Vec::new();
        for (i, dir) in folders.iter().enumerate() {
            items.push(PrefItem {
                value: dir.to_string_lossy().into_owned().into(),
                action: if env { "".into() } else { "Remove".into() },
                ..item(
                    &format!("music_folder_remove:{i}"),
                    PATH,
                    if i == 0 { "Music folders" } else { "" },
                    if env && i == 0 {
                        "Set by OPENDJ_MUSIC for now."
                    } else {
                        ""
                    },
                )
            });
        }
        if !env {
            items.push(PrefItem {
                action: "Add…".into(),
                ..item(
                    "music_folder_add",
                    BUTTON,
                    if folders.is_empty() {
                        "Music folders"
                    } else {
                        "Add a music folder"
                    },
                    "Where openDJ looks for your music. Tracks elsewhere show up in Library \
                     Health under Outside folders.",
                )
            });
        }
        items
    }

    /// Settings › Library › Stages: the inbox folders, the triage and
    /// collection folders, and what happens to files between them.
    fn stage_items(&self) -> Vec<PrefItem> {
        let folders = settings::stage_folders();
        let mut items = vec![item(
            "",
            NOTE,
            "",
            "New music lands in the Inbox. Send what you like to Triage to prepare it \
                 (tags, cover, rating, tempo and key), then add it to the Collection. \
                 Subfolders of the Triage folder are triage crates.",
        )];
        for (i, dir) in folders.inbox.iter().enumerate() {
            items.push(PrefItem {
                value: dir.to_string_lossy().into_owned().into(),
                action: "Remove".into(),
                ..item(
                    &format!("stage_inbox_remove:{i}"),
                    PATH,
                    if i == 0 { "Inbox folders" } else { "" },
                    "",
                )
            });
        }
        items.push(PrefItem {
            action: "Add…".into(),
            ..item(
                "stage_inbox_add",
                BUTTON,
                if folders.inbox.is_empty() { "Inbox folders" } else { "Add an inbox folder" },
                "Downloads, a Soulseek folder, where purchases land: anything new there shows up in the Inbox.",
            )
        });
        items.push(PrefItem {
            value: show(folders.triage.clone(), "None").into(),
            action: "Choose…".into(),
            ..item(
                settings::STAGE_TRIAGE,
                PATH,
                "Triage folder",
                "Where tracks you are preparing are kept.",
            )
        });
        items.push(PrefItem {
            value: show(folders.collection.clone(), "None").into(),
            action: "Choose…".into(),
            ..item(
                settings::STAGE_COLLECTION,
                PATH,
                "Collection folder",
                "Where prepared tracks are kept. Its subfolders (such as Albums) stay as they are.",
            )
        });
        items.push(switch(
            settings::MOVE_FILES,
            "Move files",
            "Moves files into these folders as tracks change stage. Leave it off while another \
             app such as VirtualDJ still uses them: moves wait, and happen when you turn it on.",
            settings::move_files(),
        ));
        items.push(switch(
            settings::RENAME_TO_TRIAGE,
            "Rename files sent to Triage",
            "Names them \"Artist - Title (Remix)\" from their tags.",
            settings::rename_to_triage(),
        ));
        items.push(item("", HEADING, "CLEAN-UP", ""));
        items.push(item(
            "",
            NOTE,
            "",
            "What Library Health's \"Junk in names\" looks for. Clean up shows every change before \
             it is made.",
        ));
        for (key, name, about, default) in opendj_library::cleanup::Rules::ALL {
            items.push(switch(key, name, about, settings::flag(key, default)));
        }
        items.push(item("sets", HEADING, "SETS", ""));
        items.push(PrefItem {
            value: settings::set_template().join(", ").into(),
            on: true,
            ..item(
                "set_template",
                TEXT,
                "New set template",
                "The lists + › New set… makes, separated by commas. History's Add to Set \
                 List puts tracks in the one called Live set. Openers, Closers or \
                 Transitions can be added too.",
            )
        });
        items
    }

    /// The tabs a page has; most have none.
    fn tabs(&self, page: i32) -> Vec<String> {
        if page != LIBRARY {
            return Vec::new();
        }
        LIBRARY_TABS.iter().map(|t| t.to_string()).collect()
    }

    /// Settings › Library: General, Stages, Fields and Import. `tab` None
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
        items
    }

    /// Settings › Library › Import: each DJ app openDJ can bring a library
    /// in from. One not set up offers Set up…; once it's on, its own
    /// settings open up under its name. Apps still to come say so.
    fn import_items(&self) -> Vec<PrefItem> {
        let mut items = vec![item(
            "",
            NOTE,
            "",
            "Bring in another DJ app's library: its tracks, cues, grids and lists. openDJ \
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
                        &format!(
                            "Brings in {title}'s tracks, cues, grids and lists, and keeps them \
                             up to date."
                        ),
                    )
                });
            }
        }
        items.push(item("", HEADING, "COMING SOON", ""));
        for (title, detail) in COMING_IMPORTS {
            items.push(PrefItem {
                value: "Coming soon".into(),
                ..item("", BUTTON, &format!("Import from {title}"), detail)
            });
        }
        items
    }

    /// Settings › Library › General: the music folders, the cache and
    /// cover art.
    fn library_general(&self) -> Vec<PrefItem> {
        let (count, bytes) = cache::analysis_usage();
        let cache_detail = if self.cache_note.is_empty() {
            format!(
                "{count} {} · {}. Tracks are analysed again when loaded; stems are kept.",
                if count == 1 { "track" } else { "tracks" },
                size(bytes),
            )
        } else {
            self.cache_note.clone()
        };
        let mut items = self.music_folder_items();
        items.extend([
            PrefItem {
                value: show(settings::albums_folder(), "None").into(),
                action: "Choose…".into(),
                ..item(
                    settings::ALBUMS_FOLDER,
                    PATH,
                    "Albums folder",
                    "A folder per album, or Artist › Album. In the collection and under Albums.",
                )
            },
            PrefItem {
                value: show(settings::mixed_sets_folder(), "None").into(),
                action: "Choose…".into(),
                ..item(
                    settings::MIXED_SETS_FOLDER,
                    PATH,
                    "Mixed Sets folder",
                    "Whole DJ sets, kept out of the collection. Your recordings go in it too.",
                )
            },
            PrefItem {
                action: "Rescan".into(),
                ..item(
                    "rescan",
                    BUTTON,
                    "Update library",
                    if self.library_note.is_empty() {
                        "Looks for new and changed tracks in the music folders now."
                    } else {
                        &self.library_note
                    },
                )
            },
            switch(
                settings::SCAN_ON_START,
                "Update library on start",
                "Looks for new and changed tracks each time openDJ starts. Turn it off for a \
                 big collection and use Update library when you add music.",
                settings::scan_on_start(),
            ),
            PrefItem {
                action: "Clear".into(),
                ..item("clear_cache", BUTTON, "Analysis cache", &cache_detail)
            },
            item("cover_art", HEADING, "ONLINE LOOKUP", ""),
            PrefItem {
                value: settings::discogs_token().into(),
                on: true,
                ..item(
                    "discogs_token",
                    TEXT,
                    "Discogs token",
                    "Lets cover search and Look up online use Discogs too (label, year, \
                     style). Make one on discogs.com under Settings › Developers.",
                )
            },
            PrefItem {
                value: settings::acoustid_key().into(),
                on: true,
                ..item(
                    "acoustid_key",
                    TEXT,
                    "AcoustID key",
                    "Lets Look up online recognise tracks by their sound, so even badly \
                     named files are found. Get a free key on acoustid.org under My \
                     applications. Without one, tracks are looked up by artist and title.",
                )
            },
        ]);
        items
    }

    /// Settings › Library › Fields: names for grouping and the tag fields.
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
        let mut rows = vec![PrefItem {
            value: show(settings::import_database(importer), "Not found").into(),
            action: "Choose…".into(),
            ..item(
                &settings::import_database_key(id),
                PATH,
                &format!("{title} library"),
                &format!("Read when openDJ starts. Change its lists in {title}."),
            )
        }];
        if !importer.list_groups().is_empty() {
            rows.push(PrefItem {
                action: "Choose…".into(),
                ..item(
                    &format!("import_lists:{id}"),
                    BUTTON,
                    "Lists",
                    &format!("Which of {title}'s playlists, folders and history come in."),
                )
            });
        }
        rows.push(theirs_row(
            &settings::import_theirs_key(id),
            title,
            settings::import_theirs_win(id),
        ));
        rows.push(PrefItem {
            action: "Stop".into(),
            ..item(
                &format!("import_stop:{id}"),
                BUTTON,
                "Stop importing",
                &format!("Keeps what came in from {title}, but stops updating it."),
            )
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
                    format!("Takes out everything that came in from {title}.")
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
            "Bringing in {}… tracks appear in the library when it is done.",
            i.title()
        );
        self.import_note = self.library_note.clone();
    }

    fn toggled(&mut self, id: &str, on: bool) {
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
            "protect_playing" => self.set("[Controls]", "AllowTrackLoadToPlayingDeck", f(!on)),
            "xfader_reverse" => self.set("[Mixer Profile]", "xFaderReverse", f(on)),
            "xfader_enabled" => self.set("[Mixer]", "crossfader_enabled", f(on)),
            "head_split" => self.set("[Master]", "headSplit", f(on)),
            "auto_gain" => self.set("[ReplayGain]", "ReplayGainEnabled", f(on)),
            "auto_reset_fx" | "auto_reset_eq" | "auto_reset_gain" => {
                self.set("[Controls]", id, f(on))
            }
            _ => {}
        }
        self.refresh();
    }

    fn picked(&mut self, id: &str, index: usize) {
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
        if id == "controller_add" {
            if let (Some(c), Some(port)) = (&self.controllers, self.hidden_ports.get(index)) {
                settings::set_controller_shown(&device_key(port), true);
                if c.choice(port) == MappingChoice::Off {
                    c.set_choice(port, MappingChoice::Auto);
                    settings::set_controller_mappings(&c.choices());
                }
            }
            self.refresh();
            return;
        }
        if let Some(port) = id.strip_prefix("controller:") {
            if let Some(c) = &self.controllers {
                let choice = match index {
                    0 => MappingChoice::Auto,
                    1 => MappingChoice::Off,
                    i => match c.mappings().get(i - 2) {
                        Some((_, file)) => MappingChoice::File(file.clone()),
                        None => return,
                    },
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
            "key_match_on_load" => {
                settings::set("key_match_on_load", json!(index.min(2)));
                self.set("[Controls]", "key_match_on_load", index.min(2) as f64);
            }
            "key_notation" => {
                if let Some(app) = self.app.upgrade() {
                    app.global::<KeyNotation>().set_camelot(index == 0);
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
            "microphone_input" | "auxiliary_input" => {
                let role = if id == "microphone_input" {
                    InputRole::Microphone
                } else {
                    InputRole::Auxiliary
                };
                let Some(choice) = self.input_choices[role.index()].get(index).cloned() else {
                    return;
                };
                let mut setup = settings::audio_setup();
                setup.inputs.retain(|r| r.role != role);
                setup.inputs.extend(choice);
                settings::set_audio_setup(&setup);
                (self.inputs.changed)();
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
        if id == "set_template" {
            let lists: Vec<String> = text.split(',').map(|l| l.trim().to_string()).collect();
            settings::set_set_template(&lists);
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
        settings::set_plugin_value(plugin_id, key, text);
        plugin.configure(&settings::plugin_config(plugin_id));
        (self.plugin_changed)(plugin_id);
    }

    fn pressed(&mut self, id: &str) {
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
            self.library_note = "Its tracks leave the library on the next update, except ones \
                                 you rated, played, staged or put in a list: Library Health shows \
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
                "remove" => {
                    // Taken off the list and not used, until added again.
                    settings::set_controller_shown(&device_key(port), false);
                    if let Some(c) = &self.controllers {
                        c.set_choice(port, MappingChoice::Off);
                        settings::set_controller_mappings(&c.choices());
                    }
                    self.refresh();
                }
                _ => {}
            }
            return;
        }
        match id {
            "controllers_refresh" => {
                if let Some(c) = &self.controllers {
                    c.refresh();
                }
            }
            "stage_inbox_add" => choose(
                id,
                "Choose a folder to watch for new music",
                true,
                self.chosen_tx.clone(),
            ),
            settings::STAGE_TRIAGE => {
                choose(id, "Choose the Triage folder", true, self.chosen_tx.clone())
            }
            settings::STAGE_COLLECTION => choose(
                id,
                "Choose the Collection folder",
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
                    "Updating… new tracks appear in the library as they are found.".into();
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
            "clear_cache" => {
                let n = cache::clear_analysis();
                self.cache_note = format!(
                    "Cleared {n} {}. Tracks are analysed again when loaded.",
                    if n == 1 { "analysis" } else { "analyses" }
                );
            }
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

/// The mapping file a controller's Edit opens: the one it uses, or, for a
/// stand-in device, the one named as running.
fn edit_file(c: &Controllers, port: &Port) -> Option<String> {
    port.mapping_file.clone().or_else(|| {
        let running = port.running.as_ref()?;
        c.mappings()
            .into_iter()
            .find(|(name, _)| name == running)
            .map(|(_, file)| file)
    })
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
            "Whose titles, ratings and colours a track keeps.",
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
                     what changed; you can change these choices in Settings › Library."
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
