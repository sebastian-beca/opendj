//! Finds MIDI controllers, picks the Mixxx mapping for each by name, and runs
//! each one on its own thread. Plugging a controller in or out is picked up
//! within about a second, and [`Controllers::refresh`] reconnects them all.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use midir::{Ignore, MidiInput, MidiOutput};
use opendj_control::ControlRegistry;

use crate::controller::Controller;
use crate::format::OpenDjMapping;
use crate::mapping::Mapping;

const CLIENT: &str = "openDJ";
const RESCAN: Duration = Duration::from_secs(1);
/// LEDs and connection callbacks are refreshed at least this often.
const POLL: Duration = Duration::from_millis(10);

/// A controller that is connected and running its mapping.
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectedController {
    pub port: String,
    pub mapping: String,
    /// The Mixxx mapping's file name: the one it runs, or the one the own
    /// mapping it runs is based on.
    pub file: String,
}

/// Which mapping a device uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MappingChoice {
    /// The mapping whose name matches the device's (see [`match_mapping`]).
    Auto,
    /// Not used as a controller.
    Off,
    /// This mapping file, by file name in the mappings folder.
    File(String),
}

/// A MIDI device that is plugged in, whether or not it has a mapping.
#[derive(Clone, Debug, PartialEq)]
pub struct Port {
    pub name: String,
    /// The file name of the mapping it would use, or uses.
    pub mapping_file: Option<String>,
    /// That mapping's name, while it runs.
    pub running: Option<String>,
}

/// Running controller support. Dropping it stops every controller, which
/// runs the mappings' `shutdown` functions so LEDs go dark.
pub struct Controllers {
    connected: Arc<Mutex<Vec<ConnectedController>>>,
    shared: Arc<Shared>,
    mappings: Vec<(String, PathBuf)>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// What the device thread and the app share besides `connected`.
#[derive(Default)]
struct Shared {
    /// Every MIDI input seen at the last scan, with the mapping file picked.
    ports: Mutex<Vec<(String, Option<PathBuf>)>>,
    /// Choices by [`device_key`]; devices not listed are Auto.
    choices: Mutex<HashMap<String, MappingChoice>>,
    /// The DJ's own mapping run in place of a Mixxx mapping, by the Mixxx
    /// mapping's file name: the own mapping's file.
    versions: Mutex<HashMap<String, PathBuf>>,
    /// Mixxx mappings (by file name) whose controllers restart at the next
    /// scan, because the own mapping they run changed.
    stale: Mutex<HashSet<String>>,
    /// Set when a choice changes, so the next scan comes at once.
    rescan: AtomicBool,
    /// Set by [`Controllers::refresh`]: every device is closed and the
    /// system's MIDI device list is read again from scratch.
    reconnect: AtomicBool,
}

/// A device's name without the parts that change between systems or
/// plug-ins (client and port numbers, "MIDI 1"): what choices are kept by.
pub fn device_key(port: &str) -> String {
    tokens(port).join(" ")
}

impl Controllers {
    /// Starts watching for devices. `choices` are the mappings picked
    /// before, by [`device_key`]; `versions` the DJ's own mappings used in
    /// place of Mixxx mappings (see [`Controllers::set_version`]).
    pub fn start(
        registry: Arc<ControlRegistry>,
        mappings_dir: PathBuf,
        choices: HashMap<String, MappingChoice>,
        versions: HashMap<String, PathBuf>,
    ) -> Self {
        let connected = Arc::new(Mutex::new(Vec::new()));
        let shared = Arc::new(Shared {
            choices: Mutex::new(choices),
            versions: Mutex::new(versions),
            ..Default::default()
        });
        let stop = Arc::new(AtomicBool::new(false));
        let mappings = list_mappings(&mappings_dir);
        let thread = {
            let (connected, shared, stop) = (connected.clone(), shared.clone(), stop.clone());
            let (dir, mappings) = (mappings_dir, mappings.clone());
            std::thread::Builder::new()
                .name("midi-devices".into())
                .spawn(move || watch_devices(registry, dir, mappings, connected, shared, stop))
                .expect("spawn MIDI device thread")
        };
        Self {
            connected,
            shared,
            mappings,
            stop,
            thread: Some(thread),
        }
    }

