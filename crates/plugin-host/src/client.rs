//! openDJ's side: starts a helper and trades audio blocks with it.
//!
//! [`spawn`] returns two halves. The [`Connection`] goes to the audio thread:
//! [`Connection::process`] never allocates, never locks and never waits
//! longer than the deadline it is given. The [`Helper`] stays on a normal
//! thread and watches the process (exit, kill).

use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::clap::Descriptor;
use crate::effects::{Info, Param, Plugin};
use crate::shm::{self, Shm, CHANNELS, MAX_PARAMS};

/// How long a new helper has to say it is ready.
pub const READY_TIMEOUT: Duration = Duration::from_secs(10);

/// What to start.
#[derive(Clone, Debug)]
pub struct Options {
    /// The `opendj-plugin-host` binary.
    pub helper: PathBuf,
    /// Which effect the helper loads.
    pub plugin: Plugin,
    pub sample_rate: u32,
    /// The longest block `process` will be given.
    pub max_frames: usize,
}

/// The routed deck's tempo and position, for beat-synced plugins.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Transport {
    pub bpm: f64,
    pub beat: f64,
    pub playing: bool,
}

/// What became of a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The plugin's output.
    Wet,
    /// The helper missed the deadline; the block is dry.
    Late,
    /// The helper is still working on an earlier, late block; dry.
    Busy,
    /// The helper is gone (crashed, killed or never started); dry.
    Gone,
}

impl Outcome {
    pub fn is_wet(self) -> bool {
        self == Outcome::Wet
    }
}

/// The audio thread's half.
pub struct Connection {
    shm: Arc<Shm>,
    wake: ChildStdin,
    seq: u64,
    /// A handed-over block the helper hasn't finished.
    pending: bool,
    gone: bool,
    missed_in_row: u32,
}

/// The control thread's half.
pub struct Helper {
    child: Child,
    shm_path: PathBuf,
    messages: Receiver<String>,
    shm: Arc<Shm>,
    info: Info,
}

/// The helper's arguments that pick the plugin.
fn plugin_args(plugin: &Plugin) -> Vec<std::ffi::OsString> {
    match plugin {
        Plugin::Test(id) => vec!["--test".into(), id.into()],
        Plugin::Clap { file, id } => vec![
            "--clap".into(),
            file.clone().into_os_string(),
            "--id".into(),
            id.into(),
        ],
    }
}

/// One line of the helper's description (see the helper's `describe`).
fn read_info(info: &mut Info, line: &str) {
    let mut f = line.split('\t');
    match f.next() {
        Some("name") => info.name = f.next().unwrap_or_default().to_string(),
        Some("latency") => info.latency = f.next().and_then(|v| v.parse().ok()).unwrap_or(0),
        Some("param") => {
            let mut num = || f.next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            let (min, max, default, value) = (num(), num(), num(), num());
            let name = f.next().unwrap_or_default().to_string();
            info.params.push(Param {
                name,
                min,
                max,
                default,
                value,
            });
        }
        _ => {}
    }
}

/// Why a file could not be scanned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanError {
    /// It took longer than the timeout (a licence dialog, a hang).
    TimedOut,
    /// The scanner died; the text says how.
    Crashed(String),
    /// The file loaded but isn't usable; the text says why.
    Failed(String),
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScanError::TimedOut => write!(f, "took too long to scan"),
            ScanError::Crashed(how) => write!(f, "crashed while scanning ({how})"),
            ScanError::Failed(why) => write!(f, "{why}"),
        }
    }
}

