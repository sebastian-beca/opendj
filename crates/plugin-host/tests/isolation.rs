//! Runs the real helper binary with the test effects: a plugin that crashes,
//! hangs or runs slow must leave the audio dry, never silent or stuck.

use std::time::{Duration, Instant};

use opendj_plugin_host::{spawn, Action, Connection, Helper, Options, Outcome, Policy, Status};

const FRAMES: usize = 256;
/// Generous, so a busy CI machine doesn't turn a good block late.
const DEADLINE: Duration = Duration::from_millis(500);

fn start(effect: &str) -> (Connection, Helper) {
    spawn(&Options {
        helper: env!("CARGO_BIN_EXE_opendj-plugin-host").into(),
        plugin: opendj_plugin_host::Plugin::Test(effect.into()),
        sample_rate: 48_000,
        max_frames: 1024,
    })
    .expect("helper starts")
}

fn block(conn: &mut Connection, deadline: Duration) -> (Outcome, Vec<f32>, Vec<f32>) {
    let l: Vec<f32> = (0..FRAMES).map(|i| (i as f32 / 10.0).sin()).collect();
    let r: Vec<f32> = l.iter().map(|x| -x).collect();
    let mut ol = vec![9.0; FRAMES];
    let mut or = vec![9.0; FRAMES];
    let out = conn.process([&l, &r], [&mut ol, &mut or], Default::default(), deadline);
    let dry = ol == l && or == r;
    let expect_dry = !out.is_wet();
    assert!(
        !expect_dry || dry,
        "a {out:?} block must be the dry input exactly"
    );
    (out, ol, l)
}

#[test]
fn the_plugin_processes_the_audio() {
    let (mut conn, _helper) = start("test-gain");
    conn.set_param(0, 0.5);
    for _ in 0..20 {
        let (out, got, input) = block(&mut conn, DEADLINE);
        assert_eq!(out, Outcome::Wet);
        assert!(got
            .iter()
            .zip(&input)
            .all(|(y, x)| (y - x * 0.5).abs() < 1e-6));
    }
}

#[test]
fn a_crash_leaves_the_audio_dry_and_is_seen() {
    let (mut conn, mut helper) = start("test-crash");
    conn.set_param(0, 3.0); // crash on the fourth block
    for _ in 0..3 {
        assert_eq!(block(&mut conn, DEADLINE).0, Outcome::Wet);
    }
    let (out, ..) = block(&mut conn, Duration::from_millis(50));
    assert_eq!(out, Outcome::Late);
    assert!(matches!(
        helper.wait_exit(Duration::from_secs(5)),
        Status::Exited(_)
    ));
    // Keep going: every block after the crash is dry, none blocks.
    for _ in 0..10 {
        let (out, ..) = block(&mut conn, Duration::from_millis(5));
        assert_ne!(out, Outcome::Wet);
    }
    let mut policy = Policy::default();
    assert_eq!(policy.on_crash(), Action::Restart);
}

#[test]
fn a_hang_is_cut_off_at_the_deadline() {
    let (mut conn, mut helper) = start("test-hang");
    assert_eq!(block(&mut conn, DEADLINE).0, Outcome::Wet);
    conn.set_param(0, 1.0);
    let t = Instant::now();
    assert_eq!(block(&mut conn, Duration::from_millis(5)).0, Outcome::Late);
    assert!(t.elapsed() < Duration::from_millis(200), "waited too long");
    let mut policy = Policy::default();
    let mut action = Action::Keep;
    for _ in 0..10 {
        let t = Instant::now();
        assert_eq!(block(&mut conn, Duration::from_millis(5)).0, Outcome::Busy);
        assert!(
            t.elapsed() < Duration::from_millis(50),
            "a busy block must not wait"
        );
        action = policy.on_missed(conn.missed_in_row());
        if action != Action::Keep {
            break;
        }
    }
    assert_eq!(action, Action::Restart);
    helper.kill();
    assert!(matches!(helper.status(), Status::Exited(_)));
}

#[test]
fn a_slow_block_plays_dry_and_the_next_one_recovers() {
    let (mut conn, helper) = start("test-slow");
    conn.set_param(0, 100.0); // 100 ms a block
    assert_eq!(block(&mut conn, Duration::from_millis(5)).0, Outcome::Late);
    conn.set_param(0, 0.0);
    // Wait until the slow block is done, then the plugin is back.
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(block(&mut conn, DEADLINE).0, Outcome::Wet);
    assert_eq!(conn.missed_in_row(), 0);
    assert!(helper.load() >= 0.0);
}

#[test]
fn the_helper_quits_when_opendj_lets_go() {
    let (conn, mut helper) = start("test-gain");
    drop(conn); // closes the helper's stdin, as when openDJ quits
    assert!(matches!(
        helper.wait_exit(Duration::from_secs(5)),
        Status::Exited(_)
    ));
}

#[test]
fn an_unknown_effect_fails_to_start() {
    let err = spawn(&Options {
        helper: env!("CARGO_BIN_EXE_opendj-plugin-host").into(),
        plugin: opendj_plugin_host::Plugin::Test("no-such-plugin".into()),
        sample_rate: 48_000,
        max_frames: 1024,
    })
    .err()
    .expect("refused");
    assert!(err.to_string().contains("unknown effect"), "{err}");
}

#[test]
fn a_round_trip_is_short() {
    let (mut conn, _helper) = start("test-gain");
    conn.set_param(0, 1.0);
    let mut times: Vec<Duration> = (0..200)
        .map(|_| {
            let t = Instant::now();
            assert_eq!(block(&mut conn, DEADLINE).0, Outcome::Wet);
            t.elapsed()
        })
        .collect();
    times.sort();
    let median = times[times.len() / 2];
    eprintln!("median round trip {median:?}");
    // It has to fit well inside the block: 256 frames at 48 kHz last 5.3 ms.
    assert!(median < Duration::from_millis(3), "median {median:?}");
}

#[test]
fn an_instrument_plays_the_notes_it_is_sent() {
    use opendj_plugin_host::Midi;
    let (mut conn, _helper) = start("test-synth");
    let silent = vec![0.0f32; FRAMES];
    let mut l = vec![9.0; FRAMES];
    let mut r = vec![9.0; FRAMES];
    let run = |conn: &mut Connection, events: &[Midi], l: &mut [f32], r: &mut [f32]| {
        let out = conn.process_events(
            [&silent, &silent],
            [l, r],
            Default::default(),
            events,
            DEADLINE,
        );
        assert_eq!(out, Outcome::Wet);
    };
    // Key 127 from frame 100 to frame 200: full level there, silence around.
    run(
        &mut conn,
        &[Midi::note_on(100, 0, 127, 100), Midi::note_off(200, 0, 127)],
        &mut l,
        &mut r,
    );
    assert!(l[..100].iter().all(|&v| v == 0.0));
    assert!(l[100..200].iter().all(|&v| v == 1.0));
    assert!(l[200..].iter().all(|&v| v == 0.0));
    assert_eq!(l, r);
    // A note held over the next block keeps sounding without new events.
    run(&mut conn, &[Midi::note_on(0, 0, 127, 100)], &mut l, &mut r);
    run(&mut conn, &[], &mut l, &mut r);
    assert!(l.iter().all(|&v| v == 1.0));
}
