//! The settings window: builds the rows of each page from what is set now,
//! applies changes to the engine's controls at once and keeps them in
//! settings.json. Saved choices are applied again at start.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use mixxx_control::ControlRegistry;
use mixxx_controllers::{Controllers, MappingChoice, Port};
use mixxx_engine::{
    analysis, cache, deck_group, key, RecordFormat, Recorder, BUFFER_SIZES,
    DEFAULT_AUTO_GAIN_TARGET, NUM_DECKS, SAMPLE_RATES,
};
use mixxx_plugins::{Plugin, SettingKind};
use serde_json::json;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::{
    settings, short_port_name, AppWindow, AudioSettings, KeyNotation, PrefItem, Preferences,
};

const AUDIO: i32 = 0;
const CONTROLLERS: i32 = 1;
const LIBRARY: i32 = 2;
const ANALYSIS: i32 = 3;
const DECKS: i32 = 4;
const MIXER: i32 = 5;
const FEEDBACK: i32 = 6;

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
        "[Master]",
        "headSplit",
        f(settings::flag("head_split", false)),
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
    cache_note: String,
    /// The devices shown on the controllers page.
    ports: Vec<Port>,
    /// Plugins with a page of their own, after the built-in pages.
    plugins: Vec<Box<dyn Plugin>>,
    /// Tells the rest of the app a plugin's settings changed, by its id.
    plugin_changed: Box<dyn Fn(&str)>,
    /// Opens the outputs again with the saved audio settings.
    audio_changed: Box<dyn Fn()>,
}

impl Prefs {
    pub fn install(
        app: &AppWindow,
        registry: Arc<ControlRegistry>,
        controllers: Option<Rc<Controllers>>,
        recorder: Recorder,
        rescan: Box<dyn Fn()>,
        plugin_changed: Box<dyn Fn(&str)>,
        audio_changed: Box<dyn Fn()>,
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
            cache_note: String::new(),
            ports: Vec::new(),
            plugins,
            plugin_changed,
            audio_changed,
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
        prefs
    }

