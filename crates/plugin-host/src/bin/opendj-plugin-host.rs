//! The helper process: maps the block openDJ shares, loads one effect, and
//! runs a block each time openDJ writes a byte to stdin. It exits when stdin
//! closes, so a helper never outlives openDJ.
//!
//! Usage: `opendj-plugin-host --shm <path> --effect <id>`

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Instant;

use opendj_plugin_host::shm::{self, Shm, CHANNELS, MAX_PARAMS};
use opendj_plugin_host::{effects, Transport};

fn main() {
    let mut shm_path = None;
    let mut effect = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--shm" => shm_path = args.next().map(PathBuf::from),
            "--effect" => effect = args.next(),
            _ => fail(&format!("unknown argument {a:?}")),
        }
    }
    let (Some(shm_path), Some(effect)) = (shm_path, effect) else {
        fail("usage: opendj-plugin-host --shm <path> --effect <id>");
    };
    let shm = Shm::open(&shm_path).unwrap_or_else(|e| fail(&format!("shared memory: {e}")));
    let h = shm.header();
    let sample_rate = h.sample_rate.load(Ordering::Relaxed);
    let mut fx = effects::load(&effect, sample_rate).unwrap_or_else(|e| fail(&e));
    let max = shm.max_frames();
    // All buffers up front: nothing allocates per block.
    let mut input = vec![vec![0.0f32; max]; CHANNELS];
    let mut output = vec![vec![0.0f32; max]; CHANNELS];
    let mut params = [0.0f32; MAX_PARAMS];
    say("ready");

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

fn say(line: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

fn fail(msg: &str) -> ! {
    say(&format!("error {msg}"));
    std::process::exit(2);
}
