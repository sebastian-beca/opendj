//! Loads a real CLAP binary (the `opendj-test-clap` gain in
//! `crates/plugin-host/test-plugin`) through the helper.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use opendj_plugin_host::{scan, spawn, Options, Outcome, Plugin, ScanError, Transport};

const FRAMES: usize = 256;
const DEADLINE: Duration = Duration::from_millis(500);
const ID: &str = "app.opendj.test-gain";

fn helper() -> PathBuf {
    env!("CARGO_BIN_EXE_opendj-plugin-host").into()
}

/// Builds the test plugin once, next to this test's own binaries.
fn test_plugin() -> PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let target = helper().parent().unwrap().parent().unwrap().to_path_buf();
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let ok = std::process::Command::new(cargo)
            .args(["build", "-q", "-p", "opendj-test-clap", "--target-dir"])
            .arg(&target)
            .status()
            .expect("cargo runs")
            .success();
        assert!(ok, "the test plugin builds");
        let name = if cfg!(windows) {
            "opendj_test_clap.dll"
        } else if cfg!(target_os = "macos") {
            "libopendj_test_clap.dylib"
        } else {
            "libopendj_test_clap.so"
        };
        target.join("debug").join(name)
    })
    .clone()
}

fn start() -> (opendj_plugin_host::Connection, opendj_plugin_host::Helper) {
    spawn(&Options {
        helper: helper(),
        plugin: Plugin::Clap {
            file: test_plugin(),
            id: ID.into(),
        },
        sample_rate: 48_000,
        max_frames: 1024,
        title: None,
    })
    .expect("the CLAP plugin loads")
}

fn run(
    conn: &mut opendj_plugin_host::Connection,
    transport: Transport,
) -> (Outcome, Vec<f32>, Vec<f32>) {
    let l: Vec<f32> = (0..FRAMES)
        .map(|i| 0.1 + (i as f32 / 10.0).sin() * 0.5)
        .collect();
    let r: Vec<f32> = l.iter().map(|x| -x).collect();
    let mut ol = vec![0.0; FRAMES];
    let mut or = vec![0.0; FRAMES];
    let out = conn.process([&l, &r], [&mut ol, &mut or], transport, DEADLINE);
    (out, ol, l)
}

#[test]
fn scanning_finds_the_effect() {
    let found = scan(&helper(), &test_plugin(), Duration::from_secs(10)).expect("scans");
    assert_eq!(found.len(), 1);
    let d = &found[0];
    assert_eq!(d.id, ID);
    assert_eq!(d.name, "openDJ Test Gain");
    assert_eq!(d.vendor, "openDJ");
    assert!(d.is_audio_effect(), "{:?}", d.features);
}

#[test]
fn scanning_a_broken_file_fails_cleanly() {
    let dir = std::env::temp_dir().join(format!("opendj-scan-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bad = dir.join("not-a-plugin.clap");
    std::fs::write(&bad, b"this is not a library").unwrap();
    let err = scan(&helper(), &bad, Duration::from_secs(10)).unwrap_err();
    assert!(matches!(err, ScanError::Failed(_)), "{err:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_plugin_describes_itself() {
    let (_conn, helper) = start();
    let info = helper.info();
    assert_eq!(info.name, "openDJ Test Gain");
    assert_eq!(info.latency, 64);
    assert_eq!(info.params.len(), 2);
    assert_eq!(info.params[0].name, "Gain");
    assert_eq!((info.params[0].min, info.params[0].max), (0.0, 2.0));
    assert_eq!(info.params[0].value, 1.0);
}

#[test]
fn audio_and_parameters_go_through_the_plugin() {
    let (mut conn, _helper) = start();
    // Gain starts at 1: the plugin passes the audio as it is.
    let (out, got, input) = run(&mut conn, Transport::default());
    assert_eq!(out, Outcome::Wet);
    assert_eq!(got, input);
    // Half gain, sent as a parameter event.
    conn.set_param(0, 0.5);
    let (out, got, input) = run(&mut conn, Transport::default());
    assert_eq!(out, Outcome::Wet);
    assert!(got
        .iter()
        .zip(&input)
        .all(|(y, x)| (y - x * 0.5).abs() < 1e-6));
}

#[test]
fn the_deck_tempo_reaches_the_plugin() {
    let (mut conn, _helper) = start();
    conn.set_param(1, 1.0); // the test plugin writes the tempo it sees into sample 0
    let t = Transport {
        bpm: 124.0,
        beat: 17.5,
        playing: true,
    };
    let (out, got, _) = run(&mut conn, t);
    assert_eq!(out, Outcome::Wet);
    assert_eq!(got[0], 124.0);
}

#[test]
fn a_missing_plugin_id_fails_to_start() {
    let err = spawn(&Options {
        helper: helper(),
        plugin: Plugin::Clap {
            file: test_plugin(),
            id: "no.such.plugin".into(),
        },
        sample_rate: 48_000,
        max_frames: 1024,
        title: None,
    })
    .err()
    .expect("refused");
    assert!(err.to_string().contains("has no plugin"), "{err}");
}

#[test]
fn notes_reach_the_plugin() {
    use opendj_plugin_host::Midi;
    let (mut conn, _helper) = start();
    conn.set_param(1, 1.0); // the test plugin writes the first note's key into sample 1
    let silent = vec![0.0f32; FRAMES];
    let mut ol = vec![0.0; FRAMES];
    let mut or = vec![0.0; FRAMES];
    let out = conn.process_events(
        [&silent, &silent],
        [&mut ol, &mut or],
        Transport::default(),
        &[Midi::note_on(10, 0, 60, 100), Midi::note_off(20, 0, 60)],
        DEADLINE,
    );
    assert_eq!(out, Outcome::Wet);
    assert_eq!(ol[1], 60.0);
    // No notes, no key.
    let out = conn.process_events(
        [&silent, &silent],
        [&mut ol, &mut or],
        Transport::default(),
        &[],
        DEADLINE,
    );
    assert_eq!(out, Outcome::Wet);
    assert_eq!(ol[1], -1.0);
}
