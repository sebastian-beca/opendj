//! Where a track has vocals: a level along the track, from Spleeter's
//! vocal network on its own (see [`spleeter::vocal_levels`]), worked out
//! once and kept in the cache beside the analysis. The deck waveforms tint
//! the sung parts with it (Settings › Decks › Show vocals).

use std::fs;
use std::path::{Path, PathBuf};

use opendj_separation::spleeter;

use crate::decode::decode;
use crate::separation::{channels, spleeter_net};
use crate::track::Audio;

/// Levels per second.
pub const RATE: f64 = spleeter::VOCAL_RATE;

const MAGIC: &[u8; 8] = b"ODJVOC01";

/// A track's vocal level along it, [`RATE`] a second, 0 (none) to 255
/// (all vocals).
pub type Levels = Vec<u8>;

/// The vocal levels of decoded `audio`. Stops early, with an error, when
/// `keep_going` turns false.
pub fn detect(audio: &Audio, mut keep_going: impl FnMut() -> bool) -> Result<Levels, String> {
    let net = spleeter_net()?;
    let [l, r] = channels(audio);
    let model_rate = opendj_separation::SAMPLE_RATE;
    let (l, r) = if audio.sample_rate == model_rate {
        (l, r)
    } else {
        (
            opendj_separation::resample(&l, audio.sample_rate, model_rate)?,
            opendj_separation::resample(&r, audio.sample_rate, model_rate)?,
        )
    };
    let mut stopped = false;
    let levels = spleeter::vocal_levels(&*net, [&l, &r], || {
        stopped |= !keep_going();
        !stopped
    });
    if stopped {
        return Err("stopped".into());
    }
    Ok(to_levels(&levels))
}

/// Levels from 0..1 shares. A share under a fifth is mostly what leaks
/// from other instruments, so it reads as none; from there up to two
/// thirds it rises to full.
fn to_levels(shares: &[f32]) -> Levels {
    shares
        .iter()
        .map(|s| (((s - 0.2) / 0.45).clamp(0.0, 1.0) * 255.0).round() as u8)
        .collect()
}

/// Where the vocal levels of `path` are kept, beside its analysis.
fn file(path: &Path) -> Option<PathBuf> {
    let entry = crate::cache::entry_path(path)?;
    Some(
        entry
            .parent()?
            .parent()?
            .join("vocals")
            .join(entry.file_name()?),
    )
}

/// The kept vocal levels of `path`, if it has them for the file as it is.
pub fn cached(path: &Path) -> Option<Levels> {
    let data = fs::read(file(path)?).ok()?;
    let levels = data.strip_prefix(MAGIC)?;
    (!levels.is_empty()).then(|| levels.to_vec())
}

fn store(path: &Path, levels: &[u8]) {
    let Some(file) = file(path) else { return };
    let write = || -> std::io::Result<()> {
        fs::create_dir_all(file.parent().unwrap())?;
        let tmp = file.with_extension(format!("tmp{}", std::process::id()));
        let mut data = MAGIC.to_vec();
        data.extend_from_slice(levels);
        fs::write(&tmp, &data)?;
        fs::rename(&tmp, &file)
    };
    if let Err(e) = write() {
        eprintln!("could not keep vocals {}: {e}", file.display());
    }
}

/// The vocal levels of the file at `path`: kept ones, or found now (which
/// decodes it and takes a few seconds) and kept. `audio` saves decoding
/// when the caller has it already.
pub fn analyse(
    path: &Path,
    audio: Option<&Audio>,
    keep_going: impl FnMut() -> bool,
) -> Result<Levels, String> {
    if let Some(l) = cached(path) {
        return Ok(l);
    }
    let decoded;
    let audio = match audio {
        Some(a) => a,
        None => {
            decoded = decode(path)?;
            &decoded
        }
    };
    let levels = detect(audio, keep_going)?;
    if !levels.is_empty() {
        store(path, &levels);
    }
    Ok(levels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leakage_reads_as_none_and_voices_as_full() {
        assert_eq!(
            to_levels(&[0.0, 0.15, 0.2, 0.65, 0.9]),
            vec![0, 0, 0, 255, 255]
        );
        let mid = to_levels(&[0.425])[0];
        assert!((120..=135).contains(&mid), "{mid}");
    }
}