    /// Called every frame: picks up chosen paths and plugged-in devices.
    pub fn poll(&mut self) {
        while let Ok((id, path)) = self.chosen_rx.try_recv() {
            if id.starts_with("plugin:") {
                self.typed(&id, &path.to_string_lossy());
                self.refresh();
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

    /// A page was opened or the search changed.
    fn shown(&mut self) {
        if let Some(app) = self.app.upgrade() {
            app.global::<AudioSettings>().invoke_refresh();
        }
        self.cache_note.clear();
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
        let items: Vec<PrefItem> = if query.trim().is_empty() {
            self.page(&app, g.get_page())
        } else {
            let words: Vec<&str> = query.split_whitespace().collect();
            (AUDIO..FEEDBACK + 1 + self.plugins.len() as i32)
                .flat_map(|p| {
                    let name = slint::Model::row_data(&pages, p as usize).unwrap_or_default();
                    self.page(&app, p).into_iter().map(move |mut item| {
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
    }

    fn page(&mut self, app: &AppWindow, page: i32) -> Vec<PrefItem> {
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
                PrefItem {
                    value: settings::recording().dir.display().to_string().into(),
                    action: "Choose…".into(),
                    ..item(
                        "recording_folder",
                        PATH,
                        "Recordings folder",
                        "Each mix is saved with a .cue tracklist and a log of every control move.",
                    )
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
            LIBRARY => self.library_page(),
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
                    switch(
                        "keylock_default",
                        "Key lock on at start",
                        "Tempo changes keep the key; each deck's lock button still switches it.",
                        settings::flag("keylock_default", false),
                    ),
                    switch(
                        "quantize",
                        "Quantize",
                        "Hot cues, loops and beat jumps snap to the nearest beat.",
                        settings::flag("quantize", true),
                    ),
                    switch(
                        "protect_playing",
                        "Protect playing decks",
                        "A track can't be loaded onto a deck while it plays.",
                        settings::flag("protect_playing", true),
                    ),
                ]
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
            FEEDBACK => vec![
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
        let mut items: Vec<PrefItem> = self
            .ports
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
                    (Some(m), _, _) => format!("Running {m}"),
                    (None, MappingChoice::Off, _) => "Not used".to_string(),
                    (None, _, None) => {
                        "No Mixxx mapping matches this device: pick one.".to_string()
                    }
                    (None, _, Some(_)) => "Starting, or its mapping failed to load".to_string(),
                };
                PrefItem {
                    choices: strings(&choices),
                    chosen: chosen as i32,
                    ..item(
                        &format!("controller:{}", port.name),
                        MENU,
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
                "Plug in a MIDI controller and it shows up here within a couple of seconds.",
            ));
        }
        items.push(item(
            "",
            NOTE,
            "",
            "Controllers run Mixxx's own mappings. Automatic picks the one named after the device.",
        ));
        items
    }

    fn library_page(&mut self) -> Vec<PrefItem> {
        let show = |p: Option<PathBuf>, none: &str| {
            p.map_or(none.to_string(), |p| p.to_string_lossy().into_owned())
        };
        let env_note = |var: &str| {
            if std::env::var_os(var).is_some() {
                format!(" Set by {var} for now.")
            } else {
                String::new()
            }
        };
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
        let mut items = vec![
            PrefItem {
                value: show(settings::music_folder(), "None").into(),
                action: "Choose…".into(),
                ..item(
                    "music_folder",
                    PATH,
                    "Music folder",
                    &format!(
                        "Scanned for new tracks each time openDJ starts.{}",
                        env_note("OPENDJ_MUSIC")
                    ),
                )
            },
            PrefItem {
                action: "Rescan".into(),
                ..item(
                    "rescan",
                    BUTTON,
                    "Update library",
                    if self.library_note.is_empty() {
                        "Looks for new and changed tracks in the music folder now."
                    } else {
                        &self.library_note
                    },
                )
            },
            PrefItem {
                action: "Clear".into(),
                ..item("clear_cache", BUTTON, "Analysis cache", &cache_detail)
            },
        ];
        // What each importer add-on brings in besides the tracks.
        for importer in crate::stores::importers() {
            let groups = importer.list_groups();
            if groups.is_empty() {
                continue;
            }
            let chosen = settings::import_groups(importer.as_ref());
            let title = importer.title().to_string();
            items.push(item(
                "",
                HEADING,
                &format!("FROM {}", title.to_uppercase()),
                "",
            ));
            items.push(item(
                "",
                NOTE,
                "",
                &format!(
                    "Lists come into a {title} folder and are made again whenever they \
                     change in {title}, so make changes to them there."
                ),
            ));
            for g in groups {
                items.push(switch(
                    &settings::import_group_key(importer.id(), g.id),
                    g.title,
                    g.detail,
                    chosen.contains(&g.id),
                ));
            }
        }
        items
    }

    fn toggled(&mut self, id: &str, on: bool) {
        settings::set(id, json!(on));
        if id.starts_with("import:") {
            // Brings the lists in again with the new choice.
            (self.rescan)();
            self.library_note = "Updating the lists brought in…".into();
        }
        match id {
            "keylock_default" if on => self.each_deck("keylock", 1.0),
            "quantize" => self.each_deck("quantize", f(on)),
            "protect_playing" => self.set("[Controls]", "AllowTrackLoadToPlayingDeck", f(!on)),
            "xfader_reverse" => self.set("[Mixer Profile]", "xFaderReverse", f(on)),
            "head_split" => self.set("[Master]", "headSplit", f(on)),
            "auto_gain" => self.set("[ReplayGain]", "ReplayGainEnabled", f(on)),
            _ => {}
        }
        self.refresh();
    }

    fn picked(&mut self, id: &str, index: usize) {
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
        if id.starts_with("plugin:") {
            choose(id, "Choose a folder", true, self.chosen_tx.clone());
            return;
        }
        match id {
            "music_folder" => choose(id, "Choose your music folder", true, self.chosen_tx.clone()),
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

fn item(id: &str, kind: i32, title: &str, detail: &str) -> PrefItem {
    PrefItem {
        id: id.into(),
        kind,
        title: title.into(),
        detail: detail.into(),
        ..Default::default()
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

/// Opens the system's dialog for picking a folder or a file, on its own
/// thread; the path comes back through `tx`. macOS uses AppleScript's,
/// Linux zenity's.
fn choose(id: &str, prompt: &str, folder: bool, tx: Sender<(String, PathBuf)>) {
    let (id, prompt) = (id.to_string(), prompt.to_string());
    std::thread::spawn(move || {
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
                if !path.is_empty() {
                    let _ = tx.send((id, PathBuf::from(path)));
                }
            }
            Ok(_) => {} // Cancelled.
            Err(e) => eprintln!("can't open a file dialog: {e}"),
        }
    });
}
