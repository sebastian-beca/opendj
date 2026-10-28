//! Stems: a track split into up to four parts (drums, bass, other and
//! vocals) that play together with their own volume and mute, as in Mixxx 2.6.
//!
//! Stems come from Native Instruments STEM files (`.stem.mp4` / `.stem.m4a`):
//! an MP4 with the full mix as its first audio stream, the four stems as the
//! next four, and a JSON manifest at `moov/udta/stem` naming and colouring
//! them. Or they come from separating an ordinary file with Demucs (see
//! [`separate`]), which leaves four FLAC files in the cache.
//!
//! The full mix stays the track's [`Audio`](crate::track::Audio), for the
//! waveform and analysis; a deck with stems plays their sum instead.

use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;

/// Stems per track, as in NI's format and Mixxx.
pub const MAX_STEMS: usize = 5;

/// Stems in an NI STEM file or a Demucs separation.
const FILE_STEMS: usize = 4;

/// Where a drums stem splits into kick (below) and hi-hats (above, with the
/// snares and claps).
pub(crate) const KICK_SPLIT_HZ: f64 = 150.0;

/// A stem's name and colour (0xRRGGBB).
#[derive(Clone, Debug, PartialEq)]
pub struct StemInfo {
    pub name: String,
    pub color: u32,
}

/// The names and colours of separated stems, VirtualDJ's (drums become
/// Kick and HiHat, see [`split_drums`]); NI STEM files without their own
/// take them too. In NI's order: drums, bass, other ("synth") and vocals.
pub fn default_info() -> Vec<StemInfo> {
    [
        ("Drums", KICK_COLOR),
        ("Bass", 0xA21418),
        ("Instru", 0xA26018),
        ("Vocal", 0x09AD18),
    ]
    .map(|(name, color)| StemInfo {
        name: name.into(),
        color,
    })
    .to_vec()
}

const KICK_COLOR: u32 = 0x2E4E75;
const HIHAT_COLOR: u32 = 0x0960B1;

/// Frames per chunk, as in [`Audio`](crate::track::Audio).
pub(crate) const CHUNK: usize = 1 << 16;

/// One frame of all stems, 16-bit: half the memory of floats, and stems are
/// five times a track's audio.
pub(crate) type StemFrame = [[i16; 2]; MAX_STEMS];

/// How a track's stems were made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    /// Spleeter, when the track was loaded: quick, with some bleed.
    Fast,
    /// HTDemucs (or an NI STEM file): kept in the cache.
    High,
}

/// A track's stems, filled in while they may already be playing: decoded
/// in order, or separated a chunk at a time from wherever the DJ is.
/// Frames not there yet read as silence, or as the original for separated
/// stems (see [`Stems::mix`]).
pub struct Stems {
    pub info: Vec<StemInfo>,
    pub sample_rate: u32,
    pub quality: Quality,
    /// Separated from the track rather than mastered with it, so they play
    /// as the original minus what is turned down (see [`Stems::mix`]).
    separated: bool,
    chunks: Box<[OnceLock<Box<[StemFrame]>>]>,
    decoded: AtomicUsize,
    complete: AtomicBool,
    /// Better stems that replace these once they are there.
    upgrade: OnceLock<Arc<Stems>>,
}

impl Stems {
    fn new(info: Vec<StemInfo>, sample_rate: u32, expected_frames: usize) -> Self {
        Stems {
            info,
            sample_rate,
            quality: Quality::High,
            separated: false,
            chunks: (0..expected_frames / CHUNK + 2)
                .map(|_| OnceLock::new())
                .collect(),
            decoded: AtomicUsize::new(0),
            complete: AtomicBool::new(false),
            upgrade: OnceLock::new(),
        }
    }

