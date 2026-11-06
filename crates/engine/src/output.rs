//! Sends the engine's mixes to output devices with cpal, as a list of
//! routes: each route takes the master,
//! headphone (cue) or booth mix, or one deck or the sampler on its own, to a
//! pair of channels on a device. Without
//! a usable device (a headless machine, CI) the engine runs on a timer
//! instead, so everything except the sound itself keeps working.
//!
//! The master route's device runs the engine in its callback. Other devices
//! are fed through ring buffers; as their clocks drift from the master
//! device's, they drop or pad a little audio to keep the delay short.
//!
//! The microphone and auxiliary inputs come in the same way, from input
//! devices: each callback queues its frames for the engine (see
//! [`crate::inputs`]).
//!
//! [`Health`] keeps watch: how much of each buffer the engine takes, the
//! dropouts the device reports or that show as late callbacks, and a device
//! that goes away, which is reopened once it is back.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, ErrorKind, FromSample, SampleFormat, SizedSample, SupportedBufferSize};
use opendj_control::{ControlHandle, ControlRegistry};

use crate::engine::{Engine, MIX_CHANNELS, OUTPUT_CHANNELS};
use crate::inputs::{InputFeed, AUXILIARY, AUXILIARY_2, AUXILIARY_3, MICROPHONE, NUM_INPUTS};

/// What an output plays, each a stereo pair of the buffer the engine
/// renders: the mixes, or one deck or the sampler on its own (for a DAW or
/// an external mixer; what they carry is `[Master],deck_tap`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Master,
    Headphones,
    Booth,
    Deck1,
    Deck2,
    Deck3,
    Deck4,
    Sampler,
}

impl Role {
    pub const ALL: [Role; 8] = [
        Role::Master,
        Role::Headphones,
        Role::Booth,
        Role::Deck1,
        Role::Deck2,
        Role::Deck3,
        Role::Deck4,
        Role::Sampler,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Role::Master => "master",
            Role::Headphones => "headphones",
            Role::Booth => "booth",
            Role::Deck1 => "deck1",
            Role::Deck2 => "deck2",
            Role::Deck3 => "deck3",
            Role::Deck4 => "deck4",
            Role::Sampler => "sampler",
        }
    }

    pub fn from_name(name: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.name() == name)
    }

    /// One deck or the sampler on its own, not a mix.
    pub fn is_tap(self) -> bool {
        self.source() * 2 >= MIX_CHANNELS
    }

    /// Its pair in the engine's buffer.
    fn source(self) -> usize {
        self as usize
    }
}

/// Channels the engine renders for `setup`: the mixes, and the deck and
/// sampler pairs up to the last one routed. Without deck outputs it is the
/// six of the mixes, so the engine does no tap work at all.
fn render_width(setup: &AudioSetup) -> usize {
    setup
        .routes
        .iter()
        .map(|r| 2 * r.role.source() + 2)
        .fold(MIX_CHANNELS, usize::max)
}

/// One output: a mix to channels `2 * pair + 1` and `2 * pair + 2` of the
/// device called `device` (empty for the system default).
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub role: Role,
    pub device: String,
    pub pair: u16,
}

/// The engine's inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputRole {
    /// Mono: one channel of the device, heard in both sides.
    Microphone,
    /// Stereo by default (a pair of channels), or mono. Up to three.
    Auxiliary,
    Auxiliary2,
    Auxiliary3,
}

impl InputRole {
    pub const ALL: [InputRole; 4] = [
        InputRole::Microphone,
        InputRole::Auxiliary,
        InputRole::Auxiliary2,
        InputRole::Auxiliary3,
    ];

    pub fn name(self) -> &'static str {
        match self {
            InputRole::Microphone => "microphone",
            InputRole::Auxiliary => "auxiliary",
            InputRole::Auxiliary2 => "auxiliary2",
            InputRole::Auxiliary3 => "auxiliary3",
        }
    }

    pub fn from_name(name: &str) -> Option<InputRole> {
        InputRole::ALL.into_iter().find(|r| r.name() == name)
    }

    /// Its input in the engine (see [`crate::inputs`]).
    pub fn index(self) -> usize {
        match self {
            InputRole::Microphone => MICROPHONE,
            InputRole::Auxiliary => AUXILIARY,
            InputRole::Auxiliary2 => AUXILIARY_2,
            InputRole::Auxiliary3 => AUXILIARY_3,
        }
    }

    /// Whether it can take one channel or two (the auxiliary inputs).
    pub fn can_be_stereo(self) -> bool {
        self != InputRole::Microphone
    }
}

/// One input: from channel `channel + 1` of the device called `device`
/// (empty for the system default), and for a stereo input the channel after.
#[derive(Clone, Debug, PartialEq)]
pub struct InputRoute {
    pub role: InputRole,
    pub device: String,
    pub channel: u16,
    /// An auxiliary input taking one channel, heard in both sides. The
    /// microphone is always mono.
    pub mono: bool,
}

impl InputRoute {
    /// Channels it takes from the device.
    pub fn channels(&self) -> u16 {
        if self.mono || !self.role.can_be_stereo() {
            1
        } else {
            2
        }
    }
}