/// Lists the plugins in one CLAP file, in a throwaway helper that is killed
/// after `timeout`. Loading a plugin runs its code, so this never happens
/// inside openDJ.
pub fn scan(helper: &Path, file: &Path, timeout: Duration) -> Result<Vec<Descriptor>, ScanError> {
    let mut child = Command::new(helper)
        .arg("--scan")
        .arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| ScanError::Failed(e.to_string()))?;
    let stdout = child.stdout.take().expect("piped stdout");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let end = Instant::now() + timeout;
    let mut found = Vec::new();
    loop {
        let left = end.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(line) if line == "done" => break,
            Ok(line) => {
                if let Some(msg) = line.strip_prefix("error ") {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ScanError::Failed(msg.to_string()));
                }
                let f: Vec<&str> = line.split('\t').collect();
                if let ["plugin", id, name, vendor, version, features] = f[..] {
                    found.push(Descriptor {
                        id: id.into(),
                        name: name.into(),
                        vendor: vendor.into(),
                        version: version.into(),
                        features: features
                            .split(',')
                            .filter(|s| !s.is_empty())
                            .map(String::from)
                            .collect(),
                    });
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ScanError::TimedOut);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let status = child.wait().map(|s| s.to_string()).unwrap_or_default();
                return Err(ScanError::Crashed(status));
            }
        }
    }
    let _ = child.wait();
    Ok(found)
}

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// Where the shared block for a new helper goes: memory-backed `/dev/shm`
/// on Linux, the temp folder elsewhere.
fn shm_path() -> PathBuf {
    let dir = if cfg!(target_os = "linux") && Path::new("/dev/shm").is_dir() {
        PathBuf::from("/dev/shm")
    } else {
        std::env::temp_dir()
    };
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    dir.join(format!("opendj-plugin-{}-{id}", std::process::id()))
}

/// Starts a helper for `opts.plugin` and waits (up to [`READY_TIMEOUT`]) for
/// it to load. Call this off the audio thread.
pub fn spawn(opts: &Options) -> io::Result<(Connection, Helper)> {
    let path = shm_path();
    let shm = Arc::new(Shm::create(&path, opts.max_frames, opts.sample_rate)?);
    let child = Command::new(&opts.helper)
        .arg("--shm")
        .arg(&path)
        .args(plugin_args(&opts.plugin))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            let _ = std::fs::remove_file(&path);
            return Err(e);
        }
    };
    let wake = child.stdin.take().expect("piped stdin");
    let stdout = child.stdout.take().expect("piped stdout");
    let (tx, messages) = mpsc::channel();
    std::thread::Builder::new()
        .name("plugin-host-messages".into())
        .spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        })?;
    let mut helper = Helper {
        child,
        shm_path: path,
        messages,
        shm: shm.clone(),
        info: Info::default(),
    };
    // It describes the plugin, line by line, then says "ready".
    let end = Instant::now() + READY_TIMEOUT;
    loop {
        let left = end.saturating_duration_since(Instant::now());
        match helper.messages.recv_timeout(left) {
            Ok(line) if line == "ready" => break,
            Ok(line) => {
                if let Some(msg) = line.strip_prefix("error ") {
                    helper.kill();
                    return Err(io::Error::other(msg.to_string()));
                }
                read_info(&mut helper.info, &line);
            }
            Err(_) => {
                helper.kill();
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "the plugin did not start in time",
                ));
            }
        }
    }
    // Both sides have it mapped: on Unix the name can go now, so nothing is
    // left behind even if both processes die.
    #[cfg(unix)]
    let _ = std::fs::remove_file(&helper.shm_path);
    let conn = Connection {
        shm,
        wake,
        seq: 0,
        pending: false,
        gone: false,
        missed_in_row: 0,
    };
    Ok((conn, helper))
}

fn copy_dry(input: [&[f32]; CHANNELS], output: &mut [&mut [f32]; CHANNELS]) {
    for ch in 0..CHANNELS {
        output[ch].copy_from_slice(input[ch]);
    }
}