    /// Separated stems of `frames` frames, filled in by [`Stems::fill`].
    pub(crate) fn separated(
        info: Vec<StemInfo>,
        sample_rate: u32,
        frames: usize,
        quality: Quality,
    ) -> Stems {
        Stems {
            quality,
            separated: true,
            ..Stems::new(info, sample_rate, frames)
        }
    }

    /// Fills chunk `k`, the frames from `k * CHUNK`; the last is complete
    /// once every chunk is filled.
    pub(crate) fn fill(&self, k: usize, frames: Box<[StemFrame]>) {
        let n = frames.len();
        if let Some(slot) = self.chunks.get(k) {
            if slot.set(frames).is_ok() {
                self.decoded.fetch_add(n, Ordering::AcqRel);
            }
        }
    }

    /// Marks the stems as complete.
    pub(crate) fn finish(&self) {
        self.complete.store(true, Ordering::Release);
    }

    /// The stems that play: the upgrade once it is complete, else these.
    pub fn current(&self) -> &Stems {
        match self.upgrade.get() {
            Some(u) if u.is_complete() => u,
            _ => self,
        }
    }

    /// Sets the better stems that replace these once they are complete.
    /// False if there already are some, or they don't fit.
    pub fn set_upgrade(&self, stems: Arc<Stems>) -> bool {
        stems.sample_rate == self.sample_rate
            && stems.count() == self.count()
            && self.upgrade.set(stems).is_ok()
    }

    /// Stems that are already decoded: `frames[i][k]` is stem k's frame i.
    pub fn from_frames(
        info: Vec<StemInfo>,
        sample_rate: u32,
        frames: &[[[f32; 2]; MAX_STEMS]],
    ) -> Arc<Stems> {
        let stems = Stems::new(info, sample_rate, frames.len());
        for (slot, chunk) in stems.chunks.iter().zip(frames.chunks(CHUNK)) {
            let chunk: Box<[StemFrame]> = chunk
                .iter()
                .map(|f| f.map(|[l, r]| [to_i16(l), to_i16(r)]))
                .collect();
            let _ = slot.set(chunk);
        }
        stems.decoded.store(frames.len(), Ordering::Release);
        stems.complete.store(true, Ordering::Release);
        Arc::new(stems)
    }

    pub fn count(&self) -> usize {
        self.info.len().min(MAX_STEMS)
    }

    pub fn decoded(&self) -> usize {
        self.decoded.load(Ordering::Acquire)
    }

    pub fn is_complete(&self) -> bool {
        self.complete.load(Ordering::Acquire)
    }

    /// The mix of frame `i` with each stem scaled by its gain. Separated
    /// stems play as `original() − Σ (1 − gain) × stem`: the track itself
    /// at full gains, and the original where the stems aren't separated
    /// yet. Stems from an NI file play as their sum.
    #[inline]
    pub fn mix(
        &self,
        i: usize,
        gains: &[f32; MAX_STEMS],
        original: impl FnOnce() -> [f32; 2],
    ) -> [f32; 2] {
        let stems = self.current();
        let f = stems
            .chunks
            .get(i / CHUNK)
            .and_then(OnceLock::get)
            .and_then(|c| c.get(i % CHUNK));
        if stems.separated {
            let mut out = original();
            if let Some(f) = f {
                for (s, g) in f.iter().zip(gains) {
                    let cut = (1.0 - g) / 32_768.0;
                    out[0] -= s[0] as f32 * cut;
                    out[1] -= s[1] as f32 * cut;
                }
            }
            return out;
        }
        let Some(f) = f else {
            return [0.0; 2];
        };
        let mut out = [0.0; 2];
        for (s, g) in f.iter().zip(gains) {
            out[0] += s[0] as f32 * g;
            out[1] += s[1] as f32 * g;
        }
        [out[0] / 32_768.0, out[1] / 32_768.0]
    }

    /// How the stems that play now were made.
    pub fn playing_quality(&self) -> Quality {
        self.current().quality
    }
}

impl std::fmt::Debug for Stems {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Stems({} @ {} Hz)", self.count(), self.sample_rate)
    }
}