/// The routes in use, in the order the settings show them, and how the
/// devices run.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioSetup {
    pub routes: Vec<Route>,
    /// At most one for each input role; the first counts.
    pub inputs: Vec<InputRoute>,
    /// The rate to run at, if the clock device can; else the device's own.
    /// Read when the engine starts.
    pub sample_rate: Option<u32>,
    /// Frames per audio callback, within what each device allows; else the
    /// device's default. Smaller is less delay but more risk of dropouts.
    pub buffer_frames: Option<u32>,
}

impl Default for AudioSetup {
    /// The master mix on the system's default device.
    fn default() -> Self {
        Self {
            routes: vec![Route {
                role: Role::Master,
                device: String::new(),
                pair: 0,
            }],
            inputs: Vec::new(),
            sample_rate: None,
            buffer_frames: None,
        }
    }
}

/// Sample rates offered in the settings.
pub const SAMPLE_RATES: [u32; 4] = [44_100, 48_000, 88_200, 96_000];
/// Buffer sizes offered in the settings, in frames.
pub const BUFFER_SIZES: [u32; 6] = [64, 128, 256, 512, 1024, 2048];

/// How the audio keeps up, as Mixxx's `[App],audio_latency_*` controls (for
/// the settings and controller mappings), and whether an output device went
/// away.
pub(crate) struct Health {
    /// Percent of the buffer's time the slowest callback of the last half
    /// second took to render. Over 100 the device runs dry.
    usage: ControlHandle,
    /// The highest `usage` since the outputs opened.
    peak: ControlHandle,
    /// On for half a second after a dropout.
    overload: ControlHandle,
    /// Dropouts since the start: callbacks that took longer than their
    /// buffer, came late, or that the device reported (xruns).
    overloads: ControlHandle,
    /// An output stream reported its device gone or its stream invalid.
    lost: AtomicBool,
    /// The system refused the audio thread real-time priority.
    realtime_denied: AtomicBool,
}

/// Seconds `usage` takes the worst of, and that `overload` stays on.
const HEALTH_WINDOW: f64 = 0.5;
/// Seconds after the outputs open before callbacks count: devices start
/// unevenly.
const HEALTH_WARMUP: f64 = 1.0;
/// A callback this many buffers after the one before means audio was missed.
const LATE_CALLBACK: f64 = 3.0;
/// How often a lost device is looked for.
const RETRY: Duration = Duration::from_secs(2);

impl Health {
    pub(crate) fn new(reg: &ControlRegistry) -> Arc<Self> {
        Arc::new(Self {
            usage: reg.indicator("[App]", "audio_latency_usage", 1000.0),
            peak: reg.indicator("[App]", "audio_latency_peak", 1000.0),
            overload: reg.indicator("[App]", "audio_latency_overload", 1.0),
            overloads: reg.indicator("[App]", "audio_latency_overload_count", f64::MAX),
            lost: AtomicBool::new(false),
            realtime_denied: AtomicBool::new(false),
        })
    }

    fn count_overload(&self) {
        self.overloads.set(self.overloads.get() + 1.0);
        self.overload.set_bool(true);
    }

    /// An output stream's error callback.
    fn stream_error(&self, err: &cpal::Error) {
        match err.kind() {
            // Counted, not logged: this can come from the audio thread.
            ErrorKind::Xrun => self.count_overload(),
            ErrorKind::DeviceNotAvailable | ErrorKind::StreamInvalidated => {
                self.lost.store(true, Ordering::Release);
                eprintln!("audio output lost: {err}");
            }
            ErrorKind::RealtimeDenied => {
                self.realtime_denied.store(true, Ordering::Relaxed);
                eprintln!("audio output: {err}");
            }
            _ => eprintln!("audio output error: {err}"),
        }
    }
}

/// Times the clock device's callbacks for [`Health`]. Lives in the callback.
struct Meter {
    health: Arc<Health>,
    /// Seconds of audio rendered since the outputs opened.
    elapsed: f64,
    last: Option<(Instant, f64)>,
    worst: f64,
    window: f64,
    overload_left: f64,
}

impl Meter {
    fn new(health: Arc<Health>) -> Self {
        Self {
            health,
            elapsed: 0.0,
            last: None,
            worst: 0.0,
            window: HEALTH_WINDOW,
            overload_left: 0.0,
        }
    }

    /// After a callback that started at `start` and rendered `period`
    /// seconds of audio.
    fn measure(&mut self, start: Instant, period: f64) {
        let now = Instant::now();
        let used = now.duration_since(start).as_secs_f64() / period;
        let late = self.last.is_some_and(|(at, before)| {
            start.duration_since(at).as_secs_f64() > LATE_CALLBACK * before.max(period)
        });
        self.last = Some((start, period));
        self.elapsed += period;
        if self.elapsed < HEALTH_WARMUP {
            return;
        }
        if used > 1.0 || late {
            self.health.count_overload();
            self.overload_left = HEALTH_WINDOW;
        } else if self.overload_left > 0.0 {
            self.overload_left -= period;
            if self.overload_left <= 0.0 {
                self.health.overload.set_bool(false);
            }
        }
        self.worst = self.worst.max(used);
        self.window -= period;
        if self.window <= 0.0 {
            let percent = (self.worst * 100.0).round();
            self.health.usage.set(percent);
            if percent > self.health.peak.get() {
                self.health.peak.set(percent);
            }
            self.worst = 0.0;
            self.window = HEALTH_WINDOW;
        }
    }
}

