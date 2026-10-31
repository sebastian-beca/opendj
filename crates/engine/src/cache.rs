//! Keeps each file's analysis (waveform, tempo, first beat) on disk, as Mixxx
//! does in its library, so a track loaded again shows its waveform at once.
//!
//! Entries live in the user's cache folder (`~/Library/Caches/openDJ` on
//! macOS, `%LOCALAPPDATA%\openDJ` on Windows, `$XDG_CACHE_HOME/opendj` or
//! `~/.cache/opendj` elsewhere, or `OPENDJ_CACHE`), named by a hash of the file's path, size and
//! modification time, so an edited file is analysed again.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::analysis::Analysis;
use crate::beats::{BeatGrid, GridMarker};
use crate::key::{Detector, Key};
use crate::loudness::Loudness;
use crate::track::{OverviewSlice, Waveform, WaveformBin};

/// Changes whenever the analysis or this format changes, so old entries are ignored.
const MAGIC: &[u8; 8] = b"MXRSAN05";

/// Starts each stored overview, followed by the analysis's own [`MAGIC`],
/// so overviews of an older analysis are made again too.
const OVERVIEW_MAGIC: &[u8; 8] = b"ODJOVW01";

/// Slices in a stored overview: the library's mini waveforms.
pub const OVERVIEW_SLICES: usize = 256;

/// Where tests keep their cache, instead of the user's.
#[cfg(test)]
pub(crate) static TEST_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// openDJ's cache folder (`OPENDJ_CACHE` overrides it).
pub fn cache_dir() -> Option<PathBuf> {
    #[cfg(test)]
    if let Some(dir) = TEST_DIR.get() {
        return Some(dir.clone());
    }
    if let Some(dir) = std::env::var_os("OPENDJ_CACHE") {
        return Some(PathBuf::from(dir));
    }
    let base = crate::paths::cache_base()?;
    let name = crate::paths::app_folder_name();
    static MOVED: std::sync::Once = std::sync::Once::new();
    MOVED.call_once(|| move_old_folder(&base, name));
    Some(base.join(name))
}

/// Renames the folder an earlier version kept its files in (`mixxx-rs`)
/// to `name` inside `base`, unless `name` is there already.
pub fn move_old_folder(base: &Path, name: &str) {
    let (old, new) = (base.join("mixxx-rs"), base.join(name));
    if old.is_dir() && !new.exists() {
        if let Err(e) = fs::rename(&old, &new) {
            eprintln!("can't move {} to {}: {e}", old.display(), new.display());
        }
    }
}

/// Where downloaded models are kept (see [`crate::separation::models_dir`]).
pub(crate) fn models_dir() -> Option<PathBuf> {
    cache_dir().map(|d| d.join("models"))
}

/// The folder of stored analyses (waveforms, tempo and key per file).
pub fn analysis_dir() -> Option<PathBuf> {
    cache_dir().map(|d| d.join("analysis"))
}

/// How many stored analyses there are and the bytes they take.
pub fn analysis_usage() -> (usize, u64) {
    let Some(entries) = analysis_dir().and_then(|d| fs::read_dir(d).ok()) else {
        return (0, 0);
    };
    entries
        .flatten()
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .fold((0, 0), |(n, bytes), m| (n + 1, bytes + m.len()))
}

/// Deletes every stored analysis; tracks are analysed again when next
/// loaded. Separated stems are kept. Returns how many were deleted.
pub fn clear_analysis() -> usize {
    // Their overviews go too, so the library shows them as not analysed.
    if let Some(dir) = cache_dir() {
        let _ = fs::remove_dir_all(dir.join("overviews"));
    }
    let Some(entries) = analysis_dir().and_then(|d| fs::read_dir(d).ok()) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|e| fs::remove_file(e.path()).is_ok())
        .count()
}

/// Where `path`'s analysis is kept, or None if the file can't be read.
fn entry(path: &Path) -> Option<PathBuf> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    // FNV-1a: stable across Rust versions, unlike the standard hasher.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let path_bytes = path.as_os_str().as_encoded_bytes();
    for b in path_bytes
        .iter()
        .chain(&meta.len().to_le_bytes())
        .chain(&modified.to_le_bytes())
    {
        h = (h ^ *b as u64).wrapping_mul(0x0100_0000_01b3);
    }
    Some(analysis_dir()?.join(format!("{h:016x}")))
}

/// Where `path`'s analysis is kept now, to hand to [`tags_rewritten`]
/// after rewriting its tags.
pub(crate) fn before_retag(path: &Path) -> Option<PathBuf> {
    entry(path)
}

/// Moves the analysis and separated stems of `path` to where they belong
/// now that its tags were rewritten (which changes its size and time), so
/// neither is worked out again. `old` is what [`before_retag`] gave.
pub(crate) fn tags_rewritten(path: &Path, old: Option<PathBuf>) {
    let (Some(old), Some(new)) = (old, entry(path)) else {
        return;
    };
    if old == new {
        return;
    }
    let _ = fs::rename(&old, &new);
    let stems = |entry: &Path| -> Option<PathBuf> {
        Some(
            entry
                .parent()?
                .parent()?
                .join("stems")
                .join(entry.file_name()?),
        )
    };
    for (from, to) in [
        (stems(&old), stems(&new)),
        (overview_file(&old), overview_file(&new)),
    ] {
        if let (Some(from), Some(to)) = (from, to) {
            if from.exists() {
                let _ = fs::rename(from, to);
            }
        }
    }
}

