//! Runs Mixxx's mappings for the DDJ-FLX4, Xone K2 and MIDI Fighter Twister
//! against the real engine controls, feeding them MIDI as the hardware would,
//! and checks that every bundled mapping loads.

use std::sync::Arc;
use std::time::{Duration, Instant};

use mixxx_control::ControlRegistry;
use mixxx_controllers::{default_mappings_dir, list_mappings, Controller, Mapping};

fn load(file: &str) -> (Arc<ControlRegistry>, Controller) {
    let reg = ControlRegistry::new();
    // Creates every engine control; the engine itself isn't run.
    let _engine = mixxx_engine::engine::Engine::new(&reg, 44_100.0);
    let path = default_mappings_dir().unwrap().join(file);
    let mapping = Mapping::load(&path).unwrap();
    let c = Controller::new("test", mapping, reg.clone()).unwrap();
    assert_eq!(
        c.errors(),
        Vec::<String>::new(),
        "{file}: errors during init"
    );
    (reg, c)
}

fn get(reg: &ControlRegistry, group: &str, key: &str) -> f64 {
    reg.get(group, key)
        .unwrap_or_else(|| panic!("{group},{key} missing"))
        .get()
}

/// Lets timers and connections run for a while.
fn settle(c: &mut Controller, ms: u64) {
    let start = Instant::now();
    for i in 0..=ms {
        c.poll(start + Duration::from_millis(i));
    }
}

#[test]
fn ddj_flx4() {
    let (reg, mut c) = load("Pioneer-DDJ-FLX4.midi.xml");
    let out = c.take_output();
    assert!(!out.is_empty(), "init lights the LEDs");

    // Play on deck 1, press and release.
    c.receive(&[0x90, 0x0B, 0x7F]);
    c.receive(&[0x90, 0x0B, 0x00]);
    assert_eq!(get(&reg, "[Channel1]", "play"), 1.0);
    // The engine sets the indicator while processing audio; do it by hand here.
    reg.get("[Channel1]", "play_indicator").unwrap().set(1.0);
    c.poll(Instant::now());
    let out = c.take_output();
    assert!(
        out.iter().any(|m| m[..] == [0x90, 0x0B, 0x7F]),
        "play LED lit: {out:x?}"
    );

    // Channel fader, 14-bit (MSB 0x13, LSB 0x33).
    c.receive(&[0xB0, 0x13, 0x20]);
    c.receive(&[0xB0, 0x33, 0x00]);
    let v = get(&reg, "[Channel1]", "volume");
    assert!((v - 0.25).abs() < 0.02, "volume {v}");

    // Jog wheel: touching the platter enables scratching, turning it
    // moves the deck (CC 0x22 on channel 1 is the platter, 0x40 = still).
    c.receive(&[0x90, 0x36, 0x7F]);
    c.poll(Instant::now());
    assert_eq!(
        get(&reg, "[Channel1]", "scratch2_enable"),
        1.0,
        "touch enables scratch"
    );
    let start = Instant::now();
    for i in 0..20u64 {
        c.receive(&[0xB0, 0x22, 0x48]);
        c.poll(start + Duration::from_millis(i));
    }
    let s = get(&reg, "[Channel1]", "scratch2");
    assert!(s > 0.0, "scratching forward: {s}");
    c.receive(&[0x90, 0x36, 0x00]);
    settle(&mut c, 500);
    assert_eq!(
        get(&reg, "[Channel1]", "scratch2_enable"),
        0.0,
        "release stops scratch"
    );

    // Errors so far: none.
    assert_eq!(c.errors(), Vec::<String>::new());
}