/// An output device and how many channels it can play.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputDevice {
    pub name: String,
    pub channels: u16,
}

/// What ASIO drivers' names start with in the device lists, next to the
/// Windows Audio (WASAPI) devices.
#[cfg(windows)]
const ASIO_PREFIX: &str = "ASIO: ";

/// The audio systems devices are listed from, each with what its devices'
/// names start with: Windows Audio (WASAPI) and, on Windows, ASIO drivers
/// ("ASIO: Focusrite USB ASIO"); elsewhere only the system's own.
fn hosts() -> Vec<(&'static str, cpal::Host)> {
    #[allow(unused_mut)]
    let mut hosts = vec![("", cpal::default_host())];
    #[cfg(windows)]
    if let Ok(asio) = cpal::host_from_id(cpal::HostId::Asio) {
        hosts.push((ASIO_PREFIX, asio));
    }
    hosts
}

/// The audio system a listed device name belongs to, and the device's own
/// name there.
fn host_of(name: &str) -> (cpal::Host, &str) {
    for (prefix, host) in hosts() {
        if let Some(rest) = name.strip_prefix(prefix).filter(|_| !prefix.is_empty()) {
            return (host, rest);
        }
    }
    (cpal::default_host(), name)
}

/// The output devices, for choosing between them.
pub fn output_devices() -> Vec<OutputDevice> {
    if no_audio() {
        return Vec::new();
    }
    let mut list: Vec<OutputDevice> = hosts()
        .into_iter()
        .filter_map(|(prefix, host)| Some((prefix, host.output_devices().ok()?)))
        .flat_map(|(prefix, devices)| devices.map(move |d| (prefix, d)))
        .map(|(prefix, d)| OutputDevice {
            name: format!("{prefix}{}", device_name(&d)),
            channels: d
                .supported_output_configs()
                .map(|c| c.map(|r| r.channels()).max().unwrap_or(2))
                .unwrap_or(2),
        })
        .collect();
    list.dedup_by(|a, b| a.name == b.name);
    list
}

/// The input devices, for choosing between them.
pub fn input_devices() -> Vec<OutputDevice> {
    if no_audio() {
        return Vec::new();
    }
    let mut list: Vec<OutputDevice> = hosts()
        .into_iter()
        .filter_map(|(prefix, host)| Some((prefix, host.input_devices().ok()?)))
        .flat_map(|(prefix, devices)| devices.map(move |d| (prefix, d)))
        .map(|(prefix, d)| OutputDevice {
            name: format!("{prefix}{}", device_name(&d)),
            channels: d
                .supported_input_configs()
                .map(|c| c.map(|r| r.channels()).max().unwrap_or(1))
                .unwrap_or(1),
        })
        .collect();
    list.dedup_by(|a, b| a.name == b.name);
    list
}

/// The input device called `name`, or the default one for an empty name.
fn find_input(name: &str) -> Option<cpal::Device> {
    if name.is_empty() {
        return cpal::default_host().default_input_device();
    }
    let (host, name) = host_of(name);
    host.input_devices().ok()?.find(|d| device_name(d) == name)
}

/// The system's default output device, by name.
pub fn default_output_device() -> Option<String> {
    if no_audio() {
        return None;
    }
    find("").map(|d| device_name(&d))
}

fn no_audio() -> bool {
    std::env::var_os("OPENDJ_NO_AUDIO").is_some()
}

fn device_name(d: &cpal::Device) -> String {
    d.description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "audio device".into())
}

/// The device called `name`, or the default device for an empty name.
fn find(name: &str) -> Option<cpal::Device> {
    if name.is_empty() {
        return cpal::default_host().default_output_device();
    }
    let (host, name) = host_of(name);
    host.output_devices().ok()?.find(|d| device_name(d) == name)
}

/// The device whose callback runs the engine: the first master route's,
/// else the first route's, else the system default.
fn clock_device(setup: &AudioSetup) -> &str {
    setup
        .routes
        .iter()
        .find(|r| r.role == Role::Master)
        .or(setup.routes.first())
        .map_or("", |r| r.device.as_str())
}

/// Whether a deck or sampler output goes to another device than the
/// master's: each device keeps its own clock, so those outputs can drift
/// apart from the master and from each other by a few samples.
pub fn deck_outputs_split(setup: &AudioSetup) -> bool {
    let clock = clock_device(setup);
    setup
        .routes
        .iter()
        .any(|r| r.role.is_tap() && r.device != clock)
}

/// The sample rate the engine runs at: the one chosen if the clock device
/// can play it, else the clock device's own.
pub(crate) fn sample_rate(setup: &AudioSetup) -> f64 {
    if no_audio() {
        return 44_100.0;
    }
    let Some(device) = find(clock_device(setup)).or_else(|| find("")) else {
        return 44_100.0;
    };
    if let Some(rate) = setup.sample_rate {
        if config_at(&device, rate as f64, 1).is_some() {
            return rate as f64;
        }
    }
    device
        .default_output_config()
        .map_or(44_100.0, |c| c.sample_rate() as f64)
}