/// Converts a sample to 16 bits, clipping.
pub(crate) fn to_i16(x: f32) -> i16 {
    (x * 32_768.0).round().clamp(-32_768.0, 32_767.0) as i16
}

/// One stream being decoded: a file, and which of its audio tracks.
struct Input {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    rate: u32,
    frames: Option<u64>,
    queue: VecDeque<[i16; 2]>,
    done: bool,
    buf: Vec<f32>,
}

impl Input {
    /// Opens the `index`th audio track of `path`.
    fn open(path: &Path, index: usize) -> Result<Input, String> {
        let err = |e: Error| format!("{}: {e}", path.display());
        let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let format = symphonia::default::get_probe()
            .probe(
                &hint,
                mss,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(err)?;
        let track = format
            .tracks()
            .iter()
            .filter(|t| matches!(t.codec_params, Some(CodecParameters::Audio(_))))
            .nth(index)
            .ok_or_else(|| format!("{}: no audio stream {index}", path.display()))?;
        let Some(CodecParameters::Audio(params)) = track.codec_params.clone() else {
            unreachable!("filtered above");
        };
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&params, &AudioDecoderOptions::default())
            .map_err(err)?;
        Ok(Input {
            track_id: track.id,
            rate: params.sample_rate.unwrap_or(0),
            frames: track.num_frames,
            format,
            decoder,
            queue: VecDeque::new(),
            done: false,
            buf: Vec::new(),
        })
    }

    /// Decodes this stream's next packet into the queue.
    fn step(&mut self) -> Result<(), String> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(p)) => p,
                Ok(None) => {
                    self.done = true;
                    return Ok(());
                }
                Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    self.done = true;
                    return Ok(());
                }
                Err(e) => return Err(e.to_string()),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(d) => d,
                Err(Error::DecodeError(_)) => continue,
                Err(e) => return Err(e.to_string()),
            };
            let spec = decoded.spec();
            if self.rate == 0 {
                self.rate = spec.rate();
            }
            let channels = spec.channels().count().max(1);
            decoded.copy_to_vec_interleaved(&mut self.buf);
            for f in self.buf.chunks_exact(channels) {
                let r = if channels > 1 { f[1] } else { f[0] };
                self.queue.push_back([to_i16(f[0]), to_i16(r)]);
            }
            return Ok(());
        }
    }
}

/// Where a track's stems come from.
#[derive(Clone, Debug, PartialEq)]
pub enum StemSource {
    /// An NI STEM file: audio streams 1 to 4 of the file itself.
    NiStem(PathBuf),
    /// One file per stem, in [`default_info`]'s order.
    Files(Vec<PathBuf>),
}

/// Where the stems of the file at `path` are, if it has any: the file itself
/// when it is an NI STEM file, else a finished separation in the cache.
pub fn find(path: &Path) -> Option<StemSource> {
    if is_ni_stem(path) {
        return Some(StemSource::NiStem(path.to_owned()));
    }
    let files = separated_files(&crate::cache::stems_dir(path)?);
    files
        .iter()
        .all(|f| f.is_file())
        .then_some(StemSource::Files(files))
}

/// Whether `path` is named like an NI STEM file.
pub fn is_ni_stem(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    name.ends_with(".stem.mp4") || name.ends_with(".stem.m4a")
}

/// The four files a separation into `dir` leaves.
fn separated_files(dir: &Path) -> Vec<PathBuf> {
    ["drums", "bass", "other", "vocals"]
        .iter()
        .map(|s| dir.join(format!("{s}.flac")))
        .collect()
}

/// Opens `source`'s stems; they decode as [`Decode::run`] goes.
pub struct Decode {
    stems: Arc<Stems>,
    inputs: Vec<Input>,
    /// Splits the first stem, drums, into kick and hi-hats (see
    /// [`split_drums`]).
    split: Option<crate::dsp::Lr4>,
}