    pub fn connected(&self) -> Vec<ConnectedController> {
        self.connected.lock().unwrap().clone()
    }

    /// The MIDI devices plugged in now, and what each one runs.
    pub fn ports(&self) -> Vec<Port> {
        let connected = self.connected();
        let mut ports: Vec<Port> = self
            .shared
            .ports
            .lock()
            .unwrap()
            .iter()
            .map(|(name, path)| Port {
                name: name.clone(),
                mapping_file: path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|f| f.to_string_lossy().into_owned()),
                running: connected
                    .iter()
                    .find(|c| &c.port == name)
                    .map(|c| c.mapping.clone()),
            })
            .collect();
        // Stand-in devices (OPENDJ_VIRTUAL_MIDI) are not MIDI ports.
        for c in connected {
            if !ports.iter().any(|p| p.name == c.port) {
                ports.push(Port {
                    name: c.port,
                    mapping_file: Some(c.file),
                    running: Some(c.mapping),
                });
            }
        }
        ports
    }

    /// Every mapping there is: its name and file name.
    pub fn mappings(&self) -> Vec<(String, String)> {
        self.mappings
            .iter()
            .filter_map(|(name, path)| {
                Some((
                    name.clone(),
                    path.file_name()?.to_string_lossy().into_owned(),
                ))
            })
            .collect()
    }

    /// The name of the mapping Automatic picks for `port`.
    pub fn auto_mapping(&self, port: &str) -> Option<String> {
        let path = match_mapping(port, &self.mappings)?;
        self.mappings
            .iter()
            .find(|(_, p)| p == path)
            .map(|(n, _)| n.clone())
    }

    pub fn choice(&self, port: &str) -> MappingChoice {
        self.shared
            .choices
            .lock()
            .unwrap()
            .get(&device_key(port))
            .cloned()
            .unwrap_or(MappingChoice::Auto)
    }

    /// Changes the mapping `port` uses; a running controller restarts with it.
    pub fn set_choice(&self, port: &str, choice: MappingChoice) {
        let mut choices = self.shared.choices.lock().unwrap();
        if choice == MappingChoice::Auto {
            choices.remove(&device_key(port));
        } else {
            choices.insert(device_key(port), choice);
        }
        self.shared.rescan.store(true, Ordering::Relaxed);
    }

    /// Runs the DJ's own mapping in the file `own` (see [`crate::own`]) in
    /// place of the Mixxx mapping `based_on` (a file name), or the Mixxx
    /// mapping itself again for `None`. Its controllers restart with it.
    pub fn set_version(&self, based_on: &str, own: Option<PathBuf>) {
        let mut versions = self.shared.versions.lock().unwrap();
        match own {
            Some(p) => versions.insert(based_on.to_string(), p),
            None => versions.remove(based_on),
        };
        self.reload(based_on);
    }

    /// Restarts the controllers running the Mixxx mapping `based_on`, or an
    /// own mapping based on it: after that own mapping was changed.
    pub fn reload(&self, based_on: &str) {
        self.shared
            .stale
            .lock()
            .unwrap()
            .insert(based_on.to_string());
        self.shared.rescan.store(true, Ordering::Relaxed);
    }

    /// Looks for devices now, closing and reopening every controller: for a
    /// controller that was plugged in but is not showing, or stopped
    /// answering.
    pub fn refresh(&self) {
        self.shared.reconnect.store(true, Ordering::Relaxed);
    }

    pub fn choices(&self) -> HashMap<String, MappingChoice> {
        self.shared.choices.lock().unwrap().clone()
    }

    /// Starts or stops keeping a copy of every message the controllers
    /// send, for the Controller Editor's "press a control to find it".
    pub fn set_monitor(&self, on: bool) {
        *MONITOR.lock().unwrap() = on.then(Vec::new);
    }