/// A stream config at `rate` with at least `channels` channels, preferring
/// float samples and the fewest channels.
fn config_at(
    device: &cpal::Device,
    rate: f64,
    channels: u16,
) -> Option<cpal::SupportedStreamConfig> {
    let mut ranges: Vec<_> = device
        .supported_output_configs()
        .ok()?
        .filter(|r| r.channels() >= channels)
        .collect();
    ranges.sort_by_key(|r| (r.sample_format() != SampleFormat::F32, r.channels()));
    ranges
        .into_iter()
        .find_map(|r| r.try_with_sample_rate(rate as u32))
}

/// The routes going to one device: (engine pair, device pair, route index).
struct DeviceRoutes {
    name: String,
    device: cpal::Device,
    routes: Vec<(usize, usize, usize)>,
}

/// Copies `pairs` of `mix` (one frame of the engine's buffer) into `out`.
fn write_pairs(out: &mut [f32], mix: &[f32], pairs: &[(usize, usize)]) {
    out.fill(0.0);
    for &(src, dst) in pairs {
        for c in 0..2 {
            if let Some(o) = out.get_mut(2 * dst + c) {
                *o = mix[2 * src + c];
            }
        }
    }
}

struct Timer {
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Timer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// The running outputs. Dropping it stops them.
pub(crate) struct Outputs {
    engine: Arc<Mutex<Engine>>,
    rate: f64,
    streams: Vec<cpal::Stream>,
    timer: Option<Timer>,
    /// The clock device and rate, or why there is no sound.
    pub(crate) main_status: String,
    /// Each route's state, in the setup's order: its format, or what is wrong.
    pub(crate) route_status: Vec<String>,
    inputs: Vec<cpal::Stream>,
    /// Each input's state, by [`InputRole::index`]: its device and format,
    /// what is wrong, or empty when it has no device.
    input_status: [String; NUM_INPUTS],
    /// Whether each open input has sent anything but silence yet. macOS
    /// sends silence to an app not allowed to use the microphone.
    heard: [Option<Arc<AtomicBool>>; NUM_INPUTS],
    health: Arc<Health>,
    /// The setup the outputs were last opened with, to reopen a lost device.
    setup: AudioSetup,
    /// When to look for a lost device next.
    retry_at: Option<Instant>,
    /// The person was told the device went away.
    lost_reported: bool,
}

impl Outputs {
    pub(crate) fn new(engine: Engine, setup: &AudioSetup, health: Arc<Health>) -> Self {
        let rate = engine.sample_rate();
        let mut o = Self {
            // The callbacks only ever try-lock, so they never wait.
            engine: Arc::new(Mutex::new(engine)),
            rate,
            streams: Vec::new(),
            timer: None,
            main_status: String::new(),
            route_status: Vec::new(),
            inputs: Vec::new(),
            input_status: Default::default(),
            heard: Default::default(),
            health,
            setup: setup.clone(),
            retry_at: None,
            lost_reported: false,
        };
        o.configure(setup);
        o
    }

    /// Opens the devices `setup` names, closing the ones in use.
    pub(crate) fn configure(&mut self, setup: &AudioSetup) {
        self.configure_outputs(setup);
        self.configure_inputs(setup);
    }

    /// Opens the input devices `setup` names, closing the ones in use, and
    /// leaves the outputs as they are.
    pub(crate) fn configure_inputs(&mut self, setup: &AudioSetup) {
        // Stop the old streams before taking their queues from the engine.
        self.inputs.clear();
        self.heard = Default::default();
        let mut old = Vec::new();
        for role in InputRole::ALL {
            let i = role.index();
            let route = setup.inputs.iter().find(|r| r.role == role);
            let feed = match route {
                None => {
                    self.input_status[i] = String::new();
                    None
                }
                Some(r) if no_audio() => {
                    self.input_status[i] = format!("{} unavailable", r.device);
                    None
                }
                Some(r) => match self.open_input(r) {
                    Ok((stream, feed, status, heard)) => {
                        self.inputs.push(stream);
                        self.heard[i] = Some(heard);
                        self.input_status[i] = status;
                        Some(feed)
                    }
                    Err(e) => {
                        self.input_status[i] = e;
                        None
                    }
                },
            };
            if let Ok(mut engine) = self.engine.lock() {
                old.push(engine.set_input(i, feed));
            }
        }
        drop(old);
    }

    /// Opens the input device `r` names.
    /// Each input's state, by [`InputRole::index`]: its device and format,
    /// what is wrong, or empty when it has no device.
    pub(crate) fn input_status(&self) -> Vec<String> {
        self.input_status
            .iter()
            .zip(&self.heard)
            .map(|(status, heard)| match heard {
                Some(h) if !h.load(Ordering::Relaxed) => format!(
                    "{status} · silent so far{}",
                    if cfg!(target_os = "macos") {
                        ": allow openDJ in System Settings › Privacy & Security › Microphone"
                    } else {
                        ""
                    }
                ),
                _ => status.clone(),
            })
            .collect()
    }