/// Splits a drums frame into kick (through `low`) and hi-hats, the rest: the
/// two add up to the drums exactly, so taking both out of a separated track
/// takes out the drums and nothing else.
pub(crate) fn split(low: &mut crate::dsp::Lr4, drums: [f32; 2]) -> ([f32; 2], [f32; 2]) {
    let kick = low.process(drums);
    (kick, [drums[0] - kick[0], drums[1] - kick[1]])
}

/// With the usual four stems, the drums stem (first) becomes a kick stem and
/// a fifth, hi-hat stem takes the rest of the kit.
pub(crate) fn split_drums(info: &mut Vec<StemInfo>) -> bool {
    if info.len() != FILE_STEMS || !info[0].name.eq_ignore_ascii_case("drums") {
        return false;
    }
    info[0].name = "Kick".into();
    info.push(StemInfo {
        name: "HiHat".into(),
        color: HIHAT_COLOR,
    });
    true
}

impl Decode {
    pub fn open(source: &StemSource) -> Result<Decode, String> {
        let (inputs, info) = match source {
            StemSource::NiStem(path) => {
                let info = read_manifest(path).unwrap_or_else(default_info);
                let inputs = (1..=FILE_STEMS)
                    .map(|i| Input::open(path, i))
                    .collect::<Result<Vec<_>, _>>()?;
                (inputs, info)
            }
            StemSource::Files(files) => {
                let inputs = files
                    .iter()
                    .map(|f| Input::open(f, 0))
                    .collect::<Result<Vec<_>, _>>()?;
                (inputs, default_info())
            }
        };
        let rate = inputs[0].rate;
        if rate == 0 || inputs.iter().any(|i| i.rate != rate) {
            return Err("stems have different or unknown sample rates".into());
        }
        let expected = inputs.iter().filter_map(|i| i.frames).max().unwrap_or(0) as usize;
        let mut info = info;
        info.truncate(inputs.len());
        let split =
            split_drums(&mut info).then(|| crate::dsp::Lr4::lowpass(rate as f64, KICK_SPLIT_HZ));
        let mut stems = Stems::new(info, rate, expected.max(CHUNK));
        stems.separated = matches!(source, StemSource::Files(_));
        Ok(Decode {
            stems: Arc::new(stems),
            inputs,
            split,
        })
    }

    pub fn stems(&self) -> &Arc<Stems> {
        &self.stems
    }

    /// Decodes to the end; stops early (with an error) if `keep_going`
    /// returns false.
    pub fn run(mut self, mut keep_going: impl FnMut() -> bool) -> Result<Arc<Stems>, String> {
        let stems = self.stems.clone();
        let mut buf: Vec<StemFrame> = Vec::with_capacity(CHUNK);
        let mut chunk = 0;
        let mut publish = |buf: &mut Vec<StemFrame>| {
            if buf.is_empty() {
                return;
            }
            let n = buf.len();
            let frames = std::mem::replace(buf, Vec::with_capacity(CHUNK));
            if let Some(slot) = stems.chunks.get(chunk) {
                let _ = slot.set(frames.into_boxed_slice());
                chunk += 1;
                stems.decoded.fetch_add(n, Ordering::AcqRel);
            }
        };
        loop {
            // Feed the stream that is furthest behind, so the queues stay short.
            let next = self
                .inputs
                .iter_mut()
                .filter(|i| !i.done)
                .min_by_key(|i| i.queue.len());
            let Some(input) = next else { break };
            input.step()?;
            // Frames all stems have reached are ready; a stream that ended
            // early plays silence from there.
            loop {
                let all = self.inputs.iter().all(|i| !i.queue.is_empty() || i.done);
                let any = self.inputs.iter().any(|i| !i.queue.is_empty());
                if !(all && any) {
                    break;
                }
                let mut frame = [[0i16; 2]; MAX_STEMS];
                for (k, i) in self.inputs.iter_mut().enumerate() {
                    frame[k] = i.queue.pop_front().unwrap_or_default();
                }
                if let Some(low) = &mut self.split {
                    let drums = frame[0].map(|v| v as f32 / 32_768.0);
                    let (kick, hats) = split(low, drums);
                    frame[0] = kick.map(to_i16);
                    frame[FILE_STEMS] = hats.map(to_i16);
                }
                buf.push(frame);
                if buf.len() == CHUNK {
                    publish(&mut buf);
                    if !keep_going() {
                        return Err("stem decoding cancelled".into());
                    }
                }
            }
        }
        publish(&mut buf);
        self.stems.complete.store(true, Ordering::Release);
        Ok(self.stems)
    }
}