impl Connection {
    /// Runs one block through the plugin, or copies it dry if the helper
    /// can't deliver within `deadline`. Real-time safe.
    pub fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        mut output: [&mut [f32]; CHANNELS],
        transport: Transport,
        deadline: Duration,
    ) -> Outcome {
        let frames = input[0].len();
        assert!(
            frames <= self.shm.max_frames(),
            "block longer than max_frames"
        );
        assert!(
            input.iter().all(|c| c.len() == frames) && output.iter().all(|c| c.len() == frames)
        );
        let start = Instant::now();
        let h = self.shm.header();
        if self.gone {
            copy_dry(input, &mut output);
            return Outcome::Gone;
        }
        if self.pending {
            if h.seq_out.load(Ordering::Acquire) == self.seq {
                // The late block finished; its output is stale, drop it.
                self.pending = false;
            } else {
                self.missed_in_row += 1;
                copy_dry(input, &mut output);
                return Outcome::Busy;
            }
        }
        for (ch, data) in input.iter().enumerate() {
            self.shm.write(false, ch, data);
        }
        h.frames.store(frames as u32, Ordering::Relaxed);
        shm::store_f64(&h.bpm, transport.bpm);
        shm::store_f64(&h.beat, transport.beat);
        h.playing.store(transport.playing as u32, Ordering::Relaxed);
        self.seq += 1;
        h.seq_in.store(self.seq, Ordering::Release);
        // One byte per block, and only while the helper is idle, so the pipe
        // never fills and this write never blocks.
        if self.wake.write_all(&[1]).is_err() {
            self.gone = true;
            copy_dry(input, &mut output);
            return Outcome::Gone;
        }
        let end = start + deadline;
        let mut spins = 0u32;
        loop {
            if h.seq_out.load(Ordering::Acquire) == self.seq {
                for (ch, data) in output.iter_mut().enumerate() {
                    self.shm.read(true, ch, data);
                }
                self.missed_in_row = 0;
                return Outcome::Wet;
            }
            spins += 1;
            if spins.is_multiple_of(64) && Instant::now() >= end {
                break;
            }
            std::hint::spin_loop();
        }
        self.pending = true;
        self.missed_in_row += 1;
        copy_dry(input, &mut output);
        Outcome::Late
    }

    /// Sets parameter `index` (0..32); the helper sees it on its next block.
    pub fn set_param(&self, index: usize, value: f32) {
        if index < MAX_PARAMS {
            shm::store_f32(&self.shm.header().params[index], value);
        }
    }

    /// Blocks in a row that came back dry because the helper was late or busy.
    pub fn missed_in_row(&self) -> u32 {
        self.missed_in_row
    }

    pub fn is_gone(&self) -> bool {
        self.gone
    }
}

/// Whether a helper is still running.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Running,
    /// It ended; the text says how (exit code or signal).
    Exited(String),
}

impl Helper {
    pub fn status(&mut self) -> Status {
        match self.child.try_wait() {
            Ok(None) => Status::Running,
            Ok(Some(s)) => Status::Exited(s.to_string()),
            Err(e) => Status::Exited(e.to_string()),
        }
    }

    /// Waits up to `timeout` for the helper to end.
    pub fn wait_exit(&mut self, timeout: Duration) -> Status {
        let end = Instant::now() + timeout;
        loop {
            let s = self.status();
            if s != Status::Running || Instant::now() >= end {
                return s;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Stops a hung or unwanted helper.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// How much of the last block's time the plugin used (1.0 = all of it).
    pub fn load(&self) -> f32 {
        let h = self.shm.header();
        let frames = h.frames.load(Ordering::Relaxed) as f64;
        let rate = h.sample_rate.load(Ordering::Relaxed) as f64;
        if frames == 0.0 || rate == 0.0 {
            return 0.0;
        }
        let block_ns = frames / rate * 1e9;
        (h.process_ns.load(Ordering::Relaxed) as f64 / block_ns) as f32
    }

    /// Lines the helper printed after it started (errors, notes).
    pub fn messages(&self) -> impl Iterator<Item = String> + '_ {
        self.messages.try_iter()
    }

    /// The plugin's name, parameters and latency, as it reported them.
    pub fn info(&self) -> &Info {
        &self.info
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        self.kill();
        let _ = std::fs::remove_file(&self.shm_path);
    }
}