    /// Opens the input device `r` names. Also returns whether it has sent
    /// anything but silence yet.
    fn open_input(
        &self,
        r: &InputRoute,
    ) -> Result<(cpal::Stream, InputFeed, String, Arc<AtomicBool>), String> {
        let label = if r.device.is_empty() {
            "System default"
        } else {
            &r.device
        };
        let device = find_input(&r.device).ok_or_else(|| format!("{label} unavailable"))?;
        let name = device_name(&device);
        let first = r.channel as usize;
        let needed = r.channel + r.channels();
        let khz = self.rate / 1000.0;
        let config = input_config_at(&device, self.rate, needed).ok_or_else(|| {
            format!(
                "{name} can't record {} at {khz} kHz",
                channel_names(r.channel, r.channels())
            )
        })?;
        let channels = config.channels() as usize;
        // Half a second of room; the engine keeps it much emptier.
        let (mut tx, rx) = rtrb::RingBuffer::<f32>::new((self.rate * 0.5) as usize * 2);
        let period = Arc::new(AtomicUsize::new(0));
        let heard = Arc::new(AtomicBool::new(false));
        let stereo = r.channels() == 2;
        let stream = input_stream(&device, &config, {
            let (period, heard) = (period.clone(), heard.clone());
            move |data: &[f32]| {
                period.store(data.len() / channels, Ordering::Relaxed);
                if !heard.load(Ordering::Relaxed) && data.iter().any(|v| *v != 0.0) {
                    heard.store(true, Ordering::Relaxed);
                }
                for frame in data.chunks_exact(channels) {
                    if tx.slots() < 2 {
                        break;
                    }
                    let l = frame[first];
                    let r = if stereo { frame[first + 1] } else { l };
                    let _ = tx.push(l);
                    let _ = tx.push(r);
                }
            }
        })?;
        let status = format!(
            "{name} · {} · {khz} kHz",
            channel_names(r.channel, r.channels())
        );
        Ok((stream, InputFeed { rx, period }, status, heard))
    }

    /// Reopens the outputs after a device went away or its stream stopped,
    /// once the clock device is there again. Call it every so often; it says
    /// what happened, for the person at the decks.
    pub(crate) fn recover(&mut self) -> Option<String> {
        if !self.health.lost.load(Ordering::Acquire) {
            return None;
        }
        let now = Instant::now();
        if self.retry_at.is_some_and(|t| now < t) {
            return None;
        }
        self.retry_at = Some(now + RETRY);
        let name = clock_device(&self.setup).to_string();
        let label = if name.is_empty() {
            "The system's audio output".to_string()
        } else {
            name.clone()
        };
        if find(&name).is_none() {
            // Decks stop where they are, as the callback no longer runs.
            return (!std::mem::replace(&mut self.lost_reported, true)).then(|| {
                format!(
                    "{label} went away. openDJ plays on there once it is back, \
                     or pick another in Settings › Audio."
                )
            });
        }
        self.health.lost.store(false, Ordering::Release);
        let setup = self.setup.clone();
        self.configure_outputs(&setup);
        Some(if std::mem::take(&mut self.lost_reported) {
            format!("{label} is back.")
        } else {
            format!("{label} stopped; openDJ reopened it.")
        })
    }

    /// Whether the system refused the audio thread real-time priority.
    pub(crate) fn realtime_denied(&self) -> bool {
        self.health.realtime_denied.load(Ordering::Relaxed)
    }

    fn configure_outputs(&mut self, setup: &AudioSetup) {
        self.streams.clear();
        self.timer = None;
        self.setup = setup.clone();
        self.health.lost.store(false, Ordering::Release);
        self.lost_reported = false;
        self.retry_at = None;
        self.route_status = vec![String::new(); setup.routes.len()];
        if let Err(e) = self.open(setup) {
            eprintln!("audio output unavailable ({e}); running without sound");
            self.streams.clear();
            self.timer = Some(self.start_timer());
            self.main_status = if no_audio() {
                "No audio output".into()
            } else {
                e
            };
            for s in &mut self.route_status {
                if s.is_empty() || !s.contains("unavailable") {
                    *s = self.main_status.clone();
                }
            }
        }
    }