#[test]
fn xone_k2() {
    let (reg, mut c) = load("Allen and Heath Xone K2.midi.xml");
    settle(&mut c, 50);
    // MIDI channel 14 is the layout with deck A on the left column, two
    // effect units in the middle and deck B on the right. Faders are CC 16-19.
    c.receive(&[0xBD, 0x10, 0x00]);
    c.receive(&[0xBD, 0x10, 0x01]);
    c.poll(Instant::now());
    let v = get(&reg, "[Channel1]", "volume");
    assert!(v < 0.05, "deck A fader down: {v}");
    c.receive(&[0xBD, 0x13, 0x7F]);
    c.poll(Instant::now());
    assert!(get(&reg, "[Channel2]", "volume") > 0.95);
    assert_eq!(c.errors(), Vec::<String>::new());
}

#[test]
fn midi_fighter_twister_drives_the_effects() {
    let (reg, mut c) = load("DJ TechTools MIDI Fighter Twister.midi.xml");
    settle(&mut c, 50);
    let fx = "[EffectRack1_EffectUnit1_Effect1]";
    // Effect 1 of unit 1: knob (CC 0x11) is its meta. Turning it sends each
    // step; soft takeover drops one value early on, as in Mixxx.
    for v in 0x41..=0x50 {
        c.receive(&[0xB0, 0x11, v]);
    }
    c.poll(Instant::now());
    let meta = get(&reg, fx, "meta");
    assert!((meta - 79.0 / 126.0).abs() < 0.01, "meta {meta}");
    for v in 0x51..=0x7F {
        c.receive(&[0xB0, 0x11, v]);
        c.poll(Instant::now());
    }
    let meta = get(&reg, fx, "meta");
    assert!(meta > 0.99, "meta at the top {meta}");
    // Its push button (channel 2, same CC) turns the effect on.
    let before = get(&reg, fx, "enabled");
    c.receive(&[0xB1, 0x11, 0x7F]);
    c.receive(&[0xB1, 0x11, 0x00]);
    c.poll(Instant::now());
    assert_ne!(get(&reg, fx, "enabled"), before, "enable toggled");
    // Dry/wet of unit 1.
    for v in (0x20..=0x7F).rev() {
        c.receive(&[0xB0, 0x1D, v]);
    }
    c.poll(Instant::now());
    let mix = get(&reg, "[EffectRack1_EffectUnit1]", "mix");
    assert!((mix - 0.25).abs() < 0.01, "mix {mix}");
    assert_eq!(c.errors(), Vec::<String>::new());
}

#[test]
fn every_bundled_mapping_loads_and_takes_its_messages() {
    let dir = default_mappings_dir().unwrap();
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".midi.xml"))
        .collect();
    files.sort();
    assert_eq!(
        list_mappings(&dir).len(),
        files.len(),
        "every mapping is listed"
    );
    for path in files {
        let mapping = Mapping::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        for s in &mapping.scripts {
            assert!(
                s.path.exists(),
                "{}: missing {}",
                path.display(),
                s.path.display()
            );
        }
        let messages: Vec<(u8, u8)> = mapping.inputs.keys().copied().collect();
        let reg = ControlRegistry::new();
        let _engine = mixxx_engine::engine::Engine::new(&reg, 44_100.0);
        let mut c = Controller::new("test", mapping, reg)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        // Some of Mixxx's mappings have script errors of their own; what
        // matters here is that nothing they do takes the host down.
        let start = Instant::now();
        for (i, &(status, control)) in messages.iter().enumerate() {
            for value in [0x7F, 0x00] {
                c.receive(&[status, control, value]);
            }
            c.poll(start + Duration::from_millis(i as u64));
        }
        settle(&mut c, 100);
        c.shutdown();
    }
}