/// Where `path`'s analysis is kept now, to hand to [`file_moved`] once the
/// file has moved.
pub fn before_move(path: &Path) -> Option<PathBuf> {
    entry(path)
}

/// Moves the analysis and stems of a file that moved to `path` (the cache
/// is keyed by path), so neither is worked out again. `old` is what
/// [`before_move`] gave for its old path.
pub fn file_moved(path: &Path, old: Option<PathBuf>) {
    tags_rewritten(path, old)
}

/// Where the stems separated from `path` are kept.
pub(crate) fn stems_dir(path: &Path) -> Option<PathBuf> {
    let analysis = entry(path)?;
    let name = analysis.file_name()?.to_owned();
    Some(analysis.parent()?.parent()?.join("stems").join(name))
}

/// Where the overview of the analysis kept at `entry` is kept: a small
/// file beside the analyses, so the library can draw a whole list of
/// mini waveforms without reading every track's full waveform.
fn overview_file(entry: &Path) -> Option<PathBuf> {
    Some(
        entry
            .parent()?
            .parent()?
            .join("overviews")
            .join(entry.file_name()?),
    )
}

/// The overview of `path` (see [`Waveform::overview`]), if it has been
/// analysed as the file is now. Made from the stored analysis and kept
/// the first time it is asked for, so this may take a moment once.
pub fn overview(path: &Path) -> Option<Vec<OverviewSlice>> {
    let file = overview_file(&entry(path)?)?;
    if let Some(o) = fs::read(&file).ok().and_then(|d| decode_overview(&d)) {
        return Some(o);
    }
    let o = load(path)?.waveform.overview(OVERVIEW_SLICES);
    store_overview(&file, &o);
    Some(o)
}

fn store_overview(file: &Path, o: &[OverviewSlice]) {
    if o.is_empty() {
        return;
    }
    let mut data = Vec::with_capacity(16 + o.len() * 2);
    data.extend_from_slice(OVERVIEW_MAGIC);
    data.extend_from_slice(MAGIC);
    data.extend(o.iter().flatten());
    let write = || -> std::io::Result<()> {
        fs::create_dir_all(file.parent().unwrap())?;
        let tmp = file.with_extension(format!("tmp{}", std::process::id()));
        fs::write(&tmp, &data)?;
        fs::rename(&tmp, file)
    };
    if let Err(e) = write() {
        eprintln!("could not keep overview {}: {e}", file.display());
    }
}

fn decode_overview(data: &[u8]) -> Option<Vec<OverviewSlice>> {
    let rest = data.strip_prefix(OVERVIEW_MAGIC)?.strip_prefix(MAGIC)?;
    let (slices, tail) = rest.as_chunks::<2>();
    (tail.is_empty() && !slices.is_empty()).then(|| slices.to_vec())
}

/// The stored analysis of `path`, if it has one for the file as it is now.
pub fn load(path: &Path) -> Option<Analysis> {
    let file = fs::File::open(entry(path)?).ok()?;
    let mut data = Vec::new();
    ZlibDecoder::new(file).read_to_end(&mut data).ok()?;
    decode(&data)
}

/// Stores `analysis` of `path`. Failures are ignored: the cache only saves time.
pub fn store(path: &Path, analysis: &Analysis) {
    let Some(entry) = entry(path) else { return };
    let write = || -> std::io::Result<()> {
        fs::create_dir_all(entry.parent().unwrap())?;
        // Write aside and rename, so a reader never sees half an entry.
        let tmp = entry.with_extension(format!("tmp{}", std::process::id()));
        let mut z = ZlibEncoder::new(fs::File::create(&tmp)?, Compression::fast());
        z.write_all(&encode(analysis))?;
        z.finish()?;
        fs::rename(&tmp, &entry)
    };
    if let Err(e) = write() {
        eprintln!("could not cache analysis of {}: {e}", path.display());
        return;
    }
    if let Some(file) = overview_file(&entry) {
        store_overview(&file, &analysis.waveform.overview(OVERVIEW_SLICES));
    }
}