    fn open(&mut self, setup: &AudioSetup) -> Result<(), String> {
        if no_audio() {
            return Err("OPENDJ_NO_AUDIO is set".into());
        }
        let khz = self.rate / 1000.0;
        let buffer = setup.buffer_frames;
        let clock = find(clock_device(setup))
            .or_else(|| find(""))
            .ok_or("no output device")?;
        let clock_name = device_name(&clock);

        // Group the routes by device, the clock device first. A route to
        // channels another route already uses is left out.
        let mut devices = vec![DeviceRoutes {
            name: clock_name.clone(),
            device: clock,
            routes: Vec::new(),
        }];
        for (i, r) in setup.routes.iter().enumerate() {
            let name = if r.device.is_empty() {
                find("").map(|d| device_name(&d)).unwrap_or_default()
            } else {
                r.device.clone()
            };
            let at = match devices.iter().position(|d| d.name == name) {
                Some(at) => at,
                None => match find(&name) {
                    Some(device) => {
                        devices.push(DeviceRoutes {
                            name: name.clone(),
                            device,
                            routes: Vec::new(),
                        });
                        devices.len() - 1
                    }
                    None => {
                        self.route_status[i] = format!("{name} unavailable");
                        continue;
                    }
                },
            };
            let pair = r.pair as usize;
            if let Some(&(_, _, other)) = devices[at].routes.iter().find(|x| x.1 == pair) {
                self.route_status[i] =
                    format!("same channels as {}", setup.routes[other].role.name());
                continue;
            }
            devices[at].routes.push((r.role.source(), pair, i));
        }

        // Open the other devices first, so the clock knows where to feed.
        let mut feeds = Vec::new();
        for d in devices.drain(1..).collect::<Vec<_>>() {
            if d.routes.is_empty() {
                continue;
            }
            let needed = d
                .routes
                .iter()
                .map(|r| r.1 as u16 * 2 + 2)
                .max()
                .unwrap_or(2);
            let Some(config) = config_at(&d.device, self.rate, needed) else {
                for &(_, pair, i) in &d.routes {
                    self.route_status[i] = format!(
                        "{} unavailable at {khz} kHz with ch {} & {}",
                        d.name,
                        2 * pair + 1,
                        2 * pair + 2
                    );
                }
                continue;
            };
            let channels = config.channels() as usize;
            let width = d.routes.len() * 2;
            // A quarter second of audio in flight at most.
            let (tx, mut rx) = rtrb::RingBuffer::<f32>::new((self.rate * 0.5) as usize * width);
            let pairs: Vec<(usize, usize)> =
                d.routes.iter().enumerate().map(|(k, r)| (k, r.1)).collect();
            let mut frame = [0.0f32; OUTPUT_CHANNELS];
            match stream(
                &d.device,
                &config,
                buffer,
                &self.health,
                move |data: &mut [f32]| {
                    let frames = data.len() / channels;
                    // Keep at most two buffers queued: drop the oldest beyond that.
                    let excess = rx.slots().saturating_sub(frames * width * 2) / width * width;
                    for _ in 0..excess {
                        let _ = rx.pop();
                    }
                    for out in data.chunks_exact_mut(channels) {
                        if rx.slots() >= width {
                            for v in &mut frame[..width] {
                                *v = rx.pop().unwrap_or(0.0);
                            }
                            write_pairs(out, &frame, &pairs);
                        } else {
                            out.fill(0.0);
                        }
                    }
                },
            ) {
                Ok(s) => {
                    self.streams.push(s);
                    for &(_, _, i) in &d.routes {
                        self.route_status[i] = format!("{khz} kHz · {channels} out");
                    }
                    let sources: Vec<usize> = d.routes.iter().map(|r| r.0).collect();
                    feeds.push((tx, sources));
                }
                Err(e) => {
                    for &(_, _, i) in &d.routes {
                        self.route_status[i] = format!("{}: {e}", d.name);
                    }
                }
            }
        }

        // The clock device runs the engine and feeds the others.
        let d = devices.pop().expect("clock device");
        let needed = d
            .routes
            .iter()
            .map(|r| r.1 as u16 * 2 + 2)
            .max()
            .unwrap_or(2);
        let config = config_at(&d.device, self.rate, needed)
            .or_else(|| config_at(&d.device, self.rate, 1))
            .ok_or_else(|| format!("{clock_name} can't play at {khz} kHz"))?;
        let channels = config.channels() as usize;
        let pairs: Vec<(usize, usize)> = d.routes.iter().map(|r| (r.0, r.1)).collect();
        for &(_, pair, i) in &d.routes {
            self.route_status[i] = if 2 * pair + 2 <= channels {
                format!("{khz} kHz · {channels} out")
            } else {
                format!("{clock_name} has no ch {} & {}", 2 * pair + 1, 2 * pair + 2)
            };
        }
        let engine = self.engine.clone();
        let width = render_width(setup);
        let mut mix = vec![0.0f32; 16_384 * width];
        let mut feeds = feeds;
        let mut meter = Meter::new(self.health.clone());
        let rate = self.rate;
        self.streams.push(stream(
            &d.device,
            &config,
            buffer,
            &self.health,
            move |data: &mut [f32]| {
                let start = Instant::now();
                let frames = data.len() / channels;
                if mix.len() < frames * width {
                    mix.resize(frames * width, 0.0);
                }
                let mix = &mut mix[..frames * width];
                match engine.try_lock() {
                    Ok(mut e) => e.process(mix, width),
                    Err(_) => mix.fill(0.0),
                }
                for (out, m) in data.chunks_exact_mut(channels).zip(mix.chunks_exact(width)) {
                    write_pairs(out, m, &pairs);
                    for (tx, sources) in &mut feeds {
                        if tx.slots() >= sources.len() * 2 {
                            for &s in sources.iter() {
                                let _ = tx.push(m[2 * s]);
                                let _ = tx.push(m[2 * s + 1]);
                            }
                        }
                    }
                }
                if frames > 0 {
                    meter.measure(start, frames as f64 / rate);
                }
            },
        )?);
        self.main_status = match buffer_frames(&config, buffer) {
            BufferSize::Fixed(n) => format!(
                "{clock_name} · {khz} kHz · {n} samples ({:.1} ms)",
                n as f64 / self.rate * 1000.0
            ),
            BufferSize::Default => format!("{clock_name} · {khz} kHz"),
        };
        Ok(())
    }