/// A one-off mapping with its own script, in a fresh folder.
fn scripted(name: &str, xml_controls: &str, script: &str) -> (Arc<ControlRegistry>, Controller) {
    let dir = std::env::temp_dir().join(format!("opendj-test-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let common = "common-controller-scripts.js";
    std::fs::copy(
        default_mappings_dir().unwrap().join(common),
        dir.join(common),
    )
    .unwrap();
    std::fs::write(dir.join("test.js"), script).unwrap();
    let xml = format!(
        r#"<MixxxControllerPreset><info><name>{name}</name></info><controller>
        <scriptfiles><file filename="test.js" functionprefix="T"/></scriptfiles>
        <controls>{xml_controls}</controls><outputs/></controller></MixxxControllerPreset>"#
    );
    let mapping = Mapping::parse(&xml, &dir).unwrap();
    let reg = ControlRegistry::new();
    let _engine = mixxx_engine::engine::Engine::new(&reg, 44_100.0);
    let c = Controller::new("test", mapping, reg.clone()).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    (reg, c)
}

#[test]
fn trigger_runs_connections_after_the_script_returns() {
    // As in Mixxx, where engine.trigger is queued: callbacks may use what
    // init sets up after triggering.
    let (_reg, c) = scripted(
        "trigger",
        "",
        r#"var T = {};
        T.init = function() {
            engine.makeConnection("[Channel1]", "play", function(v) { T.lights.push(v); });
            engine.trigger("[Channel1]", "play");
            T.lights = [];
        };"#,
    );
    assert_eq!(c.errors(), Vec::<String>::new());
}

#[test]
fn input_handlers_get_their_messages() {
    let (reg, mut c) = scripted(
        "inputhandler",
        r#"<control><group>[Channel1]</group><key>T.stop</key><status>0x90</status>
        <midino>0x01</midino><options><script-binding/></options></control>"#,
        r#"var T = {};
        T.init = function() {
            T.handler = midi.makeInputHandler(0x91, 0x10, function(channel, control, value, status) {
                engine.setValue("[Channel1]", "pregain", channel + control / 100 + value / 1000);
            });
        };
        T.stop = function() { T.handler.disconnect(); };"#,
    );
    c.receive(&[0x91, 0x10, 0x05]);
    assert!((get(&reg, "[Channel1]", "pregain") - 1.165).abs() < 1e-9);
    // Disconnected, it gets nothing more.
    c.receive(&[0x90, 0x01, 0x7F]);
    reg.get("[Channel1]", "pregain").unwrap().set(1.0);
    c.receive(&[0x91, 0x10, 0x7F]);
    assert_eq!(get(&reg, "[Channel1]", "pregain"), 1.0);
    assert_eq!(c.errors(), Vec::<String>::new());
}

#[test]
fn ddj_flx4_mixer_pads_and_eq_modes() {
    let (reg, mut c) = load("Pioneer-DDJ-FLX4.midi.xml");
    settle(&mut c, 50);
    // Tempo fader pushed from the middle to the top: rate +1, which
    // rate_dir turns into slower (soft takeover wants it moved, not set).
    for msb in (0..=0x40).rev() {
        c.receive(&[0xB0, 0x00, msb]);
        c.receive(&[0xB0, 0x20, 0x00]);
    }
    assert_eq!(get(&reg, "[Channel1]", "rate"), 1.0);
    // Master and headphones level, and the headphones mix.
    for (msb, lsb, key) in [(0x08, 0x28, "gain"), (0x0D, 0x2D, "headGain")] {
        c.receive(&[0xB6, msb, 0x40]);
        c.receive(&[0xB6, lsb, 0x00]);
        let v = get(&reg, "[Master]", key);
        assert!((v - 0.5).abs() < 0.01, "{key} {v}");
    }
    c.receive(&[0xB6, 0x0C, 0x00]);
    c.receive(&[0xB6, 0x2C, 0x00]);
    assert_eq!(get(&reg, "[Master]", "headMix"), -1.0);
    // Its centre detent is half cue, half main.
    for msb in 0x01..=0x40 {
        c.receive(&[0xB6, 0x0C, msb]);
        c.receive(&[0xB6, 0x2C, 0x00]);
    }
    let centre = get(&reg, "[Master]", "headMix");
    assert!(centre.abs() < 0.01, "centre {centre}");
    // Hot cue LEDs follow hotcue_N_status.
    c.take_output();
    reg.get("[Channel1]", "hotcue_1_status").unwrap().set(1.0);
    c.poll(Instant::now());
    let out = c.take_output();
    assert!(out.iter().any(|m| m[..] == [0x97, 0x00, 0x7F]), "{out:x?}");
    // 1/2X and 2X press loop_halve / loop_double, so the loop size box
    // follows even with no loop on.
    c.receive(&[0x90, 0x51, 0x7F]);
    assert_eq!(get(&reg, "[Channel1]", "loop_halve"), 1.0);
    c.receive(&[0x90, 0x51, 0x00]);
    assert_eq!(get(&reg, "[Channel1]", "loop_halve"), 0.0);
    c.receive(&[0x90, 0x53, 0x7F]);
    assert_eq!(get(&reg, "[Channel1]", "loop_double"), 1.0);
    // RELOOP/EXIT starts a new loop of the set size (or ends one).
    c.receive(&[0x90, 0x4D, 0x7F]);
    assert_eq!(get(&reg, "[Channel1]", "beatloop_activate"), 1.0);
    // BEAT FX: FX SELECT steps the Beat FX unit's effect, BEAT < > halve
    // and double its length, ON/OFF turns it on, SHIFT + ON/OFF releases it.
    let fx = "[EffectRack1_EffectUnit5_Effect1]";
    let first = get(&reg, fx, "loaded_effect");
    c.receive(&[0x94, 0x63, 0x7F]);
    c.receive(&[0x94, 0x63, 0x00]);
    let pushed = reg.get(fx, "next_effect").unwrap();
    assert!(
        pushed.version() > 0 && !pushed.get_bool(),
        "next_effect pressed: {first}"
    );
    reg.get(fx, "beats").unwrap().set(0.25);
    c.receive(&[0x94, 0x4B, 0x7F]);
    assert_eq!(get(&reg, fx, "beats"), 0.5);
    c.receive(&[0x94, 0x4A, 0x7F]);
    c.receive(&[0x94, 0x4A, 0x7F]);
    assert_eq!(get(&reg, fx, "beats"), 0.125);
    for v in 0..=0x40 {
        c.receive(&[0xB4, 0x02, v]);
    }
    assert!((get(&reg, fx, "meta") - 0.5).abs() < 0.01);
    c.receive(&[0x94, 0x47, 0x7F]);
    assert_eq!(get(&reg, fx, "enabled"), 1.0);
    let release = reg.get("[EffectRack1_EffectUnit5]", "release").unwrap();
    let before = release.version();
    c.receive(&[0x94, 0x43, 0x7F]);
    assert!(release.version() > before, "SHIFT + ON/OFF releases");
    // Pad mode buttons ask deck 2 to show the beat jump pads.
    c.receive(&[0x91, 0x20, 0x7F]);
    assert_eq!(get(&reg, "[Channel2]", "pad_panel"), 2.0);
    // SHIFT + left LOAD asks for the Linked tab; each press counts.
    reg.free("[Library]", "ShowLinked");
    c.receive(&[0x96, 0x58, 0x7F]);
    c.receive(&[0x96, 0x58, 0x00]);
    assert_eq!(get(&reg, "[Library]", "ShowLinked"), 1.0);
    // In stems mode the low EQ knob turns the kick stem.
    reg.get("[Mixer]", "knob_mode").unwrap().set(1.0);
    c.receive(&[0xB0, 0x0F, 0x10]);
    c.receive(&[0xB0, 0x2F, 0x00]);
    let kick = reg
        .get("[Channel1_Stem1]", "volume")
        .unwrap()
        .get_parameter();
    assert!((kick - 0.125).abs() < 0.01, "kick {kick}");
    reg.get("[Mixer]", "knob_mode").unwrap().set(2.0);
    c.receive(&[0xB6, 0x17, 0x40]);
    c.receive(&[0xB6, 0x37, 0x00]);
    let hpf = reg
        .get("[Sculpt_[Channel1]]", "hpf")
        .unwrap()
        .get_parameter();
    assert!((hpf - 0.5).abs() < 0.01, "hpf {hpf}");
    assert_eq!(c.errors(), Vec::<String>::new());
}