    /// While on, messages from the controllers are only watched (see
    /// [`Controllers::set_monitor`]), not acted on: for Map mode, where moving
    /// a control teaches it rather than playing a deck.
    pub fn set_learning(&self, on: bool) {
        LEARNING.store(on, Ordering::Relaxed);
    }

    /// The messages received since the last call, with the port each came
    /// from, while the monitor is on.
    pub fn take_monitored(&self) -> Vec<Monitored> {
        MONITOR
            .lock()
            .unwrap()
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }
}

/// A message from a controller, with the port it came from.
type Monitored = (String, Vec<u8>);

/// Messages kept for [`Controllers::take_monitored`]; `None` while off.
static MONITOR: Mutex<Option<Vec<Monitored>>> = Mutex::new(None);

/// See [`Controllers::set_learning`].
static LEARNING: AtomicBool = AtomicBool::new(false);

fn monitor(port: &str, msg: &[u8]) {
    if let Some(seen) = MONITOR.lock().unwrap().as_mut() {
        // Nobody is reading: don't grow without end.
        if seen.len() < 4096 {
            seen.push((port.to_string(), msg.to_vec()));
        }
    }
}

impl Drop for Controllers {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Words of a device or mapping name, minus ones that say nothing about
/// which device it is ("MIDI", port numbers).
fn tokens(name: &str) -> Vec<String> {
    name.to_lowercase()
        .replace("&amp;", " ")
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .filter(|t| !matches!(*t, "midi" | "port" | "in" | "out" | "usb" | "and"))
        .filter(|t| !t.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_string)
        .collect()
}

/// The mapping for a MIDI port: every word of the port's name must appear in
/// the mapping's name ("DDJ-FLX4" → "Pioneer DDJ-FLX4", "XONE:K2" →
/// "Allen&Heath Xone K1/K2/K3"); among those, the most specific name wins.
pub fn match_mapping<'a>(port: &str, mappings: &'a [(String, PathBuf)]) -> Option<&'a Path> {
    let port_tokens = tokens(port);
    if port_tokens.is_empty() {
        return None;
    }
    mappings
        .iter()
        .filter_map(|(name, path)| {
            let m = tokens(name);
            port_tokens
                .iter()
                .all(|t| m.contains(t))
                .then_some((m.len(), path))
        })
        .min_by_key(|(extra, _)| *extra)
        .map(|(_, p)| p.as_path())
}

/// Names of all MIDI mappings in `dir`.
pub fn list_mappings(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".midi.xml"))
        .filter_map(|p| Mapping::read_name(&p).map(|n| (n, p)))
        .collect();
    v.sort();
    v
}

struct Device {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
    mapping: PathBuf,
    own: Option<PathBuf>,
}

/// The own mapping run in place of the Mixxx mapping at `path`, if any.
fn own_for(path: &Path, versions: &HashMap<String, PathBuf>) -> Option<PathBuf> {
    let file = path.file_name()?.to_string_lossy();
    versions.get(file.as_ref()).filter(|p| p.is_file()).cloned()
}

/// The mapping a controller runs: the Mixxx mapping at `path`, or the own
/// mapping `own` put over it.
pub(crate) fn load_mapping(path: &Path, own: Option<&Path>) -> Result<Mapping, String> {
    let base = Mapping::load(path)?;
    let Some(own) = own else {
        return Ok(base);
    };
    let mine = OpenDjMapping::read(own)?;
    let dir = path.parent().unwrap_or(Path::new("."));
    Ok(mine.build(Some(&base), dir))
}

/// The mapping file `port` should run, given the choices.
fn pick_mapping(
    port: &str,
    dir: &Path,
    mappings: &[(String, PathBuf)],
    choices: &HashMap<String, MappingChoice>,
) -> Option<PathBuf> {
    match choices.get(&device_key(port)) {
        Some(MappingChoice::Off) => None,
        Some(MappingChoice::File(f)) if dir.join(f).is_file() => Some(dir.join(f)),
        _ => match_mapping(port, mappings).map(Path::to_path_buf),
    }
}

