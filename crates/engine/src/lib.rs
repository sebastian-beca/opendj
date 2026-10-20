//! The audio engine.
//!
//! It owns the same controls Mixxx's engine does (`[Channel1],play`,
//! `[Master],crossfader`, ...), plays the decks through a 3-band isolator EQ,
//! a filter and the effect units, mixes them with the crossfader and sends the
//! result to the sound card. The UI only talks to controls.
//!
//! Threads: the audio callback runs [`engine::Engine`]; loader threads put
//! tracks on decks over a wait-free queue as soon as their files are open,
//! then decode and analyse them while they play.

pub mod analysis;
pub mod beats;
pub mod cache;
mod controls;
pub mod cover;
pub mod decode;
pub mod dsp;
mod effects;
pub mod engine;
pub mod import;
pub mod inputs;
pub mod key;
pub mod keynet;
mod keyshift;
pub mod link;
pub mod loader;
pub mod loudness;
mod output;
pub mod preview;
pub mod recorder;
pub mod sampler;
pub mod scan;
pub mod stems;
pub mod synth;
pub mod tags;
pub mod track;
mod visual;

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, RwLock};

use mixxx_control::ControlRegistry;

pub use beats::{BeatGrid, GridMarker};
pub use controls::{
    deck_group, effect_group, effect_unit_group, eq_group, quick_effect_group, size_name,
    stem_group, BEATJUMP_SIZES, BEATLOOP_SIZES, BEAT_FX_UNIT, DEFAULT_AUTO_GAIN_TARGET,
    EFFECTS_PER_UNIT, MIC_FX_UNIT, NUM_DECKS, NUM_EFFECT_UNITS, NUM_HOTCUES,
};
pub use effects::{
    effect_name, parameter_names, AVAILABLE_EFFECTS, DEFAULT_EFFECTS, MAX_BUTTON_PARAMETERS,
    MAX_PARAMETERS,
};
pub use output::{
    channel_names, default_output_device, input_devices, output_devices, AudioSetup, InputRole,
    InputRoute, OutputDevice, Role, Route, BUFFER_SIZES, SAMPLE_RATES,
};
pub use recorder::{RecordFormat, RecordSettings, Recorder};
pub use synth::sample_library;
pub use track::{
    Audio, Hotcue, LibraryInfo, Marker, MarkerKind, Source, Track, Waveform, WaveformBin,
    MEMORY_CUE, WAVEFORM_RATE,
};
pub use visual::VisualPositions;

/// The tracks currently loaded on each deck, with their audio, readable by the
/// UI and the waveform renderers. While a track is being analysed its entry is
/// replaced with fuller versions (more waveform, then tempo and first beat).
pub type LoadedTracks = Arc<RwLock<[Option<Arc<Track>>; NUM_DECKS]>>;

/// Messages for the person at the decks, such as why a track didn't load,
/// oldest first. The UI shows and empties them.
pub type Alerts = Arc<Mutex<Vec<String>>>;

pub(crate) fn alert(alerts: &Alerts, message: String) {
    if let Ok(mut a) = alerts.lock() {
        a.push(message);
    }
}

/// Handle to the running engine. Dropping it stops playback.
pub struct AudioEngine {
    registry: Arc<ControlRegistry>,
    alerts: Alerts,
    requests: Sender<loader::LoadRequest>,
    loaded: LoadedTracks,
    visual: Arc<VisualPositions>,
    output: RefCell<output::Outputs>,
    recorder: Recorder,
    preview: Arc<Mutex<preview::PreviewInput>>,
    /// Counts preview requests, so a file still decoding for an earlier
    /// one stops.
    preview_generation: Arc<AtomicU64>,
}

/// Creates every engine control in `registry` without starting audio, for
/// tools that only need to know which controls exist.
pub fn create_controls(registry: &Arc<ControlRegistry>) {
    let rate = 44_100.0;
    let (mut engine, _input) = engine::Engine::new(registry, rate);
    let feed = engine.take_record_feed().expect("record feed");
    Recorder::new(registry, rate, feed, LoadedTracks::default());
}

impl AudioEngine {
    /// Creates all engine controls in `registry` and starts audio output to
    /// the devices `setup` names. The engine runs at the main device's rate.
    pub fn start(registry: Arc<ControlRegistry>, setup: &AudioSetup) -> Self {
        let rate = output::sample_rate(setup);
        let (mut engine, input) = engine::Engine::new(&registry, rate);
        let feed = engine.take_record_feed().expect("record feed");
        let preview = engine.take_preview_input().expect("preview input");
        let visual = engine.visual_positions();
        link::watch(engine.link_handle(), Arc::downgrade(&registry));
        let output = output::Outputs::new(engine, setup);

        let loaded: LoadedTracks = Arc::new(RwLock::new(Default::default()));
        let recorder = Recorder::new(&registry, rate, feed, loaded.clone());
        let (requests, rx) = mpsc::channel();
        let alerts = Alerts::default();
        {
            let (loaded, alerts) = (loaded.clone(), alerts.clone());
            std::thread::Builder::new()
                .name("track-loader".into())
                .spawn(move || loader::run(rx, input, loaded, alerts))
                .expect("spawn track loader");
        }
        Self {
            registry,
            alerts,
            requests,
            loaded,
            visual,
            output: RefCell::new(output),
            recorder,
            preview: Arc::new(Mutex::new(preview)),
            preview_generation: Arc::default(),
        }
    }