    fn start_timer(&self) -> Timer {
        let running = Arc::new(AtomicBool::new(true));
        let engine = self.engine.clone();
        let rate = self.rate;
        let thread = {
            let running = running.clone();
            std::thread::Builder::new()
                .name("engine-timer".into())
                .spawn(move || {
                    const FRAMES: usize = 512;
                    let mut buf = vec![0.0f32; FRAMES * 2];
                    let period = Duration::from_secs_f64(FRAMES as f64 / rate);
                    let mut next = Instant::now();
                    while running.load(Ordering::Relaxed) {
                        if let Ok(mut e) = engine.lock() {
                            e.process(&mut buf, 2);
                        }
                        next += period;
                        if let Some(wait) = next.checked_duration_since(Instant::now()) {
                            std::thread::sleep(wait);
                        } else {
                            next = Instant::now();
                        }
                    }
                })
                .expect("spawn engine timer")
        };
        Timer {
            running,
            thread: Some(thread),
        }
    }
}

/// The buffer size to ask `config`'s device for: `frames` within its range.
fn buffer_frames(config: &cpal::SupportedStreamConfig, frames: Option<u32>) -> BufferSize {
    match (frames, config.buffer_size()) {
        (None, _) => BufferSize::Default,
        (Some(n), &SupportedBufferSize::Range { min, max }) => BufferSize::Fixed(n.clamp(min, max)),
        (Some(n), SupportedBufferSize::Unknown) => BufferSize::Fixed(n),
    }
}

/// "In 1", or "In 1 & 2" for a pair.
pub fn channel_names(first: u16, count: u16) -> String {
    if count >= 2 {
        format!("In {} & {}", first + 1, first + 2)
    } else {
        format!("In {}", first + 1)
    }
}

/// An input stream config at `rate` with at least `channels` channels,
/// preferring float samples and the fewest channels.
fn input_config_at(
    device: &cpal::Device,
    rate: f64,
    channels: u16,
) -> Option<cpal::SupportedStreamConfig> {
    let mut ranges: Vec<_> = device
        .supported_input_configs()
        .ok()?
        .filter(|r| r.channels() >= channels)
        .collect();
    ranges.sort_by_key(|r| (r.sample_format() != SampleFormat::F32, r.channels()));
    ranges
        .into_iter()
        .find_map(|r| r.try_with_sample_rate(rate as u32))
}

/// Opens and starts an input stream that hands `take` its audio in f32,
/// interleaved.
fn input_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    take: impl FnMut(&[f32]) + Send + 'static,
) -> Result<cpal::Stream, String> {
    let stream = match config.sample_format() {
        SampleFormat::F32 => build_input::<f32>(device, config, take),
        SampleFormat::I16 => build_input::<i16>(device, config, take),
        SampleFormat::I32 => build_input::<i32>(device, config, take),
        SampleFormat::U16 => build_input::<u16>(device, config, take),
        other => Err(format!("unsupported sample format {other:?}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

fn build_input<T>(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    mut take: impl FnMut(&[f32]) + Send + 'static,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels() as usize;
    let mut scratch = vec![0.0f32; 16_384 * channels];
    device
        .build_input_stream::<T, _, _>(
            config.config(),
            move |data: &[T], _| {
                if scratch.len() < data.len() {
                    // Rare: a device delivering more than asked for.
                    scratch.resize(data.len(), 0.0);
                }
                let buf = &mut scratch[..data.len()];
                for (s, d) in buf.iter_mut().zip(data) {
                    *s = <f32 as FromSample<T>>::from_sample_(*d);
                }
                take(buf);
            },
            |err| eprintln!("audio input error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())
}

/// Opens and starts an output stream that `fill` renders in f32, with
/// buffers of `frames` if the device takes them, else its default.
fn stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    frames: Option<u32>,
    health: &Arc<Health>,
    fill: impl FnMut(&mut [f32]) + Send + 'static,
) -> Result<cpal::Stream, String> {
    let fill = Arc::new(Mutex::new(fill));
    let open = |size: BufferSize| {
        let fill = fill.clone();
        // Only this stream's callback locks it, so it never waits.
        let fill = move |data: &mut [f32]| {
            if let Ok(mut f) = fill.try_lock() {
                f(data)
            }
        };
        let health = health.clone();
        let error = move |err: cpal::Error| health.stream_error(&err);
        match config.sample_format() {
            SampleFormat::F32 => build::<f32>(device, config, size, fill, error),
            SampleFormat::I16 => build::<i16>(device, config, size, fill, error),
            SampleFormat::I32 => build::<i32>(device, config, size, fill, error),
            SampleFormat::U16 => build::<u16>(device, config, size, fill, error),
            other => Err(format!("unsupported sample format {other:?}")),
        }
    };
    let size = buffer_frames(config, frames);
    let stream = match open(size) {
        Err(e) if size != BufferSize::Default => {
            eprintln!("audio output: {size:?} refused ({e}); using the device's default");
            open(BufferSize::Default)
        }
        other => other,
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    buffer_size: BufferSize,
    mut fill: impl FnMut(&mut [f32]) + Send + 'static,
    error: impl FnMut(cpal::Error) + Send + 'static,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    // Scratch buffer, sized generously up front so the callback does not allocate.
    let channels = config.channels() as usize;
    let mut scratch = vec![0.0f32; 16_384 * channels];
    device
        .build_output_stream::<T, _, _>(
            cpal::StreamConfig {
                buffer_size,
                ..config.config()
            },
            move |data: &mut [T], _| {
                if scratch.len() < data.len() {
                    scratch.resize(data.len(), 0.0);
                }
                let buf = &mut scratch[..data.len()];
                fill(buf);
                for (d, s) in data.iter_mut().zip(buf.iter()) {
                    *d = T::from_sample(*s);
                }
            },
            error,
            None,
        )
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A meter with `seconds` of callbacks behind it, each `period` long
    /// and taking `share` of it.
    fn metered(seconds: f64, period: f64, share: f64) -> (Arc<ControlRegistry>, Meter) {
        let reg = ControlRegistry::new();
        let mut meter = Meter::new(Health::new(&reg));
        for _ in 0..(seconds / period) as usize {
            meter.measure(
                Instant::now() - Duration::from_secs_f64(period * share),
                period,
            );
        }
        (reg, meter)
    }

    fn app(reg: &ControlRegistry, item: &str) -> f64 {
        reg.get("[App]", item).unwrap().get()
    }

    #[test]
    fn measures_the_share_of_each_buffer_used() {
        let (reg, _) = metered(2.0, 0.01, 0.2);
        let usage = app(&reg, "audio_latency_usage");
        assert!((19.0..30.0).contains(&usage), "{usage}");
        assert!(app(&reg, "audio_latency_peak") >= usage);
        assert_eq!(app(&reg, "audio_latency_overload_count"), 0.0);
    }

    #[test]
    fn counts_slow_and_late_callbacks_as_dropouts() {
        let (reg, mut meter) = metered(2.0, 0.01, 0.2);
        // Took longer than its buffer.
        meter.measure(Instant::now() - Duration::from_millis(15), 0.01);
        assert_eq!(app(&reg, "audio_latency_overload_count"), 1.0);
        assert_eq!(app(&reg, "audio_latency_overload"), 1.0);
        // Came four buffers after the one before.
        meter.measure(Instant::now() + Duration::from_millis(40), 0.01);
        assert_eq!(app(&reg, "audio_latency_overload_count"), 2.0);
        // The light goes out half a second later.
        for _ in 0..60 {
            meter.measure(Instant::now(), 0.01);
        }
        assert_eq!(app(&reg, "audio_latency_overload"), 0.0);
        assert_eq!(app(&reg, "audio_latency_overload_count"), 2.0);
    }

    #[test]
    fn ignores_the_first_second() {
        let (reg, mut meter) = metered(0.5, 0.01, 0.2);
        meter.measure(Instant::now() - Duration::from_millis(15), 0.01);
        assert_eq!(app(&reg, "audio_latency_overload_count"), 0.0);
        assert_eq!(app(&reg, "audio_latency_usage"), 0.0);
    }

    fn route(role: Role, device: &str, pair: u16) -> Route {
        Route {
            role,
            device: device.into(),
            pair,
        }
    }

    #[test]
    fn renders_deck_pairs_only_up_to_the_last_one_routed() {
        let mut setup = AudioSetup::default();
        setup.routes.push(route(Role::Headphones, "", 1));
        assert_eq!(render_width(&setup), MIX_CHANNELS);
        setup.routes.push(route(Role::Deck2, "", 2));
        assert_eq!(render_width(&setup), 10);
        setup.routes.push(route(Role::Sampler, "", 3));
        assert_eq!(render_width(&setup), OUTPUT_CHANNELS);
        assert!(Role::Deck1.is_tap() && Role::Sampler.is_tap());
        assert!(!Role::Booth.is_tap());
        assert!(Role::ALL
            .iter()
            .all(|r| Role::from_name(r.name()) == Some(*r)));
    }

    #[test]
    fn warns_when_deck_outputs_leave_the_master_device() {
        let mut setup = AudioSetup {
            routes: vec![route(Role::Master, "BlackHole 16ch", 0)],
            ..AudioSetup::default()
        };
        setup.routes.push(route(Role::Deck1, "BlackHole 16ch", 1));
        setup.routes.push(route(Role::Headphones, "DDJ-FLX4", 1));
        assert!(!deck_outputs_split(&setup));
        setup.routes.push(route(Role::Deck2, "DDJ-FLX4", 0));
        assert!(deck_outputs_split(&setup));
    }

    #[test]
    fn device_errors_count_dropouts_and_losses() {
        let reg = ControlRegistry::new();
        let health = Health::new(&reg);
        health.stream_error(&ErrorKind::Xrun.into());
        assert_eq!(app(&reg, "audio_latency_overload_count"), 1.0);
        assert!(!health.lost.load(Ordering::Acquire));
        health.stream_error(&ErrorKind::DeviceNotAvailable.into());
        assert!(health.lost.load(Ordering::Acquire));
        health.stream_error(&ErrorKind::RealtimeDenied.into());
        assert!(health.realtime_denied.load(Ordering::Relaxed));
    }
}