/// Reads an MP4 box header at the reader's position: size and type, with
/// the size counting the header. None at the end of the file.
fn box_header(f: &mut (impl Read + Seek), end: u64) -> Option<(u64, [u8; 4], u64)> {
    let start = f.stream_position().ok()?;
    if start + 8 > end {
        return None;
    }
    let mut h = [0u8; 8];
    f.read_exact(&mut h).ok()?;
    let mut size = u32::from_be_bytes(h[..4].try_into().unwrap()) as u64;
    let kind: [u8; 4] = h[4..].try_into().unwrap();
    let mut header = 8;
    if size == 1 {
        let mut big = [0u8; 8];
        f.read_exact(&mut big).ok()?;
        size = u64::from_be_bytes(big);
        header = 16;
    } else if size == 0 {
        size = end - start;
    }
    (size >= header).then_some((start + size, kind, start + header))
}

/// Finds the box at `path` (e.g. `moov`, `udta`, `stem`) in `f` and returns its
/// contents.
fn find_box(f: &mut (impl Read + Seek), path: &[&[u8; 4]]) -> Option<Vec<u8>> {
    let mut end = f.seek(SeekFrom::End(0)).ok()?;
    f.seek(SeekFrom::Start(0)).ok()?;
    let mut path = path.iter();
    let mut want = path.next()?;
    while let Some((next, kind, body)) = box_header(f, end) {
        if &kind == *want {
            match path.next() {
                Some(w) => {
                    want = w;
                    end = next;
                    f.seek(SeekFrom::Start(body)).ok()?;
                }
                None => {
                    let mut data = vec![0u8; (next - body).min(1 << 20) as usize];
                    f.read_exact(&mut data).ok()?;
                    return Some(data);
                }
            }
        } else {
            f.seek(SeekFrom::Start(next)).ok()?;
        }
    }
    None
}

/// The stem names and colours from an NI STEM file's manifest.
pub fn read_manifest(path: &Path) -> Option<Vec<StemInfo>> {
    let mut f = File::open(path).ok()?;
    let data = find_box(&mut f, &[b"moov", b"udta", b"stem"])?;
    parse_manifest(&data)
}

fn parse_manifest(data: &[u8]) -> Option<Vec<StemInfo>> {
    let json: serde_json::Value = serde_json::from_slice(data).ok()?;
    let stems = json.get("stems")?.as_array()?;
    let info: Vec<StemInfo> = stems
        .iter()
        .take(MAX_STEMS)
        .zip(default_info())
        .map(|(s, default)| StemInfo {
            name: s
                .get("name")
                .and_then(|n| n.as_str())
                .filter(|n| !n.is_empty())
                .map_or(default.name, str::to_owned),
            color: s
                .get("color")
                .and_then(|c| c.as_str())
                .and_then(|c| c.strip_prefix('#'))
                .filter(|c| c.len() == 6)
                .and_then(|c| u32::from_str_radix(c, 16).ok())
                .unwrap_or(default.color),
        })
        .collect();
    (!info.is_empty()).then_some(info)
}

