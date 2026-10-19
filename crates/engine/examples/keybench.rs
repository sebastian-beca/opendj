//! Measures the key detectors on a set of annotated tracks, such as the
//! GiantSteps key dataset (https://github.com/GiantSteps/giantsteps-key-dataset):
//!
//!     cargo run --release -p mixxx-engine --example keybench -- AUDIO_DIR [KEY_DIR]
//!
//! Each audio file's key is read from `KEY_DIR/<file name>.key` (or `.txt`)
//! holding text such as "C minor"; KEY_DIR defaults to AUDIO_DIR. Scores
//! follow MIREX: 1 for the right key, 0.5 a fifth away, 0.3 the relative
//! key, 0.2 the parallel key.
//!
//! `--dump DIR` also writes each file's neural spectrogram (105 little-endian
//! f32 per frame) and scores, to compare with openkeyscan's own Python code.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use mixxx_engine::analysis::detect_key;
use mixxx_engine::decode::decode;
use mixxx_engine::key::{Detector, Key};
use mixxx_engine::keynet::Spectrogram;

#[derive(Default)]
struct Tally {
    n: usize,
    correct: usize,
    fifth: usize,
    relative: usize,
    parallel: usize,
    time: Duration,
}

impl Tally {
    fn add(&mut self, found: Option<Key>, truth: Key, time: Duration) {
        self.n += 1;
        self.time += time;
        let Some(k) = found else { return };
        let step = (k.tonic + 12 - truth.tonic) % 12;
        if k == truth {
            self.correct += 1;
        } else if k.minor == truth.minor && matches!(step, 5 | 7) {
            self.fifth += 1;
        } else if k.minor != truth.minor && k.compatible(truth) {
            // Different letters are compatible only on the same number.
            self.relative += 1;
        } else if k.minor != truth.minor && step == 0 {
            self.parallel += 1;
        }
    }

    fn report(&self, name: &str) {
        let n = self.n.max(1) as f64;
        let pc = |c: usize| 100.0 * c as f64 / n;
        let weighted = pc(self.correct)
            + 0.5 * pc(self.fifth)
            + 0.3 * pc(self.relative)
            + 0.2 * pc(self.parallel);
        println!(
            "{name:9} weighted {weighted:5.1}%  correct {:5.1}%  fifth {:4.1}%  relative {:4.1}%  parallel {:4.1}%  {:.2} s a track",
            pc(self.correct),
            pc(self.fifth),
            pc(self.relative),
            pc(self.parallel),
            self.time.as_secs_f64() / n,
        );
    }
}

fn annotation(dirs: &Path, audio: &Path) -> Option<Key> {
    let name = audio.file_name()?.to_string_lossy();
    let stem = audio.file_stem()?.to_string_lossy();
    for candidate in [
        format!("{stem}.key"),
        format!("{stem}.txt"),
        format!("{name}.key"),
    ] {
        if let Ok(text) = std::fs::read_to_string(dirs.join(candidate)) {
            // "C minor", maybe followed by a confidence column.
            let words: Vec<&str> = text.split_whitespace().take(2).collect();
            return Key::parse(&words.join(" "));
        }
    }
    None
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let dump = args.iter().position(|a| a == "--dump").map(|i| {
        args.remove(i);
        PathBuf::from(args.remove(i))
    });
    let Some(audio_dir) = args.first().map(PathBuf::from) else {
        eprintln!("usage: keybench AUDIO_DIR [KEY_DIR] [--dump DIR]");
        std::process::exit(2);
    };
    let key_dir = args.get(1).map_or(audio_dir.clone(), PathBuf::from);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&audio_dir)
        .expect("readable audio folder")
        .flatten()
        .map(|e| e.path())
        .filter(|p| mixxx_engine::scan::is_audio(p))
        .collect();
    files.sort();
    let (mut standard, mut neural) = (Tally::default(), Tally::default());
    for path in &files {
        let audio = match decode(path) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("{}: {e}", path.display());
                continue;
            }
        };
        let t = Instant::now();
        let s = detect_key(&audio, Detector::Standard);
        let ts = t.elapsed();
        let t = Instant::now();
        let n = detect_key(&audio, Detector::Neural);
        let tn = t.elapsed();
        let truth = annotation(&key_dir, path);
        let show = |k: Option<Key>| k.map_or("-".into(), |k| k.camelot());
        println!(
            "{}\t{}\tstandard {}\tneural {}",
            path.file_name().unwrap().to_string_lossy(),
            show(truth),
            show(s),
            show(n)
        );
        if let Some(truth) = truth {
            standard.add(s, truth, ts);
            neural.add(n, truth, tn);
        }
        if let Some(dir) = &dump {
            let spectrogram = || {
                let mut s = Spectrogram::new(audio.sample_rate as f64);
                for f in audio.decoded_chunks().flatten() {
                    s.push((f[0] + f[1]) * 0.5);
                }
                s
            };
            let frames = spectrogram().frames();
            let bytes: Vec<u8> = frames
                .iter()
                .flatten()
                .flat_map(|v| v.to_le_bytes())
                .collect();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join(format!("{name}.spec")), bytes).unwrap();
            if let Some(scores) = spectrogram().scores() {
                let line: Vec<String> = scores.iter().map(|v| v.to_string()).collect();
                std::fs::write(dir.join(format!("{name}.scores")), line.join(" ")).unwrap();
            }
        }
    }
    if standard.n > 0 {
        println!("\n{} annotated tracks", standard.n);
        standard.report("standard");
        neural.report("neural");
    }
}
