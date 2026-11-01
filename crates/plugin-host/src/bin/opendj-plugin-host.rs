//! The helper process. Two modes:
//!
//! - `opendj-plugin-host --shm <path> (--test <id> | --clap <file> --id <id>)`
//!   maps the block openDJ shares, loads one effect, describes it on stdout
//!   (`name`, `latency`, `param` lines, then `ready`) and runs a block each
//!   time openDJ writes a byte to stdin. It exits when stdin closes, so a
//!   helper never outlives openDJ.
//! - `opendj-plugin-host --scan <file>` lists the plugins in a CLAP file
//!   (`plugin` lines, then `done`) and exits.
//!
//! The plugin's main-thread half stays on this process's main thread; the
//! audio runs on a second thread.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use opendj_plugin_host::effects::{self, Effect, Plugin};
use opendj_plugin_host::shm::{self, Shm, CHANNELS, MAX_PARAMS};
use opendj_plugin_host::Transport;

fn main() {
    let mut shm_path = None;
    let mut test = None;
    let mut clap = None;
    let mut id = None;
    let mut scan = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(a) = args.next() {
        match a.to_str() {
            Some("--shm") => shm_path = args.next().map(PathBuf::from),
            Some("--test") => test = args.next().and_then(|s| s.into_string().ok()),
            Some("--clap") => clap = args.next().map(PathBuf::from),
            Some("--id") => id = args.next().and_then(|s| s.into_string().ok()),
            Some("--scan") => scan = args.next().map(PathBuf::from),
            _ => fail(&format!("unknown argument {a:?}")),
        }
    }
    if let Some(file) = scan {
        return run_scan(&file);
    }
    let plugin = match (test, clap, id) {
        (Some(t), None, _) => Plugin::Test(t),
        (None, Some(file), Some(id)) => Plugin::Clap { file, id },
        _ => fail("usage: opendj-plugin-host --shm <path> (--test <id> | --clap <file> --id <id>)"),
    };
    let Some(shm_path) = shm_path else {
        fail("--shm <path> is needed");
    };
    let shm =
        Arc::new(Shm::open(&shm_path).unwrap_or_else(|e| fail(&format!("shared memory: {e}"))));
    let h = shm.header();
    let sample_rate = h.sample_rate.load(Ordering::Relaxed);
    let loaded = effects::load(&plugin, sample_rate, shm.max_frames()).unwrap_or_else(|e| fail(&e));

    // Describe it, and start the shared parameters at its current values,
    // so openDJ changes only what the DJ touches.
    let info = &loaded.info;
    say(&format!("name\t{}", clean(&info.name)));
    say(&format!("latency\t{}", info.latency));
    for (i, p) in info.params.iter().enumerate() {
        shm::store_f32(&h.params[i], p.value as f32);
        say(&format!(
            "param\t{}\t{}\t{}\t{}\t{}",
            p.min,
            p.max,
            p.default,
            p.value,
            clean(&p.name)
        ));
    }
    let mut main = loaded.main;
    let effect = loaded.effect;
    let audio_shm = shm.clone();
    let audio = std::thread::Builder::new()
        .name("plugin-audio".into())
        .spawn(move || run_audio(&audio_shm, effect))
        .unwrap_or_else(|e| fail(&format!("audio thread: {e}")));
    say("ready");

    // Main thread: serve the plugin's main-thread requests until openDJ
    // lets go. Exiting without tearing the plugin down is deliberate: a
    // plugin can't crash on the way out if it is never asked to clean up.
    while !audio.is_finished() {
        if let Some(m) = main.as_mut() {
            m.idle();
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    std::process::exit(0);
}

fn run_audio(shm: &Shm, mut fx: Box<dyn Effect>) {
    let h = shm.header();
    let max = shm.max_frames();
    // All buffers up front: nothing allocates per block.
    let mut input = vec![vec![0.0f32; max]; CHANNELS];
    let mut output = vec![vec![0.0f32; max]; CHANNELS];
    let mut params = [0.0f32; MAX_PARAMS];
    let mut stdin = std::io::stdin().lock();
    let mut wake = [0u8; 64];
    loop {
        match stdin.read(&mut wake) {
            Ok(0) | Err(_) => return, // openDJ closed the pipe or quit
            Ok(_) => {}
        }
        let seq = h.seq_in.load(Ordering::Acquire);
        if seq == h.seq_out.load(Ordering::Relaxed) {
            continue;
        }
        let start = Instant::now();
        let frames = (h.frames.load(Ordering::Relaxed) as usize).min(max);
        for (ch, buf) in input.iter_mut().enumerate() {
            shm.read(false, ch, &mut buf[..frames]);
        }
        for (p, a) in params.iter_mut().zip(&h.params) {
            *p = shm::load_f32(a);
        }
        let transport = Transport {
            bpm: shm::load_f64(&h.bpm),
            beat: shm::load_f64(&h.beat),
            playing: h.playing.load(Ordering::Relaxed) != 0,
        };
        {
            let [l, r] = &mut output[..] else {
                unreachable!()
            };
            fx.process(
                [&input[0][..frames], &input[1][..frames]],
                [&mut l[..frames], &mut r[..frames]],
                &params,
                &transport,
            );
        }
        for (ch, buf) in output.iter().enumerate() {
            shm.write(true, ch, &buf[..frames]);
        }
        h.process_ns
            .store(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
        h.seq_out.store(seq, Ordering::Release);
    }
}

fn run_scan(file: &std::path::Path) {
    match opendj_plugin_host::clap::scan(file) {
        Ok(found) => {
            for d in found {
                say(&format!(
                    "plugin\t{}\t{}\t{}\t{}\t{}",
                    clean(&d.id),
                    clean(&d.name),
                    clean(&d.vendor),
                    clean(&d.version),
                    clean(&d.features.join(","))
                ));
            }
            say("done");
        }
        Err(e) => fail(&e),
    }
}

/// Tabs and newlines would break the line format.
fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

fn say(line: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

fn fail(msg: &str) -> ! {
    say(&format!("error {}", clean(msg)));
    std::process::exit(2);
}