fn watch_devices(
    registry: Arc<ControlRegistry>,
    dir: PathBuf,
    mappings: Vec<(String, PathBuf)>,
    connected: Arc<Mutex<Vec<ConnectedController>>>,
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
) {
    let versions = shared.versions.lock().unwrap().clone();
    let virtual_devices = start_virtual_devices(&registry, &mappings, &versions, &connected, &stop);
    let Ok(mut input) = MidiInput::new(CLIENT) else {
        eprintln!("MIDI is unavailable; controllers are disabled");
        for t in virtual_devices {
            let _ = t.join();
        }
        return;
    };
    let mut devices: HashMap<String, Device> = HashMap::new();
    let mut last_scan: Option<Instant> = None;
    while !stop.load(Ordering::Relaxed) {
        let mut rescan = shared.rescan.swap(false, Ordering::Relaxed);
        if shared.reconnect.swap(false, Ordering::Relaxed) {
            rescan = true;
            for (port, d) in devices.drain() {
                d.stop.store(true, Ordering::Relaxed);
                let _ = d.thread.join();
                connected.lock().unwrap().retain(|c| c.port != port);
            }
            if let Ok(fresh) = MidiInput::new(CLIENT) {
                input = fresh;
            }
        }
        if rescan || last_scan.is_none_or(|t| t.elapsed() >= RESCAN) {
            last_scan = Some(Instant::now());
            let ports: Vec<String> = input
                .ports()
                .iter()
                .filter_map(|p| input.port_name(p).ok())
                .filter(|n| !n.contains(CLIENT) && !n.contains("Midi Through"))
                .collect();
            let picked: Vec<(String, Option<PathBuf>)> = {
                let choices = shared.choices.lock().unwrap();
                ports
                    .iter()
                    .map(|p| (p.clone(), pick_mapping(p, &dir, &mappings, &choices)))
                    .collect()
            };
            let versions = shared.versions.lock().unwrap().clone();
            let stale = std::mem::take(&mut *shared.stale.lock().unwrap());
            // Unplugged, its thread ended (e.g. the mapping failed to load),
            // given another mapping, or its own mapping changed.
            let gone: Vec<String> = devices
                .iter()
                .filter(|(port, d)| {
                    let file = d.mapping.file_name().map(|f| f.to_string_lossy());
                    d.thread.is_finished()
                        || file.is_some_and(|f| stale.contains(f.as_ref()))
                        || d.own != own_for(&d.mapping, &versions)
                        || !picked
                            .iter()
                            .any(|(p, m)| p == *port && m.as_ref() == Some(&d.mapping))
                })
                .map(|(port, _)| port.clone())
                .collect();
            for port in gone {
                let d = devices.remove(&port).unwrap();
                d.stop.store(true, Ordering::Relaxed);
                let _ = d.thread.join();
                connected.lock().unwrap().retain(|c| c.port != port);
            }
            *shared.ports.lock().unwrap() = picked.clone();
            for (port, path) in picked {
                if devices.contains_key(&port)
                    || connected.lock().unwrap().iter().any(|c| c.port == port)
                {
                    continue;
                }
                let Some(path) = path else {
                    continue;
                };
                let own = own_for(&path, &versions);
                let d_stop = Arc::new(AtomicBool::new(false));
                let thread = {
                    let (registry, path, own, port, d_stop, connected) = (
                        registry.clone(),
                        path.clone(),
                        own.clone(),
                        port.clone(),
                        d_stop.clone(),
                        connected.clone(),
                    );
                    std::thread::Builder::new()
                        .name(format!("midi {port}"))
                        .spawn(move || {
                            run_device(&port, &path, own.as_deref(), registry, &d_stop, &connected)
                        })
                        .expect("spawn controller thread")
                };
                devices.insert(
                    port,
                    Device {
                        stop: d_stop,
                        thread,
                        mapping: path,
                        own,
                    },
                );
            }
        }
        wait_for_system(Duration::from_millis(100));
    }
    for (_, d) in devices.drain() {
        d.stop.store(true, Ordering::Relaxed);
        let _ = d.thread.join();
    }
    for t in virtual_devices {
        let _ = t.join();
    }
}