/// How a separation is going.
#[derive(Clone, Debug, PartialEq)]
pub enum Separation {
    Running,
    Done,
    Failed(String),
}

/// Separations started this session, by file.
static JOBS: Mutex<Option<HashMap<PathBuf, Separation>>> = Mutex::new(None);

/// How the separation of `path` is going, if one was started.
pub fn separation(path: &Path) -> Option<Separation> {
    JOBS.lock().unwrap().as_ref()?.get(path).cloned()
}

fn set_job(path: &Path, state: Separation) {
    JOBS.lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(path.to_owned(), state);
}

/// The command that runs Demucs. `OPENDJ_DEMUCS` overrides it; otherwise
/// the first `demucs` program found (see [`search_dirs`]), else the first
/// Python there that has the `demucs` module, else `python3 -m demucs`.
fn demucs_command() -> Command {
    if let Some(cmd) = std::env::var_os("OPENDJ_DEMUCS") {
        return Command::new(cmd);
    }
    // Remembered once found, so later separations skip the search. Not
    // finding it is not remembered: it may be installed while the app runs.
    static FOUND: Mutex<Option<(PathBuf, bool)>> = Mutex::new(None);
    let mut found = FOUND.lock().unwrap();
    if found.is_none() {
        let dirs = search_dirs();
        *found = dirs
            .iter()
            .map(|d| d.join("demucs"))
            .find(|p| p.is_file())
            .map(|p| (p, false))
            .or_else(|| {
                dirs.iter()
                    .map(|d| d.join("python3"))
                    .filter(|p| p.is_file())
                    .find(|p| has_demucs_module(p))
                    .map(|p| (p, true))
            });
    }
    match found.as_ref() {
        Some((program, false)) => Command::new(program),
        Some((python, true)) => {
            let mut c = Command::new(python);
            c.args(["-m", "demucs"]);
            c
        }
        None => {
            let mut c = Command::new("python3");
            c.args(["-m", "demucs"]);
            c
        }
    }
}

/// Where `pip install demucs` may have put Demucs. An app opened from the
/// Finder or a desktop menu doesn't get the PATH set up by the user's shell
/// profile, so this asks the login shell for it and adds the usual Homebrew,
/// python.org, pip `--user`, pyenv and conda folders.
fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    if let Some(path) = login_shell_path() {
        dirs.extend(std::env::split_paths(&path));
    }
    let home = crate::paths::home_dir();
    for d in ["/opt/homebrew/bin", "/usr/local/bin"] {
        dirs.push(d.into());
    }
    let mut globbed = |parent: PathBuf, tail: &str| {
        if let Ok(entries) = std::fs::read_dir(&parent) {
            let mut found: Vec<_> = entries.flatten().map(|e| e.path().join(tail)).collect();
            // Newest version first.
            found.sort();
            dirs.extend(found.into_iter().rev());
        }
    };
    globbed(
        "/Library/Frameworks/Python.framework/Versions".into(),
        "bin",
    );
    if let Some(home) = &home {
        globbed(home.join("Library/Python"), "bin");
        for d in [
            ".local/bin",
            ".pyenv/shims",
            "miniconda3/bin",
            "anaconda3/bin",
            "miniforge3/bin",
        ] {
            dirs.push(home.join(d));
        }
    }
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|d| d.is_dir() && seen.insert(d.clone()));
    dirs
}