fn encode(a: &Analysis) -> Vec<u8> {
    let finest = a.waveform.levels.first().map_or(&[][..], |l| &l[..]);
    let mut out = Vec::with_capacity(40 + finest.len() * 8);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&a.bpm.to_le_bytes());
    out.extend_from_slice(&a.first_beat.to_le_bytes());
    out.extend_from_slice(&a.waveform.bins_per_second.to_le_bytes());
    // The key's index (0xff for none), and above it the detector's number;
    // entries from before there was a choice of detector say u64::MAX.
    let key = a.key.map_or(0xff, |k| k.index() as u64) | (a.key_detector.id() as u64) << 8;
    out.extend_from_slice(&key.to_le_bytes());
    let (lufs, peak) = a
        .loudness
        .map_or((f64::NAN, f64::NAN), |l| (l.lufs as f64, l.peak as f64));
    out.extend_from_slice(&lufs.to_le_bytes());
    out.extend_from_slice(&peak.to_le_bytes());
    let markers = a.grid.markers();
    out.extend_from_slice(&(markers.len() as u64).to_le_bytes());
    for m in markers {
        out.extend_from_slice(&m.seconds.to_le_bytes());
        out.extend_from_slice(&m.bpm.to_le_bytes());
    }
    // Grid flags: unsure, then from Beat This!.
    let flags = u64::from(a.grid_unsure) | u64::from(a.neural_beats) << 1;
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&(finest.len() as u64).to_le_bytes());
    for b in finest {
        out.extend_from_slice(b);
    }
    out
}

fn decode(data: &[u8]) -> Option<Analysis> {
    let (magic, rest) = data.split_first_chunk::<8>()?;
    if magic != MAGIC {
        return None;
    }
    let mut rest = rest;
    let mut num = || -> Option<[u8; 8]> {
        let (n, r) = rest.split_first_chunk::<8>()?;
        rest = r;
        Some(*n)
    };
    let bpm = f64::from_le_bytes(num()?);
    let first_beat = f64::from_le_bytes(num()?);
    let bins_per_second = f64::from_le_bytes(num()?);
    let (key, key_detector) = match u64::from_le_bytes(num()?) {
        u64::MAX => (None, Detector::Standard),
        v => (
            Key::from_index((v & 0xff) as u8),
            Detector::from_id((v >> 8) as u8)?,
        ),
    };
    let lufs = f64::from_le_bytes(num()?);
    let peak = f64::from_le_bytes(num()?);
    let loudness = (lufs.is_finite() && peak.is_finite()).then_some(Loudness {
        lufs: lufs as f32,
        peak: peak as f32,
    });
    let markers = u64::from_le_bytes(num()?) as usize;
    let mut grid = Vec::with_capacity(markers.min(4096));
    for _ in 0..markers {
        grid.push(GridMarker {
            seconds: f64::from_le_bytes(num()?),
            bpm: f64::from_le_bytes(num()?),
        });
    }
    let flags = u64::from_le_bytes(num()?);
    let len = u64::from_le_bytes(num()?) as usize;
    let finest: Vec<WaveformBin> = rest.as_chunks::<8>().0.to_vec();
    (finest.len() == len && len > 0).then(|| Analysis {
        waveform: Waveform::from_finest(bins_per_second, finest),
        key,
        key_detector,
        bpm,
        first_beat,
        grid: BeatGrid::new(grid),
        grid_unsure: flags & 1 != 0,
        neural_beats: flags & 2 != 0,
        loudness,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analyse;
    use crate::synth;

    #[test]
    fn round_trips_and_notices_changed_files() {
        let dir = std::env::temp_dir().join(format!("opendj-cache-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let track = dir.join("track.wav");
        fs::write(&track, b"first version").unwrap();
        let mut a = analyse(&synth::render_with(124.0, 0.2, 20.0, 1), None);

        let data = encode(&a);
        let b = decode(&data).unwrap();
        assert_eq!((b.bpm, b.first_beat, b.key), (a.bpm, a.first_beat, a.key));
        assert!(a.loudness.is_some());
        assert_eq!(b.loudness, a.loudness);
        assert_eq!(b.waveform.levels, a.waveform.levels);
        assert_eq!(b.grid, a.grid);
        assert!(decode(&data[..data.len() - 3]).is_none());

        // Which detector found the key, and keys from before there was a choice.
        a.key_detector = Detector::Neural;
        let b = decode(&encode(&a)).unwrap();
        assert_eq!((b.key, b.key_detector), (a.key, Detector::Neural));
        let mut old = encode(&a);
        old[32..40].copy_from_slice(&u64::MAX.to_le_bytes());
        let b = decode(&old).unwrap();
        assert_eq!((b.key, b.key_detector), (None, Detector::Standard));

        // Its overview: peaks and RMS across the track, kept small.
        let o = a.waveform.overview(OVERVIEW_SLICES);
        assert_eq!(o.len(), OVERVIEW_SLICES);
        assert!(o.iter().all(|[peak, rms]| rms <= peak));
        let mut kept = OVERVIEW_MAGIC.to_vec();
        kept.extend_from_slice(MAGIC);
        kept.extend(o.iter().flatten());
        assert_eq!(decode_overview(&kept).unwrap(), o);
        assert!(decode_overview(&kept[..kept.len() - 1]).is_none());
        assert!(Waveform::default().overview(8).is_empty());

        let first = entry(&track).unwrap();
        fs::write(&track, b"second, longer version").unwrap();
        assert_ne!(entry(&track).unwrap(), first);
        fs::remove_dir_all(&dir).unwrap();
    }
}