/// Waits `time` on the device thread. On macOS the thread's run loop runs
/// meanwhile: CoreMIDI tells this process about devices plugged in or out
/// through the run loop of the thread that opened MIDI first, and without it
/// the device list stays as it was at start.
#[cfg(target_os = "macos")]
fn wait_for_system(time: Duration) {
    use std::ffi::c_void;
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFRunLoopDefaultMode: *const c_void;
        fn CFRunLoopRunInMode(mode: *const c_void, seconds: f64, return_after: u8) -> i32;
    }
    const FINISHED: i32 = 1;
    let start = Instant::now();
    // SAFETY: runs this thread's own run loop in the default mode.
    let result = unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, time.as_secs_f64(), 0) };
    // With nothing to wait on, the run loop returns at once.
    if result == FINISHED {
        std::thread::sleep(time.saturating_sub(start.elapsed()));
    }
}

#[cfg(not(target_os = "macos"))]
fn wait_for_system(time: Duration) {
    std::thread::sleep(time);
}

fn run_device(
    port: &str,
    mapping_path: &Path,
    own: Option<&Path>,
    registry: Arc<ControlRegistry>,
    stop: &AtomicBool,
    connected: &Mutex<Vec<ConnectedController>>,
) {
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let Ok(mut input) = MidiInput::new(CLIENT) else {
        return;
    };
    input.ignore(Ignore::TimeAndActiveSense);
    let Some(in_port) = input
        .ports()
        .into_iter()
        .find(|p| input.port_name(p).as_deref() == Ok(port))
    else {
        return;
    };
    let _in_conn = match input.connect(
        &in_port,
        "openDJ in",
        move |_, msg, _| {
            let _ = tx.send(msg.to_vec());
        },
        (),
    ) {
        Ok(c) => c,
        Err(e) => {
            // Windows lets only one app at a time open a MIDI device.
            let hint = if cfg!(windows) {
                " (is another DJ app using it?)"
            } else {
                ""
            };
            eprintln!("{port}: cannot open MIDI input: {e}{hint}");
            return;
        }
    };
    // The output with the same name, or failing that the closest one: some
    // systems name the two directions slightly differently.
    let mut out_conn = MidiOutput::new(CLIENT).ok().and_then(|output| {
        let ports = output.ports();
        let want = tokens(port);
        let p = ports
            .iter()
            .find(|p| output.port_name(p).as_deref() == Ok(port))
            .or_else(|| {
                ports
                    .iter()
                    .find(|p| output.port_name(p).is_ok_and(|n| tokens(&n) == want))
            })?
            .clone();
        output.connect(&p, "openDJ out").ok()
    });
    let send = move |m: &[u8]| {
        if let Some(out) = out_conn.as_mut() {
            let _ = out.send(m);
        }
    };
    run_controller(port, mapping_path, own, registry, stop, connected, rx, send);
}

