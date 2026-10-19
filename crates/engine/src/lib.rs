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
pub mod key;
mod keyshift;
pub mod link;
pub mod loader;
pub mod loudness;
mod output;
pub mod recorder;
pub mod sampler;
pub mod scan;
pub mod stems;
pub mod synth;
pub mod track;
mod visual;

use std::cell::RefCell;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, RwLock};

use mixxx_control::ControlRegistry;

pub use beats::{BeatGrid, GridMarker};
pub use controls::{
    deck_group, effect_group, effect_unit_group, eq_group, quick_effect_group, size_name,
    stem_group, BEATJUMP_SIZES, BEATLOOP_SIZES, DEFAULT_AUTO_GAIN_TARGET, EFFECTS_PER_UNIT,
    NUM_DECKS, NUM_EFFECT_UNITS, NUM_HOTCUES,
};
pub use effects::{
    effect_name, parameter_names, AVAILABLE_EFFECTS, DEFAULT_EFFECTS, MAX_BUTTON_PARAMETERS,
    MAX_PARAMETERS,
};
pub use output::{default_output_device, output_devices, AudioSetup, OutputDevice, Role, Route};
pub use recorder::{RecordFormat, RecordSettings, Recorder};
pub use synth::sample_library;
pub use track::{Audio, Hotcue, LibraryInfo, Source, Track, Waveform, WaveformBin, WAVEFORM_RATE};
pub use visual::VisualPositions;

/// The tracks currently loaded on each deck, with their audio, readable by the
/// UI and the waveform renderers. While a track is being analysed its entry is
/// replaced with fuller versions (more waveform, then tempo and first beat).
pub type LoadedTracks = Arc<RwLock<[Option<Arc<Track>>; NUM_DECKS]>>;

/// Handle to the running engine. Dropping it stops playback.
pub struct AudioEngine {
    registry: Arc<ControlRegistry>,
    requests: Sender<loader::LoadRequest>,
    loaded: LoadedTracks,
    visual: Arc<VisualPositions>,
    output: RefCell<output::Outputs>,
    recorder: Recorder,
}

impl AudioEngine {
    /// Creates all engine controls in `registry` and starts audio output to
    /// the devices `setup` names. The engine runs at the main device's rate.
    pub fn start(registry: Arc<ControlRegistry>, setup: &AudioSetup) -> Self {
        let rate = output::sample_rate(setup);
        let (mut engine, input) = engine::Engine::new(&registry, rate);
        let feed = engine.take_record_feed().expect("record feed");
        let visual = engine.visual_positions();
        link::watch(engine.link_handle(), Arc::downgrade(&registry));
        let output = output::Outputs::new(engine, setup);

        let loaded: LoadedTracks = Arc::new(RwLock::new(Default::default()));
        let recorder = Recorder::new(&registry, rate, feed, loaded.clone());
        let (requests, rx) = mpsc::channel();
        {
            let loaded = loaded.clone();
            std::thread::Builder::new()
                .name("track-loader".into())
                .spawn(move || loader::run(rx, input, loaded))
                .expect("spawn track loader");
        }
        Self {
            registry,
            requests,
            loaded,
            visual,
            output: RefCell::new(output),
            recorder,
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

    pub fn loaded_tracks(&self) -> LoadedTracks {
        self.loaded.clone()
    }

    /// Play positions to draw from, smooth between audio buffers.
    pub fn visual_positions(&self) -> Arc<VisualPositions> {
        self.visual.clone()
    }

    /// Switches the outputs to the routes `setup` lists.
    pub fn set_outputs(&self, setup: &AudioSetup) {
        self.output.borrow_mut().configure(setup);
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