/// The PATH the user's shell sets up, read from an interactive login shell.
/// Gives up after a few seconds, in case the profile waits for input.
fn login_shell_path() -> Option<String> {
    let shell = std::env::var_os("SHELL")?;
    let mut child = Command::new(shell)
        .args(["-ilc", "printf '\\n@@PATH@@%s@@PATH@@' \"$PATH\""])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while child.try_wait().ok()?.is_none() {
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let mut out = String::new();
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    let (_, rest) = out.rsplit_once("@@PATH@@")?.0.rsplit_once("@@PATH@@")?;
    Some(rest.to_owned())
}

/// Whether the Python at `python` can import Demucs.
fn has_demucs_module(python: &Path) -> bool {
    Command::new(python)
        .args(["-c", "import demucs"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Separates the file at `path` into high-quality stems in the background
/// (see [`crate::separation::separate_hq`]), leaving FLAC files in the
/// cache where [`find`] looks. `done` is called with the result when it
/// finishes.
pub fn separate(path: &Path, done: impl FnOnce(Result<StemSource, String>) + Send + 'static) {
    if matches!(separation(path), Some(Separation::Running)) {
        return;
    }
    set_job(path, Separation::Running);
    let path = path.to_owned();
    let spawned = std::thread::Builder::new()
        .name("stem-separation".into())
        .spawn({
            let path = path.clone();
            move || {
                // One at a time: each takes the GPU (or every core).
                static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
                let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
                let result = crate::separation::separate_hq(&path);
                set_job(
                    &path,
                    match &result {
                        Ok(_) => Separation::Done,
                        Err(e) => Separation::Failed(e.clone()),
                    },
                );
                done(result);
            }
        });
    if let Err(e) = spawned {
        set_job(&path, Separation::Failed(e.to_string()));
    }
}

/// Separates the file at `path` with the Demucs command, into the cache.
pub(crate) fn run_demucs(path: &Path) -> Result<StemSource, String> {
    let dir = crate::cache::stems_dir(path).ok_or("no cache folder")?;
    let work = dir.with_extension(format!("work{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let out = demucs_command()
        .args(["-n", "htdemucs", "--flac", "-o"])
        .arg(&work)
        .arg(path)
        .output()
        .map_err(|e| format!("could not run Demucs ({e}); install it with `pip install demucs`"))?;
    if !out.status.success() {
        let _ = std::fs::remove_dir_all(&work);
        let err = String::from_utf8_lossy(&out.stderr);
        let last = err.lines().rev().find(|l| !l.trim().is_empty());
        return Err(match last {
            Some(l) if l.contains("No module named demucs") => {
                "Demucs is not installed; install it with `pip install demucs`".into()
            }
            Some(l) => format!("Demucs failed: {}", l.trim()),
            None => format!("Demucs failed ({})", out.status),
        });
    }
    // Demucs writes <out>/htdemucs/<file name without extension>/<stem>.flac.
    let name = path.file_stem().unwrap_or_default();
    let made = work.join("htdemucs").join(name);
    let files = separated_files(&made);
    if !files.iter().all(|f| f.is_file()) {
        let _ = std::fs::remove_dir_all(&work);
        return Err("Demucs finished but left no stems".into());
    }
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::rename(&made, &dir).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&work);
    Ok(StemSource::Files(separated_files(&dir)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::tests::write_wav;

    #[test]
    fn manifest_names_and_colours() {
        let json = br##"{"version":1,"mastering_dsp":{},"stems":[
            {"name":"Kick","color":"#FF0000"},{"name":"Bass","color":"#00FF00"},
            {"name":"","color":"bad"},{"name":"Vox","color":"#0000FF"}]}"##;
        let info = parse_manifest(json).unwrap();
        assert_eq!(info.len(), 4);
        assert_eq!(info[0].name, "Kick");
        assert_eq!(info[0].color, 0xFF0000);
        // Missing values take NI's defaults.
        assert_eq!(info[2].name, "Instru");
        assert_eq!(info[2].color, 0xA26018);
        assert_eq!(info[3].color, 0x0000FF);
    }

    #[test]
    fn finds_the_manifest_box() {
        let manifest = br##"{"stems":[{"name":"A","color":"#010203"}]}"##;
        let mut stem_box = Vec::new();
        stem_box.extend_from_slice(&(8 + manifest.len() as u32).to_be_bytes());
        stem_box.extend_from_slice(b"stem");
        stem_box.extend_from_slice(manifest);
        let wrap = |kind: &[u8; 4], body: &[u8]| {
            let mut v = (8 + body.len() as u32).to_be_bytes().to_vec();
            v.extend_from_slice(kind);
            v.extend_from_slice(body);
            v
        };
        let mut file = wrap(b"ftyp", b"M4A mp42");
        file.extend(wrap(b"mdat", &[0; 32]));
        let mut moov = wrap(b"mvhd", &[0; 20]);
        moov.extend(wrap(b"udta", &stem_box));
        file.extend(wrap(b"moov", &moov));
        let mut cur = std::io::Cursor::new(file);
        let data = find_box(&mut cur, &[b"moov", b"udta", b"stem"]).unwrap();
        let info = parse_manifest(&data).unwrap();
        assert_eq!(info[0].name, "A");
        assert_eq!(info[0].color, 0x010203);
        assert!(find_box(&mut cur, &[b"moov", b"trak"]).is_none());
    }

    #[test]
    fn decodes_and_mixes_separate_stem_files() {
        let dir = std::env::temp_dir().join(format!("opendj-stems-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let files: Vec<PathBuf> = (0..4)
            .map(|k| {
                let p = dir.join(format!("{k}.wav"));
                // Stem k is a constant level (k + 1) / 10 on the left, and its
                // negative on the right; the last stem is shorter.
                let n = if k == 3 { 50_000 } else { 100_000 };
                let v = ((k + 1) as f32 / 10.0 * 32_768.0) as i16;
                let samples: Vec<i16> = (0..n).flat_map(|_| [v, -v]).collect();
                write_wav(&p, 44_100, 2, &samples);
                p
            })
            .collect();
        let d = Decode::open(&StemSource::Files(files)).unwrap();
        // The drums (first) split into kick and hi-hats.
        assert_eq!(d.stems().count(), 5);
        assert_eq!(d.stems().info[0].name, "Kick");
        assert_eq!(d.stems().info[3].name, "Vocal");
        assert_eq!(d.stems().info[4].name, "HiHat");
        let stems = d.run(|| true).unwrap();
        assert!(stems.is_complete());
        assert_eq!(stems.decoded(), 100_000);
        // Separated stems play as the original (here the sum of the
        // stems) less what is turned down; once the split filters settle.
        let original = |i: usize| move || [if i < 50_000 { 1.0 } else { 0.6 }, 0.0];
        let all = stems.mix(20_000, &[1.0; 5], original(20_000));
        assert!((all[0] - 1.0).abs() < 1e-3);
        // The drums are a constant level, so all kick and no hats.
        let kick = stems.mix(20_000, &[1.0, 0.0, 0.0, 0.0, 0.0], original(20_000));
        assert!((kick[0] - 0.1).abs() < 1e-3, "{kick:?}");
        let hats = stems.mix(20_000, &[0.0, 0.0, 0.0, 0.0, 1.0], original(20_000));
        assert!(hats[0].abs() < 1e-3, "{hats:?}");
        // Drums and vocals muted, bass at half.
        let some = stems.mix(20_000, &[0.0, 0.5, 1.0, 0.0, 0.0], original(20_000));
        assert!((some[0] - 0.4).abs() < 1e-3);
        // Past the end of the short stem it is silent.
        let late = stems.mix(60_000, &[0.0, 0.0, 0.0, 1.0, 0.0], original(60_000));
        assert!(late[0].abs() < 1e-3, "{late:?}");
        // Where there are no stems yet, the original plays.
        assert_eq!(stems.mix(500_000, &[0.0; 5], || [0.7, 0.7]), [0.7, 0.7]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn recognises_ni_stem_names() {
        assert!(is_ni_stem(Path::new("/m/Track.stem.mp4")));
        assert!(is_ni_stem(Path::new("/m/Track.STEM.M4A")));
        assert!(!is_ni_stem(Path::new("/m/Track.mp4")));
    }
}