/// Runs a mapping until `stop` is set or the input ends.
#[allow(clippy::too_many_arguments)]
fn run_controller(
    port: &str,
    mapping_path: &Path,
    own: Option<&Path>,
    registry: Arc<ControlRegistry>,
    stop: &AtomicBool,
    connected: &Mutex<Vec<ConnectedController>>,
    rx: mpsc::Receiver<Vec<u8>>,
    mut send: impl FnMut(&[u8]),
) {
    let mapping = match load_mapping(mapping_path, own) {
        Ok(m) => m,
        Err(e) => {
            let file = own.unwrap_or(mapping_path);
            eprintln!("{port}: cannot read {}: {e}", file.display());
            return;
        }
    };
    let mapping_name = mapping.name.clone();
    let mut controller = match Controller::new(port, mapping, registry) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{port}: mapping {mapping_name} failed to load: {e}");
            return;
        }
    };
    eprintln!("{port}: using mapping {mapping_name}");
    connected.lock().unwrap().push(ConnectedController {
        port: port.to_string(),
        mapping: mapping_name,
        file: mapping_path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default(),
    });
    let mut flush = |c: &mut Controller| {
        for m in c.take_output() {
            send(&m);
        }
    };
    flush(&mut controller);
    while !stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        let deadline = controller
            .next_deadline()
            .map_or(now + POLL, |d| d.min(now + POLL));
        match rx.recv_timeout(deadline.saturating_duration_since(now)) {
            Ok(msg) => {
                let learning = LEARNING.load(Ordering::Relaxed);
                monitor(port, &msg);
                if !learning {
                    controller.receive(&msg);
                }
                while let Ok(msg) = rx.try_recv() {
                    monitor(port, &msg);
                    if !learning {
                        controller.receive(&msg);
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        controller.poll(Instant::now());
        flush(&mut controller);
    }
    controller.shutdown();
    controller.poll(Instant::now());
    flush(&mut controller);
    connected.lock().unwrap().retain(|c| c.port != port);
}

/// A stand-in device for trying mappings without hardware:
/// `OPENDJ_VIRTUAL_MIDI="Midi Fighter Twister=/tmp/twister"` reads MIDI
/// messages as hex lines ("B0 11 40") from that file or FIFO, as if they came
/// from a device with that name. Messages to the device are printed.
fn start_virtual_devices(
    registry: &Arc<ControlRegistry>,
    mappings: &[(String, PathBuf)],
    versions: &HashMap<String, PathBuf>,
    connected: &Arc<Mutex<Vec<ConnectedController>>>,
    stop: &Arc<AtomicBool>,
) -> Vec<JoinHandle<()>> {
    let Ok(spec) = std::env::var("OPENDJ_VIRTUAL_MIDI") else {
        return Vec::new();
    };
    let mut threads = Vec::new();
    for entry in spec.split(';').filter(|e| !e.is_empty()) {
        let Some((port, file)) = entry.split_once('=') else {
            continue;
        };
        let Some(path) = match_mapping(port, mappings).map(Path::to_path_buf) else {
            eprintln!("{port}: no mapping matches this name");
            continue;
        };
        let own = own_for(&path, versions);
        let (port, file) = (port.to_string(), PathBuf::from(file));
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            use std::io::BufRead;
            let Ok(f) = std::fs::File::open(&file) else {
                return;
            };
            for line in std::io::BufReader::new(f).lines().map_while(Result::ok) {
                let bytes: Vec<u8> = line
                    .split_whitespace()
                    .filter_map(|b| u8::from_str_radix(b, 16).ok())
                    .collect();
                if !bytes.is_empty() && tx.send(bytes).is_err() {
                    break;
                }
            }
        });
        let (registry, connected, stop) = (registry.clone(), connected.clone(), stop.clone());
        threads.push(std::thread::spawn(move || {
            let name = port.clone();
            run_controller(
                &port,
                &path,
                own.as_deref(),
                registry,
                &stop,
                &connected,
                rx,
                |m| eprintln!("{name} <- {m:02X?}"),
            );
        }));
    }
    threads
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_ports_to_mappings() {
        let dir = crate::default_mappings_dir().unwrap();
        let mappings = list_mappings(&dir);
        assert!(mappings.len() > 100);
        let name = |port: &str| {
            match_mapping(port, &mappings)
                .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        };
        for (port, file) in [
            ("DDJ-FLX4", "Pioneer-DDJ-FLX4.midi.xml"),
            ("DDJ-FLX4:DDJ-FLX4 MIDI 1 24:0", "Pioneer-DDJ-FLX4.midi.xml"),
            ("XONE:K2", "Allen and Heath Xone K2.midi.xml"),
            (
                "XONE:K2:XONE:K2 MIDI 1 28:0",
                "Allen and Heath Xone K2.midi.xml",
            ),
            (
                "Midi Fighter Twister",
                "DJ TechTools MIDI Fighter Twister.midi.xml",
            ),
            (
                "Midi Fighter Twister:Midi Fighter Twister MIDI 1 32:0",
                "DJ TechTools MIDI Fighter Twister.midi.xml",
            ),
        ] {
            assert_eq!(name(port).as_deref(), Some(file), "{port}");
        }
        assert_eq!(name("Generic USB MIDI 1"), None);
    }
}
