//! Mixed sets: recordings join the Mixed Sets source as soon as they stop,
//! and "Export for Mixcloud" turns one into a 320 kbps MP3 whose ID3 tag
//! carries the tracklist as chapters, with a `.tracklist.txt` beside it.
//!
//! The tracklist comes from the `.cue` sheet the recorder writes next to
//! the recording (each track from when it became audible). Recordings
//! split into parts are joined back into one MP3. The work runs on its own
//! thread; long WAVs are streamed, never held in memory.

use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;

use opendj_broadcast::mixcloud::{self, Mp3File, Section};
use opendj_engine::decode::Decoder;
use opendj_engine::scan::read_track;
use slint::ComponentHandle;

use super::{file_path, Browser, MIXED_SETS};
use crate::{files, AppWindow, FilesUi};

/// What the export thread says.
pub(super) enum Progress {
    Status(String),
    Done(Result<PathBuf, String>),
}

impl Browser {
    pub(super) fn install_mix_export(
        ui: &FilesUi,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        ui.on_export_mixcloud(move |row| {
            w(&|b| {
                let path = b.rows.get(row as usize).copied().flatten().and_then(|i| {
                    let t = &b.library[i];
                    Some((
                        file_path(t)?.to_path_buf(),
                        t.title.clone(),
                        t.artist.clone(),
                    ))
                });
                if let Some((path, title, artist)) = path {
                    b.export_for_mixcloud(path, title, artist);
                }
            })
        });
    }

    /// Adds the files of a recording that just stopped to the library, so
    /// they show in Mixed Sets straight away rather than after the next
    /// scan.
    pub fn add_recordings(&mut self, files: &[PathBuf]) {
        let mut added = false;
        for path in files {
            if self.path_index.contains_key(path) || !path.is_file() {
                continue;
            }
            let mut track = read_track(self.library.len(), path);
            if let Some(db) = &mut self.db {
                match db.add_file(&track) {
                    Ok(t) => track = t,
                    Err(e) => {
                        eprintln!("can't add {} to the library: {e}", path.display());
                        continue;
                    }
                }
            }
            if !self.index_of.contains_key(&track.id) {
                self.push_track(track);
                added = true;
            }
        }
        if !added {
            return;
        }
        self.collection_stale = true;
        if self.tree == MIXED_SETS {
            self.rebuild_folder_tree(MIXED_SETS);
            self.refresh_tree();
            self.refresh_rows();
        }
    }

    fn set_mix_status(&self, text: &str) {
        if let Some(app) = self.app.upgrade() {
            app.global::<FilesUi>().set_mix_status(text.into());
        }
    }

    fn export_for_mixcloud(&mut self, path: PathBuf, title: String, artist: String) {
        if self.mix_export.is_some() {
            return;
        }
        let (tx, rx) = channel();
        self.mix_export = Some(rx);
        self.set_mix_status("Making the Mixcloud MP3…");
        std::thread::Builder::new()
            .name("mixcloud-export".into())
            .spawn(move || {
                let status = |s: String| {
                    let _ = tx.send(Progress::Status(s));
                };
                let result = export(&path, &title, &artist, &status);
                let _ = tx.send(Progress::Done(result));
            })
            .expect("spawn mixcloud export");
    }

    /// Picks up what the export thread said. Called every frame.
    pub fn poll_mix_export(&mut self, app: &AppWindow) {
        let Some(rx) = &self.mix_export else { return };
        let mut done = None;
        let mut status = None;
        while let Ok(p) = rx.try_recv() {
            match p {
                Progress::Status(s) => status = Some(s),
                Progress::Done(r) => done = Some(r),
            }
        }
        if let Some(s) = status {
            app.global::<FilesUi>().set_mix_status(s.into());
        }
        match done {
            Some(Ok(mp3)) => {
                self.mix_export = None;
                let name = mp3.file_name().unwrap_or_default().to_string_lossy();
                self.set_mix_status(&format!(
                    "Made {name}, with its tracklist. Upload it on mixcloud.com/upload."
                ));
                files::reveal(&mp3);
                self.add_recordings(&[mp3]);
            }
            Some(Err(e)) => {
                self.mix_export = None;
                self.set_mix_status(&format!("Couldn't make the Mixcloud MP3: {e}"));
            }
            None => {}
        }
    }
}

/// `dir/name` without its extension and a " (part N)" ending: the name
/// the recorder's `.cue` and `.tracklist.txt` share.
fn recording_base(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let stem = match stem.rfind(" (part ") {
        Some(at) if stem.ends_with(')') => stem[..at].to_string(),
        _ => stem,
    };
    path.with_file_name(stem)
}

fn with_suffix(base: &Path, suffix: &str) -> PathBuf {
    let mut name = base.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    base.with_file_name(name)
}

/// The parts to join and the tracklist: from the `.cue` beside the
/// recording if there is one, else just the file with no tracks.
fn plan(path: &Path) -> (Vec<PathBuf>, Vec<mixcloud::CueFile>) {
    let base = recording_base(path);
    let dir = path.parent().unwrap_or(Path::new(""));
    let cue = std::fs::read_to_string(with_suffix(&base, ".cue"))
        .map(|t| mixcloud::parse_cue(&t))
        .unwrap_or_default();
    let parts: Vec<PathBuf> = cue
        .iter()
        .map(|f| dir.join(&f.name))
        .filter(|p| p.is_file())
        .collect();
    // A cue sheet naming files that aren't there (renamed, say) is no use
    // for timing; keep the file picked.
    if cue.is_empty() || parts.len() != cue.len() || !parts.iter().any(|p| p == path) {
        return (vec![path.to_path_buf()], Vec::new());
    }
    (parts, cue)
}