    /// Puts `track` in the preview player (`[PreviewDeck1]`, heard in the
    /// headphones only) at its start, playing if `play`, or empties it. The
    /// file is opened and decoded in the background; it plays while decoding.
    pub fn preview(&self, track: Option<&Track>, play: bool) {
        let generation = self.preview_generation.fetch_add(1, Ordering::AcqRel) + 1;
        let Some(track) = track else {
            if let Ok(mut p) = self.preview.lock() {
                p.send(None, false);
            }
            return;
        };
        let source = track.source.clone();
        let base = track.clone();
        let input = self.preview.clone();
        let current = self.preview_generation.clone();
        let spawned = std::thread::Builder::new()
            .name("preview".into())
            .spawn(move || {
                let latest = || current.load(Ordering::Acquire) == generation;
                let send = |audio: Arc<Audio>| {
                    if latest() {
                        if let Ok(mut p) = input.lock() {
                            p.send(Some(audio), play);
                        }
                    }
                };
                match &source {
                    Source::Synth { seed } => send(Arc::new(synth::render(&base, *seed))),
                    Source::File(path) => match decode::Decoder::open(path) {
                        Ok(d) if d.length_known() => {
                            send(d.audio().clone());
                            let _ = d.run(|_| latest());
                        }
                        Ok(d) => {
                            if let Ok(audio) = d.run(|_| latest()) {
                                send(audio);
                            }
                        }
                        Err(e) => eprintln!("can't preview {}: {e}", path.display()),
                    },
                }
            });
        if let Err(e) = spawned {
            eprintln!("could not start the preview: {e}");
        }
    }

    /// Puts `track` on `deck` as soon as its file is open; decoding and
    /// analysis carry on in the background (see [`loader`]).
    /// Unless `[Controls],AllowTrackLoadToPlayingDeck` is on, a deck that is
    /// playing keeps its track and this returns false.
    pub fn load_track(&self, deck: usize, track: Arc<Track>) -> bool {
        let on =
            |group: &str, key: &str| self.registry.get(group, key).is_some_and(|c| c.get_bool());
        if on(&deck_group(deck), "play") && !on("[Controls]", "AllowTrackLoadToPlayingDeck") {
            alert(
                &self.alerts,
                format!(
                    "Deck {} is playing, so the track wasn't loaded. Stop the deck first, \
                     or turn off Protect playing decks in Settings.",
                    deck + 1
                ),
            );
            return false;
        }
        self.recorder.track_loaded(deck, &track);
        let _ = self.requests.send(loader::LoadRequest { deck, track });
        true
    }

    /// Records the master mix; see [`recorder`].
    pub fn recorder(&self) -> &Recorder {
        &self.recorder
    }

    /// Alerts since the last call, oldest first.
    pub fn take_alerts(&self) -> Vec<String> {
        self.alerts
            .lock()
            .map(|mut a| std::mem::take(&mut *a))
            .unwrap_or_default()
    }

    /// Shows `cover` as the album art of every deck with the file at
    /// `path` loaded, after the file's cover was changed.
    pub fn set_cover(&self, path: &std::path::Path, cover: Option<Arc<cover::Cover>>) {
        let mut loaded = self.loaded.write().unwrap();
        for slot in loaded.iter_mut() {
            if let Some(t) = slot
                .as_ref()
                .filter(|t| matches!(&t.source, Source::File(p) if p == path))
            {
                *slot = Some(Arc::new(Track {
                    cover: cover.clone(),
                    ..(**t).clone()
                }));
            }
        }
    }

    pub fn loaded_tracks(&self) -> LoadedTracks {
        self.loaded.clone()
    }

    /// Play positions to draw from, smooth between audio buffers.
    pub fn visual_positions(&self) -> Arc<VisualPositions> {
        self.visual.clone()
    }

    /// Switches the outputs to the routes `setup` lists, and the inputs to
    /// its inputs.
    pub fn set_outputs(&self, setup: &AudioSetup) {
        self.output.borrow_mut().configure(setup);
    }

    /// Switches the microphone and auxiliary inputs to the devices `setup`
    /// lists, leaving the outputs playing.
    pub fn set_inputs(&self, setup: &AudioSetup) {
        self.output.borrow_mut().configure_inputs(setup);
    }

    /// Each input's state, by [`InputRole::index`]: its device and format,
    /// what is wrong with it, or empty when it has none.
    pub fn input_status(&self) -> Vec<String> {
        self.output.borrow().input_status.to_vec()
    }

    /// Separates `track` into stems with Demucs in the background (see
    /// [`stems::separate`]); when it is done, the stems play on any deck the
    /// track is loaded on. Check on it with [`stems::separation`].
    pub fn separate_stems(&self, track: &Track) {
        let Source::File(path) = &track.source else {
            return;
        };
        let loaded = self.loaded.clone();
        let file = path.clone();
        stems::separate(path, move |result| {
            let source = match result {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("stem separation failed: {e}");
                    return;
                }
            };
            let decks: Vec<_> = loaded.read().unwrap().iter().flatten().cloned().collect();
            for t in decks {
                let here = matches!(&t.source, Source::File(p) if *p == file);
                if let (true, Some(audio)) = (here, &t.audio) {
                    if let Err(e) = loader::load_stems(audio, &source, || true) {
                        eprintln!("could not load stems: {e}");
                    }
                }
            }
        });
    }

    /// The device running the engine, or why there is no sound.
    pub fn status(&self) -> String {
        self.output.borrow().main_status.clone()
    }

    /// Each route's state, in the setup's order.
    pub fn route_status(&self) -> Vec<String> {
        self.output.borrow().route_status.clone()
    }
}
