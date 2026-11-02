//! What a helper runs. Real plugin formats (CLAP, then VST3 and AU) will
//! implement [`Effect`] too; for now there are test effects that misbehave
//! on purpose, so the isolation can be tested without third-party plugins.

use crate::client::Transport;
use crate::shm::CHANNELS;

/// One loaded effect, inside the helper, on its audio thread.
pub trait Effect: Send {
    fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        params: &[f32],
        transport: &Transport,
    );
}

/// The part of a plugin that must stay on the helper's main thread
/// (CLAP's instance, later editors). The helper calls `idle` regularly.
pub trait MainThread {
    fn idle(&mut self);
}

/// One parameter, in the plugin's own (plain) units.
#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    /// Its value when the plugin loaded.
    pub value: f64,
}

/// What openDJ learns about a loaded effect.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Info {
    pub name: String,
    /// Up to [`crate::shm::MAX_PARAMS`]; index `i` is shared parameter `i`.
    pub params: Vec<Param>,
    /// Frames the plugin delays its output by.
    pub latency: u32,
}

/// A loaded effect: the audio half, the main-thread half, and its info.
pub struct Loaded {
    pub effect: Box<dyn Effect>,
    pub main: Option<Box<dyn MainThread>>,
    pub info: Info,
}

/// Which effect a helper runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plugin {
    /// One of [`TEST_EFFECTS`].
    Test(String),
    /// Plugin `id` in the `.clap` file `file`.
    Clap {
        file: std::path::PathBuf,
        id: String,
    },
    /// Class `id` (32 hex digits) in the `.vst3` bundle `file`.
    Vst3 {
        file: std::path::PathBuf,
        id: String,
    },
    /// The Audio Unit `type:subtype:manufacturer` (macOS only).
    Au { id: String },
}

/// Ids of the test effects.
pub const TEST_EFFECTS: [&str; 4] = ["test-gain", "test-crash", "test-hang", "test-slow"];

/// Loads `plugin`, or says why not.
pub fn load(plugin: &Plugin, sample_rate: u32, max_frames: usize) -> Result<Loaded, String> {
    // Each test effect has one parameter, the one it acts on.
    let test = |effect: Box<dyn Effect>, name: &str, param: &str, max: f64, value: f64| Loaded {
        effect,
        main: None,
        info: Info {
            name: name.into(),
            params: vec![Param {
                name: param.into(),
                min: 0.0,
                max,
                default: value,
                value,
            }],
            ..Info::default()
        },
    };
    match plugin {
        Plugin::Test(id) => match id.as_str() {
            "test-gain" => Ok(test(Box::new(Gain), id, "Gain", 2.0, 1.0)),
            "test-crash" => Ok(test(
                Box::new(Crash { blocks: 0 }),
                id,
                "Crash after",
                1000.0,
                0.0,
            )),
            "test-hang" => Ok(test(Box::new(Hang), id, "Hang", 1.0, 0.0)),
            "test-slow" => Ok(test(Box::new(Slow), id, "Milliseconds", 100.0, 0.0)),
            _ => Err(format!("unknown effect {id:?}")),
        },
        #[cfg(feature = "formats")]
        Plugin::Clap { file, id } => crate::clap::load(file, id, sample_rate, max_frames),
        #[cfg(feature = "formats")]
        Plugin::Vst3 { file, id } => crate::vst3::load(file, id, sample_rate, max_frames),
        #[cfg(all(feature = "formats", target_os = "macos"))]
        Plugin::Au { id } => crate::au::load(id, sample_rate, max_frames),
        #[cfg(all(feature = "formats", not(target_os = "macos")))]
        Plugin::Au { .. } => Err("Audio Units only run on macOS".into()),
        #[cfg(not(feature = "formats"))]
        _ => {
            let _ = (sample_rate, max_frames);
            Err("this build hosts no plugin formats".into())
        }
    }
}

/// Multiplies by parameter 0.
struct Gain;

impl Effect for Gain {
    fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        params: &[f32],
        _: &Transport,
    ) {
        let g = params[0];
        for (i, o) in input.iter().zip(output) {
            for (x, y) in i.iter().zip(o.iter_mut()) {
                *y = x * g;
            }
        }
    }
}

/// Passes audio, then aborts once it has done parameter 0's number of
/// blocks (when that is above 0), like a plugin with a memory bug.
struct Crash {
    blocks: u32,
}

impl Effect for Crash {
    fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        params: &[f32],
        t: &Transport,
    ) {
        if params[0] > 0.0 && self.blocks as f32 >= params[0] {
            std::process::abort();
        }
        self.blocks += 1;
        Gain.process(input, output, &[1.0], t);
    }
}

/// Passes audio until parameter 0 goes above 0.5, then never returns.
struct Hang;

impl Effect for Hang {
    fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        params: &[f32],
        t: &Transport,
    ) {
        if params[0] > 0.5 {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        Gain.process(input, output, &[1.0], t);
    }
}

/// Passes audio after sleeping parameter 0 milliseconds per block.
struct Slow;

impl Effect for Slow {
    fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        params: &[f32],
        t: &Transport,
    ) {
        if params[0] > 0.0 {
            std::thread::sleep(std::time::Duration::from_secs_f32(params[0] / 1000.0));
        }
        Gain.process(input, output, &[1.0], t);
    }
}