fn export(
    path: &Path,
    title: &str,
    artist: &str,
    status: &dyn Fn(String),
) -> Result<PathBuf, String> {
    let (parts, cue) = plan(path);
    let decoders = parts
        .iter()
        .map(|p| Decoder::open(p))
        .collect::<Result<Vec<_>, _>>()?;
    let lengths: Vec<f64> = decoders
        .iter()
        .map(|d| d.seconds().unwrap_or(0.0))
        .collect();
    let total: f64 = lengths.iter().sum();
    let sections: Vec<Section> = mixcloud::sections(&cue, &lengths);
    let base = recording_base(&parts[0]);
    let name = if title.trim().is_empty() {
        base.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    } else {
        title.trim().to_string()
    };
    let mp3 = with_suffix(&base, " (Mixcloud).mp3");
    if mp3 == path {
        return Err("this is already a Mixcloud MP3".into());
    }
    let tag = mixcloud::id3_tag(&name, artist.trim(), &sections, total, None);
    let rate = decoders[0].audio().sample_rate;
    let mut out = Mp3File::create(&mp3, rate, &tag).map_err(|e| e.to_string())?;
    let mut done = 0.0;
    let mut last = -1i64;
    for d in decoders {
        let mut failed = None;
        let res = d.stream(|r, samples| {
            if r != rate {
                failed = Some("the parts have different sample rates".to_string());
                return false;
            }
            if let Err(e) = out.write(samples) {
                failed = Some(e.to_string());
                return false;
            }
            done += (samples.len() / 2) as f64 / r as f64;
            if total > 0.0 {
                let pct = (done / total * 100.0).min(99.0) as i64;
                if pct != last {
                    last = pct;
                    status(format!("Making the Mixcloud MP3… {pct}%"));
                }
            }
            true
        });
        if let Some(e) = failed {
            let _ = std::fs::remove_file(&mp3);
            return Err(e);
        }
        if let Err(e) = res {
            let _ = std::fs::remove_file(&mp3);
            return Err(e);
        }
    }
    out.finish().map_err(|e| e.to_string())?;
    if !sections.is_empty() {
        let list = with_suffix(&base, ".tracklist.txt");
        std::fs::write(&list, mixcloud::tracklist_text(&sections))
            .map_err(|e| format!("{}: {e}", list.display()))?;
    }
    Ok(mp3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_share_the_recordings_name() {
        assert_eq!(
            recording_base(Path::new("/s/openDJ 2026-11-03 21.05 (part 2).wav")),
            Path::new("/s/openDJ 2026-11-03 21.05")
        );
        assert_eq!(
            recording_base(Path::new("/s/openDJ 2026-11-03 21.05.flac")),
            Path::new("/s/openDJ 2026-11-03 21.05")
        );
    }

    /// A two-part recording with its cue sheet becomes one MP3 with both
    /// parts' tracks as chapters.
    #[test]
    fn joins_parts_and_times_their_tracks() {
        let dir = std::env::temp_dir().join(format!("opendj-mix-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let rate = 44_100u32;
        let wav = |name: &str, seconds: u32| {
            let frames = rate * seconds;
            let mut b = Vec::new();
            b.extend_from_slice(b"RIFF");
            b.extend_from_slice(&(36 + frames * 4).to_le_bytes());
            b.extend_from_slice(b"WAVEfmt ");
            b.extend_from_slice(&16u32.to_le_bytes());
            b.extend_from_slice(&1u16.to_le_bytes());
            b.extend_from_slice(&2u16.to_le_bytes());
            b.extend_from_slice(&rate.to_le_bytes());
            b.extend_from_slice(&(rate * 4).to_le_bytes());
            b.extend_from_slice(&4u16.to_le_bytes());
            b.extend_from_slice(&16u16.to_le_bytes());
            b.extend_from_slice(b"data");
            b.extend_from_slice(&(frames * 4).to_le_bytes());
            for i in 0..frames {
                let v = ((i as f32 * 0.05).sin() * 8000.0) as i16;
                b.extend_from_slice(&v.to_le_bytes());
                b.extend_from_slice(&v.to_le_bytes());
            }
            std::fs::write(dir.join(name), b).unwrap();
        };
        wav("set.wav", 2);
        wav("set (part 2).wav", 1);
        std::fs::write(
            dir.join("set.cue"),
            "FILE \"set.wav\" WAVE\n  TRACK 01 AUDIO\n    TITLE \"One\"\n    PERFORMER \"A\"\n    INDEX 01 00:00:00\n\
             FILE \"set (part 2).wav\" WAVE\n  TRACK 02 AUDIO\n    TITLE \"Two\"\n    PERFORMER \"B\"\n    INDEX 01 00:00:38\n",
        )
        .unwrap();
        let mp3 = export(&dir.join("set.wav"), "", "DJ", &|_| {}).unwrap();
        assert_eq!(mp3, dir.join("set (Mixcloud).mp3"));
        let list = std::fs::read_to_string(dir.join("set.tracklist.txt")).unwrap();
        assert_eq!(list, "0:00 A - One\n0:02 B - Two\n");
        // Three seconds of audio at 320 kbps.
        let len = std::fs::metadata(&mp3).unwrap().len();
        assert!((100_000..140_000).contains(&len), "{len}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
