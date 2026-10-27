//! Effect units, as in Mixxx's effect rack: `[EffectRack1_EffectUnitN]` holds
//! three effect slots in series, `[EffectRack1_EffectUnitN_EffectM]`, and is
//! routed to any of the decks and the main mix.
//!
//! Each (unit, slot, target) has its own effect instance so a unit can process
//! both decks and the main mix at once. All memory is allocated here, up
//! front: an instance owns one buffer big enough for the hungriest effect
//! (Echo's delay line) and whichever effect is loaded uses it, so switching
//! effects on the audio thread never allocates. A slot fades out before it
//! switches, and fades in and out when it is turned on or off.

use std::f32::consts::{PI, TAU};

use mixxx_control::{Behavior, ControlHandle, ControlRegistry};

use crate::controls::{
    deck_group, effect_group, effect_unit_group, Edge, BEAT_FX_UNIT, EFFECTS_PER_UNIT, MIC_FX_UNIT,
    NUM_DECKS, NUM_EFFECT_UNITS,
};
use crate::dsp::Svf;
use crate::inputs::{input_group, MICROPHONE, NUM_INPUTS};

/// Mixxx's built-in effects that this engine implements, in `loaded_effect` order.
pub const AVAILABLE_EFFECTS: [&str; 12] = [
    "Echo",
    "Reverb",
    "Filter",
    "Flanger",
    "Bitcrusher",
    "Phaser",
    "Gate",
    "Roll",
    "Echo Out",
    "Brake",
    "Noise",
    "Spiral",
];

/// Parameter knobs (`parameterK`) and buttons (`button_parameterK`) per slot.
pub const MAX_PARAMETERS: usize = 4;
pub const MAX_BUTTON_PARAMETERS: usize = 2;

/// The effects each slot starts with, as `loaded_effect` values.
pub const DEFAULT_EFFECTS: [[usize; EFFECTS_PER_UNIT]; NUM_EFFECT_UNITS] = [
    [1, 2, 3],
    [4, 5, 6],
    [1, 6, 5],
    [2, 4, 3],
    [1, 0, 0],
    [0; 3],
    [0; 3],
    [0; 3],
    [0; 3],
    [2, 1, 3],
];

/// How long a released effect's tail rings on, in beats (see `release`).
const RELEASE_BEATS: f32 = 8.0;
/// Taps further apart than this, in seconds, start a new tap tempo.
const TAP_GAP: f64 = 2.0;
/// Tap intervals averaged into the tapped tempo.
const TAPS: usize = 4;

/// The name of the effect a `loaded_effect` value refers to: 1 is the first of
/// [`AVAILABLE_EFFECTS`], 0 means the slot is empty ("").
pub fn effect_name(loaded_effect: f64) -> &'static str {
    Kind::from_loaded(loaded_effect).map_or("", |k| AVAILABLE_EFFECTS[k as usize])
}

/// Names of the loaded effect's parameters, in `parameterK` order.
pub fn parameter_names(loaded_effect: f64) -> &'static [&'static str] {
    Kind::from_loaded(loaded_effect).map_or(&[], |k| MANIFESTS[k as usize].names)
}

/// Signal paths an effect unit can be routed to: the decks, the main mix,
/// then the microphone and auxiliary inputs.
pub(crate) const TARGETS: usize = NUM_DECKS + 1 + NUM_INPUTS;
pub(crate) const MAIN: usize = NUM_DECKS;

/// The target of input `input` (see [`crate::inputs`]).
pub(crate) const fn input_target(input: usize) -> usize {
    MAIN + 1 + input
}

/// Whether `unit` can process `target`: the microphone's unit only the
/// inputs, the others only the decks and the main mix. Instances are only
/// made for these pairs, which keeps the rack's memory down.
fn routable(unit: usize, target: usize) -> bool {
    (unit == MIC_FX_UNIT) == (target > MAIN)
}

/// Longest echo, in seconds; it sizes every instance's buffer.
const ECHO_MAX_SECONDS: f32 = 2.0;
const FLANGER_MAX_SECONDS: f32 = 0.02;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Echo,
    Reverb,
    Filter,
    Flanger,
    Bitcrusher,
    Phaser,
    Gate,
    Roll,
    EchoOut,
    Brake,
    Noise,
    Spiral,
}

const KINDS: [Kind; 12] = [
    Kind::Echo,
    Kind::Reverb,
    Kind::Filter,
    Kind::Flanger,
    Kind::Bitcrusher,
    Kind::Phaser,
    Kind::Gate,
    Kind::Roll,
    Kind::EchoOut,
    Kind::Brake,
    Kind::Noise,
    Kind::Spiral,
];

impl Kind {
    fn from_loaded(v: f64) -> Option<Kind> {
        let i = v.round();
        (i >= 1.0 && i <= KINDS.len() as f64).then(|| KINDS[i as usize - 1])
    }
}

/// An effect's parameters (all normalized 0..1) and their defaults.
struct Manifest {
    names: &'static [&'static str],
    defaults: &'static [f64],
}

const MANIFESTS: [Manifest; 12] = [
    Manifest {
        names: &["Time", "Feedback", "Ping Pong", "Send"],
        defaults: &[0.5, 0.5, 0.0, 1.0],
    },
    Manifest {
        names: &["Decay", "Bandwidth", "Damping", "Send"],
        defaults: &[0.5, 0.75, 0.5, 1.0],
    },
    Manifest {
        names: &["Low Pass", "Resonance", "High Pass"],
        defaults: &[1.0, 0.25, 0.0],
    },
    Manifest {
        names: &["Speed", "Depth", "Manual", "Regeneration"],
        defaults: &[0.4, 0.5, 0.4, 0.5],
    },
    Manifest {
        names: &["Bit Depth", "Downsampling"],
        defaults: &[1.0, 1.0],
    },
    Manifest {
        names: &["Speed", "Depth", "Feedback", "Stages"],
        defaults: &[0.4, 0.5, 0.5, 0.4],
    },
    Manifest {
        names: &["Length", "Depth", "Width"],
        defaults: &[0.5, 0.8, 0.5],
    },
    Manifest {
        names: &["Rate"],
        defaults: &[0.25],
    },
    Manifest {
        names: &["Time", "Feedback", "Length"],
        defaults: &[0.5, 0.6, 0.67],
    },
    Manifest {
        names: &["Length"],
        defaults: &[0.33],
    },
    Manifest {
        names: &["Level", "Sweep", "Resonance"],
        defaults: &[0.5, 0.5, 0.4],
    },
    Manifest {
        names: &["Time", "Feedback", "Send"],
        defaults: &[0.67, 0.6, 1.0],
    },
];

/// The beat switch's range: 1/16 to 4 beats, as powers of two.
const BEATS_MIN: i32 = -4;
const BEATS_MAX: i32 = 2;
/// Where a slot's beat switch starts.
const DEFAULT_BEATS: f64 = 0.25;

/// The parameter that sets an effect's length in beats, as
/// (index, lowest and highest power of two, inverted): the inverse of
/// how its `process` reads it with [`beats`].
fn time_parameter(kind: Kind) -> Option<(usize, i32, i32, bool)> {
    match kind {
        Kind::Echo => Some((0, -4, 2, false)),
        Kind::Flanger | Kind::Phaser => Some((0, -1, 5, true)),
        Kind::Gate => Some((0, -4, 0, false)),
        Kind::Roll => Some((0, -4, 0, true)),
        Kind::EchoOut | Kind::Spiral => Some((0, -2, 0, false)),
        Kind::Brake => Some((0, 0, 3, false)),
        Kind::Reverb | Kind::Filter | Kind::Bitcrusher | Kind::Noise => None,
    }
}

/// The parameters the meta knob drives, as Mixxx's linked parameters do:
/// meta 0 is subtle and 1 strong. The knob is unipolar, so Filter is neutral
/// at 0 and turning up sweeps its high-pass.
fn link(kind: Kind, meta: f64) -> [Option<f64>; MAX_PARAMETERS] {
    match kind {
        Kind::Echo | Kind::Reverb => [None, None, None, Some(meta)],
        Kind::Filter => [Some(1.0), None, Some(meta), None],
        Kind::Flanger | Kind::Phaser | Kind::Gate => [None, Some(meta), None, None],
        Kind::Bitcrusher => [Some(1.0 - 0.8 * meta), Some(1.0 - 0.95 * meta), None, None],
        // Turning up rolls shorter slices; brakes and echo-outs get longer.
        // Their length is the slot's beat switch.
        Kind::Roll | Kind::Brake => [None; MAX_PARAMETERS],
        Kind::EchoOut => [None, Some(meta), None, None],
        Kind::Noise => [Some(meta), Some(meta), None, None],
        Kind::Spiral => [None, None, Some(meta), None],
    }
}

struct SlotControls {
    enabled: ControlHandle,
    meta: ControlHandle,
    /// Length in beats, 1/16 to 4 (0 for effects without one), as the
    /// slot's beat switch shows it.
    beats: ControlHandle,
    beats_seen: f64,
    params: [ControlHandle; MAX_PARAMETERS],
    loaded_effect: ControlHandle,
    loaded: ControlHandle,
    num_parameters: ControlHandle,
    effect_selector: ControlHandle,
    next: ControlHandle,
    prev: ControlHandle,
    clear: ControlHandle,
    edges: [Edge; 3],
    /// The effect the parameters are set up for.
    kind: Option<Kind>,
    meta_seen: f64,
}

impl SlotControls {
    fn create(reg: &ControlRegistry, unit: usize, slot: usize) -> Self {
        let g = effect_group(unit, slot);
        let g = g.as_str();
        let n = KINDS.len() as f64;
        for k in 1..=MAX_BUTTON_PARAMETERS {
            reg.toggle(g, &format!("button_parameter{k}"));
        }
        reg.indicator(g, "num_button_parameters", MAX_BUTTON_PARAMETERS as f64);
        let mut s = Self {
            enabled: reg.toggle(g, "enabled"),
            // The Beat FX and the mic's effects start half way, so turning
            // them on is heard at once.
            meta: reg.create(
                g,
                "meta",
                if unit == BEAT_FX_UNIT || unit == MIC_FX_UNIT {
                    0.5
                } else {
                    0.0
                },
                0.0,
                1.0,
                Behavior::Linear,
            ),
            beats: reg.create(g, "beats", 1.0, 0.0, 4.0, Behavior::Linear),
            beats_seen: f64::NAN,
            params: std::array::from_fn(|k| {
                reg.create(
                    g,
                    &format!("parameter{}", k + 1),
                    0.5,
                    0.0,
                    1.0,
                    Behavior::Linear,
                )
            }),
            loaded_effect: reg.create(
                g,
                "loaded_effect",
                DEFAULT_EFFECTS[unit][slot] as f64,
                0.0,
                n,
                Behavior::Linear,
            ),
            loaded: reg.indicator(g, "loaded", 1.0),
            num_parameters: reg.indicator(g, "num_parameters", MAX_PARAMETERS as f64),
            effect_selector: reg.create(g, "effect_selector", 0.0, -n, n, Behavior::Free),
            next: reg.push(g, "next_effect"),
            prev: reg.push(g, "prev_effect"),
            clear: reg.push(g, "clear"),
            edges: [Edge::default(); 3],
            kind: None,
            meta_seen: f64::NAN,
        };
        s.update();
        s
    }

    /// Handles effect selection and the meta knob; returns the loaded effect.
    fn update(&mut self) -> Option<Kind> {
        let mut step = 0i32;
        let sel = self.effect_selector.get();
        if sel != 0.0 {
            step += sel.signum() as i32;
            self.effect_selector.set(0.0);
        }
        if self.edges[0].pressed(&self.next) {
            step += 1;
        }
        if self.edges[1].pressed(&self.prev) {
            step -= 1;
        }
        if self.edges[2].pressed(&self.clear) {
            self.loaded_effect.set(0.0);
        }
        if step != 0 {
            // Cycles through the effects; an empty slot steps to either end.
            let n = KINDS.len() as i32;
            let cur = self.loaded_effect.get().round() as i32;
            let next = if cur == 0 {
                if step > 0 {
                    1
                } else {
                    n
                }
            } else {
                (cur - 1 + step).rem_euclid(n) + 1
            };
            self.loaded_effect.set(next as f64);
        }

        let kind = Kind::from_loaded(self.loaded_effect.get());
        if kind != self.kind {
            self.kind = kind;
            // A newly loaded effect starts from its defaults, then follows meta.
            let defaults = kind.map_or(&[][..], |k| MANIFESTS[k as usize].defaults);
            for (k, p) in self.params.iter().enumerate() {
                p.set(defaults.get(k).copied().unwrap_or(0.5));
            }
            self.loaded.set_bool(kind.is_some());
            self.num_parameters.set(defaults.len() as f64);
            self.meta_seen = f64::NAN;
            // The beat switch starts at 1/4 beat, or the nearest length the
            // effect allows.
            self.beats.set(if kind.and_then(time_parameter).is_some() {
                DEFAULT_BEATS
            } else {
                0.0
            });
            self.beats_seen = f64::NAN;
        }
        let meta = self.meta.get();
        if meta != self.meta_seen {
            self.meta_seen = meta;
            if let Some(k) = kind {
                for (p, v) in self.params.iter().zip(link(k, meta)) {
                    if let Some(v) = v {
                        p.set(v);
                    }
                }
            }
        }
        let wanted = self.beats.get();
        if wanted != self.beats_seen {
            match kind.and_then(time_parameter) {
                Some((i, lo, hi, inverted)) => {
                    // The nearest power of two the effect and the switch both allow.
                    let power = if wanted > 0.0 {
                        wanted.log2().round() as i32
                    } else {
                        0
                    }
                    .clamp(lo.max(BEATS_MIN), hi.min(BEATS_MAX));
                    let p = (power - lo) as f64 / (hi - lo) as f64;
                    self.params[i].set(if inverted { 1.0 - p } else { p });
                    self.beats.set(2f64.powi(power));
                }
                None => self.beats.set(0.0),
            }
            self.beats_seen = self.beats.get();
        }
        kind
    }
}

struct UnitControls {
    enabled: ControlHandle,
    mix: ControlHandle,
    super1: ControlHandle,
    /// group_[ChannelN]_enable for each deck, group_[Master]_enable, then
    /// group_[Microphone]_enable and group_[Auxiliary1]_enable.
    routes: [ControlHandle; TARGETS],
    super_seen: u32,
    /// The unit's own tempo, from `bpm_tap`; 0 follows each target's tempo.
    bpm: ControlHandle,
    /// Pushes: `bpm_tap` taps a tempo, `bpm_auto` goes back to following the
    /// decks, and `release` turns the unit's effects off and lets their
    /// tails (echoes, reverb) ring out instead of cutting them.
    tap: ControlHandle,
    auto: ControlHandle,
    release: ControlHandle,
    edges: [Edge; 3],
    /// When the last tap came, on the engine clock, and the intervals before it.
    last_tap: f64,
    intervals: [f64; TAPS],
    taps: usize,
}

impl UnitControls {
    fn create(reg: &ControlRegistry, unit: usize) -> Self {
        let g = effect_unit_group(unit);
        let g = g.as_str();
        let n = EFFECTS_PER_UNIT as f64;
        reg.toggle(g, "mix_mode");
        reg.create(g, "focused_effect", 0.0, 0.0, n, Behavior::Linear);
        reg.toggle(g, "show_focus");
        reg.toggle(g, "show_parameters");
        reg.toggle(g, "controller_input_active");
        reg.indicator(g, "num_effects", n).set(n);
        reg.indicator(g, "num_effectslots", n).set(n);
        // Chain presets are not implemented: -1 is Mixxx's "unsaved chain".
        reg.create(g, "loaded_chain_preset", -1.0, -1.0, 64.0, Behavior::Free);
        reg.indicator(g, "num_chain_presets", 64.0).set(1.0);
        reg.push(g, "next_chain_preset");
        reg.push(g, "prev_chain_preset");
        reg.create(g, "chain_preset_selector", 0.0, -64.0, 64.0, Behavior::Free);
        // The headphones are stored only, as are targets the unit can't be
        // routed to.
        reg.toggle(g, "group_[Headphone]_enable");
        let routes = std::array::from_fn(|t| {
            if t == MAIN {
                reg.toggle(g, "group_[Master]_enable")
            } else if t > MAIN {
                let input = t - MAIN - 1;
                let r = reg.toggle(g, &format!("group_{}_enable", input_group(input)));
                // The microphone's unit starts on the microphone.
                if unit == MIC_FX_UNIT && input == MICROPHONE {
                    r.set_bool(true);
                }
                r
            } else {
                let r = reg.toggle(g, &format!("group_{}_enable", deck_group(t)));
                // Each deck's unit starts on its deck, as the decks show; the
                // Beat FX on decks 1 and 2. Pad FX units are routed when
                // their pads are held.
                if t == unit || (unit == BEAT_FX_UNIT && t < 2) {
                    r.set_bool(true);
                }
                r
            }
        });
        let super1 = reg.create(g, "super1", 0.0, 0.0, 1.0, Behavior::Linear);
        Self {
            enabled: {
                let e = reg.toggle(g, "enabled");
                e.set_bool(true);
                e
            },
            mix: reg.create(g, "mix", 1.0, 0.0, 1.0, Behavior::Linear),
            super_seen: super1.version(),
            super1,
            routes,
            bpm: reg.create(g, "bpm", 0.0, 0.0, 300.0, Behavior::Free),
            tap: reg.push(g, "bpm_tap"),
            auto: reg.push(g, "bpm_auto"),
            release: reg.push(g, "release"),
            edges: [Edge::default(); 3],
            last_tap: f64::NEG_INFINITY,
            intervals: [0.0; TAPS],
            taps: 0,
        }
    }

    /// Takes in `bpm_tap` and `bpm_auto` presses; `now` is the engine clock
    /// in seconds.
    fn update_tempo(&mut self, now: f64) {
        if self.edges[1].pressed(&self.auto) {
            self.bpm.set(0.0);
            self.taps = 0;
        }
        if self.edges[0].pressed(&self.tap) {
            let gap = now - self.last_tap;
            self.last_tap = now;
            if gap > TAP_GAP || gap <= 0.0 {
                self.taps = 0;
                return;
            }
            self.intervals.rotate_right(1);
            self.intervals[0] = gap;
            self.taps = (self.taps + 1).min(TAPS);
            let mean = self.intervals[..self.taps].iter().sum::<f64>() / self.taps as f64;
            self.bpm.set((60.0 / mean).clamp(40.0, 300.0));
        }
    }
}

/// Per-buffer values an effect needs besides its parameters.
#[derive(Clone, Copy)]
struct Ctx {
    sample_rate: f32,
    /// Length of a beat at the target's current tempo.
    beat_seconds: f32,
}

/// One effect instance: every effect type's state, of which one is in use.
struct Instance {
    kind: Option<Kind>,
    /// Output level reached at the end of the last buffer, and this buffer's goal.
    gain: f32,
    target: f32,
    /// Samples left of a released tail (see `release`), and its full length.
    release: f32,
    release_len: f32,
    mem: Box<[f32]>,
    echo: Echo,
    reverb: Reverb,
    filter: Filter,
    flanger: Flanger,
    crusher: Bitcrusher,
    phaser: Phaser,
    gate: Gate,
    roll: Roll,
    echo_out: EchoOut,
    brake: Brake,
    noise: Noise,
    spiral: Spiral,
}

impl Instance {
    fn new(sample_rate: f32) -> Self {
        let echo = Echo::new(sample_rate);
        let echo_len = echo.len;
        let reverb = Reverb::new(sample_rate);
        let flanger = Flanger::new(sample_rate);
        let size = (2 * echo.len).max(reverb.size).max(2 * flanger.len);
        Self {
            kind: None,
            gain: 0.0,
            target: 0.0,
            release: 0.0,
            release_len: 0.0,
            mem: vec![0.0; size].into_boxed_slice(),
            echo,
            reverb,
            filter: Filter::new(sample_rate),
            flanger,
            crusher: Bitcrusher::default(),
            phaser: Phaser::default(),
            gate: Gate::default(),
            roll: Roll::new(echo_len),
            echo_out: EchoOut::new(sample_rate),
            brake: Brake::new(echo_len),
            noise: Noise::default(),
            spiral: Spiral::new(sample_rate),
        }
    }

    fn active(&self) -> bool {
        self.gain > 0.0 || self.target > 0.0 || self.release > 0.0
    }

    /// The tail of a released effect: what it plays with no more input,
    /// fading out over the last quarter of the release.
    #[inline]
    fn tail(&mut self) -> [f32; 2] {
        let fade = (self.release / (0.25 * self.release_len)).min(1.0);
        self.release -= 1.0;
        if self.release <= 0.0 {
            self.release = 0.0;
        }
        let y = self.process([0.0; 2]);
        [y[0] * fade, y[1] * fade]
    }

    /// Clears the loaded effect's state so it starts silent, without old tails.
    fn reset(&mut self) {
        match self.kind {
            Some(Kind::Echo) => self.echo.reset(&mut self.mem),
            Some(Kind::Reverb) => self.reverb.reset(&mut self.mem),
            Some(Kind::Filter) => self.filter.reset(),
            Some(Kind::Flanger) => self.flanger.reset(&mut self.mem),
            Some(Kind::Bitcrusher) => self.crusher = Bitcrusher::default(),
            Some(Kind::Phaser) => self.phaser = Phaser::default(),
            Some(Kind::Gate) => self.gate = Gate::default(),
            Some(Kind::Roll) => self.roll.reset(),
            Some(Kind::EchoOut) => self.echo_out.reset(&mut self.mem),
            Some(Kind::Brake) => self.brake.reset(&mut self.mem),
            Some(Kind::Noise) => self.noise.reset(),
            Some(Kind::Spiral) => self.spiral.reset(&mut self.mem),
            None => {}
        }
    }

    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        match self.kind {
            Some(Kind::Echo) => self.echo.prepare(p, ctx),
            Some(Kind::Reverb) => self.reverb.prepare(p),
            Some(Kind::Filter) => self.filter.prepare(p),
            Some(Kind::Flanger) => self.flanger.prepare(p, ctx),
            Some(Kind::Bitcrusher) => self.crusher.prepare(p),
            Some(Kind::Phaser) => self.phaser.prepare(p, ctx),
            Some(Kind::Gate) => self.gate.prepare(p, ctx),
            Some(Kind::Roll) => self.roll.prepare(p, ctx),
            Some(Kind::EchoOut) => self.echo_out.prepare(p, ctx),
            Some(Kind::Brake) => self.brake.prepare(p, ctx),
            Some(Kind::Noise) => self.noise.prepare(p, ctx),
            Some(Kind::Spiral) => self.spiral.prepare(p, ctx),
            None => {}
        }
    }

    #[inline]
    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        match self.kind {
            Some(Kind::Echo) => self.echo.process(&mut self.mem, x),
            Some(Kind::Reverb) => self.reverb.process(&mut self.mem, x),
            Some(Kind::Filter) => self.filter.process(x),
            Some(Kind::Flanger) => self.flanger.process(&mut self.mem, x),
            Some(Kind::Bitcrusher) => self.crusher.process(x),
            Some(Kind::Phaser) => self.phaser.process(x),
            Some(Kind::Gate) => self.gate.process(x),
            Some(Kind::Roll) => self.roll.process(&mut self.mem, x),
            Some(Kind::EchoOut) => self.echo_out.process(&mut self.mem, x),
            Some(Kind::Brake) => self.brake.process(&mut self.mem, x),
            Some(Kind::Noise) => self.noise.process(x),
            Some(Kind::Spiral) => self.spiral.process(&mut self.mem, x),
            None => x,
        }
    }
}

/// All effect units: their controls and their instances.
pub(crate) struct EffectRack {
    sample_rate: f32,
    units: [UnitControls; NUM_EFFECT_UNITS],
    slots: [[SlotControls; EFFECTS_PER_UNIT]; NUM_EFFECT_UNITS],
    /// Indexed by [`EffectRack::index`].
    instances: Vec<Instance>,
    /// Each (unit, slot, target)'s instance, if the unit can be routed there.
    index: [[[Option<u16>; TARGETS]; EFFECTS_PER_UNIT]; NUM_EFFECT_UNITS],
    /// Unit mix at the start and end of this buffer.
    mix: [(f32, f32); NUM_EFFECT_UNITS],
    /// Whether a unit has any slot sounding on a target this buffer.
    active: [[bool; TARGETS]; NUM_EFFECT_UNITS],
}

impl EffectRack {
    pub(crate) fn new(reg: &ControlRegistry, sample_rate: f64) -> Self {
        let sample_rate = sample_rate as f32;
        let mut n = 0;
        let index = std::array::from_fn(|u| {
            std::array::from_fn(|_| {
                std::array::from_fn(|t| {
                    routable(u, t).then(|| {
                        n += 1;
                        n as u16 - 1
                    })
                })
            })
        });
        let units: [UnitControls; NUM_EFFECT_UNITS] =
            std::array::from_fn(|u| UnitControls::create(reg, u));
        reg.indicator("[Master]", "num_effectsavailable", KINDS.len() as f64)
            .set(KINDS.len() as f64);
        Self {
            sample_rate,
            mix: std::array::from_fn(|u| {
                let m = units[u].mix.get() as f32;
                (m, m)
            }),
            units,
            slots: std::array::from_fn(|u| {
                std::array::from_fn(|s| SlotControls::create(reg, u, s))
            }),
            instances: (0..n).map(|_| Instance::new(sample_rate)).collect(),
            index,
            active: [[false; TARGETS]; NUM_EFFECT_UNITS],
        }
    }

    fn instance(&mut self, unit: usize, slot: usize, target: usize) -> Option<&mut Instance> {
        let i = self.index[unit][slot][target]?;
        Some(&mut self.instances[i as usize])
    }

    /// Reads the controls once per buffer. `beat_seconds` is each target's
    /// beat length, for tempo-synced effects; `now` is the engine clock in
    /// seconds, for tap tempo.
    pub(crate) fn prepare(&mut self, beat_seconds: [f64; TARGETS], now: f64) {
        for u in 0..NUM_EFFECT_UNITS {
            let unit = &mut self.units[u];
            unit.update_tempo(now);
            let bpm = unit.bpm.get();
            let beat_seconds = beat_seconds.map(|b| if bpm > 0.0 { 60.0 / bpm } else { b });
            let release = unit.edges[2].pressed(&unit.release);
            let version = unit.super1.version();
            if version != unit.super_seen {
                // super1 drives every slot's meta knob, as Mixxx's linked super knob does.
                unit.super_seen = version;
                let v = unit.super1.get();
                for slot in &self.slots[u] {
                    slot.meta.set(v);
                }
            }
            let on = unit.enabled.get_bool();
            let routed: [bool; TARGETS] = std::array::from_fn(|t| unit.routes[t].get_bool());
            self.mix[u] = (self.mix[u].1, unit.mix.get() as f32);
            self.active[u] = [false; TARGETS];
            for s in 0..EFFECTS_PER_UNIT {
                let slot = &mut self.slots[u][s];
                let kind = slot.update();
                let releasing = release && slot.enabled.get_bool();
                if releasing {
                    slot.enabled.set_bool(false);
                }
                let enabled = on && slot.enabled.get_bool();
                let params = slot.params.each_ref().map(|p| p.get() as f32);
                for (t, &routed) in routed.iter().enumerate() {
                    let Some(i) = self.index[u][s][t] else {
                        continue;
                    };
                    let inst = &mut self.instances[i as usize];
                    if releasing && inst.gain > 0.0 {
                        // The tail takes over from the effect at once.
                        inst.release = RELEASE_BEATS * (beat_seconds[t] as f32) * self.sample_rate;
                        inst.release_len = inst.release;
                        inst.gain = 0.0;
                    }
                    // Switch effects only once the old one has faded out.
                    if inst.kind != kind && inst.gain == 0.0 && inst.release == 0.0 {
                        inst.kind = kind;
                    }
                    let on = enabled && routed && inst.kind == kind && kind.is_some();
                    inst.target = if on { 1.0 } else { 0.0 };
                    if inst.gain == 0.0 && on {
                        inst.release = 0.0;
                        inst.reset();
                    }
                    if inst.active() {
                        // While the old effect fades out it keeps its settings;
                        // the parameters already belong to the new one.
                        if inst.kind == kind {
                            let ctx = Ctx {
                                sample_rate: self.sample_rate,
                                beat_seconds: beat_seconds[t] as f32,
                            };
                            inst.prepare(&params, ctx);
                        }
                        self.active[u][t] = true;
                    }
                }
            }
        }
    }

    /// Runs one frame of `target` through the units routed to it. `r` is how
    /// far through the buffer this frame is (0..1], for ramps.
    #[inline]
    pub(crate) fn process(&mut self, target: usize, mut x: [f32; 2], r: f32) -> [f32; 2] {
        for u in 0..NUM_EFFECT_UNITS {
            if !self.active[u][target] {
                continue;
            }
            let mut wet = x;
            let mut tails = [0.0f32; 2];
            for s in 0..EFFECTS_PER_UNIT {
                let Some(inst) = self.instance(u, s, target) else {
                    continue;
                };
                if !inst.active() {
                    continue;
                }
                if inst.release > 0.0 {
                    let y = inst.tail();
                    tails[0] += y[0];
                    tails[1] += y[1];
                    continue;
                }
                let g = inst.gain + (inst.target - inst.gain) * r;
                let y = inst.process(wet);
                for c in 0..2 {
                    wet[c] += (y[c] - wet[c]) * g;
                }
            }
            // Dry/wet. Every effect's output already carries the dry signal
            // where it musically belongs (an echo plays over the original), so
            // Mixxx's Dry+Wet mode would sound the same and is not separate.
            let (a, b) = self.mix[u];
            let m = a + (b - a) * r;
            for c in 0..2 {
                x[c] += (wet[c] - x[c]) * m + tails[c] * m;
            }
        }
        x
    }

    /// Ends the buffer: the next one ramps on from where this one finished.
    pub(crate) fn finish(&mut self) {
        for inst in &mut self.instances {
            inst.gain = inst.target;
        }
    }
}

/// Reads a delay line `delay` samples behind `write`, with linear interpolation.
#[inline]
fn tap(buf: &[f32], write: usize, delay: f32) -> f32 {
    let len = buf.len();
    let mut pos = write as f32 - delay;
    if pos < 0.0 {
        pos += len as f32;
    }
    let i = (pos as usize).min(len - 1);
    let frac = pos - i as f32;
    let j = if i + 1 == len { 0 } else { i + 1 };
    buf[i] + (buf[j] - buf[i]) * frac
}

/// `2^lo .. 2^hi` beats, in whole powers of two, picked by a 0..1 parameter.
fn beats(p: f32, lo: i32, hi: i32) -> f32 {
    2f32.powi(lo + (p * (hi - lo) as f32).round() as i32)
}

/// Gentle saturation for feedback paths, so repeats can't build up and clip
/// hard: a rational tanh approximation, close to linear for small signals.
#[inline]
fn soft(v: f32) -> f32 {
    let v = v.clamp(-3.0, 3.0);
    v * (27.0 + v * v) / (27.0 + 9.0 * v * v)
}

/// Tempo-synced stereo echo with filtered, optionally ping-ponging repeats.
struct Echo {
    len: usize,
    write: usize,
    /// Delay in samples, gliding towards `target` so time changes do not click.
    delay: f32,
    target: f32,
    /// Feedback, gliding towards `feedback_target` so turning it doesn't zip.
    feedback: f32,
    feedback_target: f32,
    pingpong: f32,
    send: f32,
    tone: [f32; 2],
    /// One-pole coefficient darkening each repeat (about 6 kHz).
    tone_coef: f32,
}

impl Echo {
    fn new(sample_rate: f32) -> Self {
        Self {
            len: (ECHO_MAX_SECONDS * sample_rate) as usize + 4,
            write: 0,
            delay: -1.0,
            target: 1.0,
            feedback: 0.0,
            feedback_target: 0.0,
            pingpong: 0.0,
            send: 0.0,
            tone: [0.0; 2],
            tone_coef: 1.0 - (-TAU * 6_000.0 / sample_rate).exp(),
        }
    }

    fn reset(&mut self, mem: &mut [f32]) {
        mem[..2 * self.len].fill(0.0);
        self.write = 0;
        self.delay = -1.0;
        self.tone = [0.0; 2];
        self.feedback = self.feedback_target;
    }

    /// Sets the delay to `beats` at the current tempo, within the buffer.
    fn set_time(&mut self, beats: f32, ctx: Ctx) {
        self.target =
            (beats * ctx.beat_seconds * ctx.sample_rate).clamp(1.0, (self.len - 2) as f32);
        if self.delay < 0.0 {
            self.delay = self.target;
        }
    }

    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        // Time: 1/16 to 4 beats (as long as the buffer allows at slow tempos).
        self.set_time(beats(p[0], -4, 2), ctx);
        self.feedback_target = p[1] * 0.9;
        self.pingpong = p[2];
        self.send = p[3];
    }

    #[inline]
    fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        let (l, r) = mem[..2 * self.len].split_at_mut(self.len);
        self.delay += (self.target - self.delay) * 0.0005;
        self.feedback += (self.feedback_target - self.feedback) * 0.002;
        let e = [
            tap(l, self.write, self.delay),
            tap(r, self.write, self.delay),
        ];
        let pp = self.pingpong;
        let fed = [e[0] * (1.0 - pp) + e[1] * pp, e[1] * (1.0 - pp) + e[0] * pp];
        // Each repeat a little darker, as on a tape echo.
        for (t, f) in self.tone.iter_mut().zip(fed) {
            *t += (f - *t) * self.tone_coef;
        }
        l[self.write] = x[0] * self.send + soft(self.tone[0] * self.feedback);
        r[self.write] = x[1] * self.send + soft(self.tone[1] * self.feedback);
        self.write += 1;
        if self.write == self.len {
            self.write = 0;
        }
        [x[0] + e[0], x[1] + e[1]]
    }
}

/// A delay line inside an instance's shared buffer.
#[derive(Clone, Copy, Default)]
struct Line {
    at: usize,
    len: usize,
    pos: usize,
}

impl Line {
    /// The sample written `delay` samples ago (1 = the last one written).
    #[inline]
    fn read(&self, mem: &[f32], delay: usize) -> f32 {
        let d = delay.min(self.len);
        let i = if self.pos >= d {
            self.pos - d
        } else {
            self.pos + self.len - d
        };
        mem[self.at + i]
    }

    /// Like [`Line::read`], between samples.
    #[inline]
    fn read_frac(&self, mem: &[f32], delay: f32) -> f32 {
        let d = delay.clamp(1.0, (self.len - 1) as f32);
        let i = d as usize;
        let f = d - i as f32;
        let a = self.read(mem, i);
        let b = self.read(mem, i + 1);
        a + (b - a) * f
    }

    #[inline]
    fn write(&mut self, mem: &mut [f32], v: f32) {
        mem[self.at + self.pos] = v;
        self.pos += 1;
        if self.pos == self.len {
            self.pos = 0;
        }
    }

    /// A Schroeder allpass with this line as its delay: returns the output.
    #[inline]
    fn allpass(&mut self, mem: &mut [f32], x: f32, g: f32) -> f32 {
        let d = self.read(mem, self.len);
        let v = x - g * d;
        self.write(mem, v);
        d + g * v
    }
}

/// Jon Dattorro's plate reverb ("Effect Design, Part 1", JAES 1997), the
/// algorithm Mixxx's reverb uses too: input diffusers into a figure-eight
/// tank of modulated allpasses, damped delays and decay, with the stereo
/// output tapped from several points of the tank. Smoother and less
/// metallic than Freeverb's parallel combs.
struct Reverb {
    /// Four input diffusers, then per tank half: modulated allpass, delay,
    /// allpass, delay.
    diffusers: [Line; 4],
    tank: [[Line; 4]; 2],
    /// Buffer space all of the above need.
    size: usize,
    /// The tunings, rescaled from the paper's 29.761 kHz.
    scale: f32,
    lfo: f32,
    lfo_step: f32,
    bandwidth: f32,
    input: f32,
    damping: f32,
    damped: [f32; 2],
    decay: f32,
    send: f32,
    /// The tank's output from the other half, fed round the figure eight.
    cross: [f32; 2],
}

/// Dattorro's delay lengths, at 29 761 Hz.
const DIFFUSER_TUNING: [usize; 4] = [142, 107, 379, 277];
const TANK_TUNING: [[usize; 4]; 2] = [[672, 4453, 1800, 3720], [908, 4217, 2656, 3163]];
/// The modulated allpasses swing this many samples (at 29 761 Hz) each way.
const TANK_EXCURSION: f32 = 16.0;

impl Reverb {
    fn new(sample_rate: f32) -> Self {
        let scale = sample_rate / 29_761.0;
        let mut offset = 0;
        let mut place = |len: usize, extra: usize| {
            let len = ((len as f32 * scale) as usize).max(1) + extra;
            let line = Line {
                at: offset,
                len,
                pos: 0,
            };
            offset += len;
            line
        };
        let diffusers = DIFFUSER_TUNING.map(|l| place(l, 0));
        let excursion = (TANK_EXCURSION * scale) as usize + 2;
        let tank = TANK_TUNING.map(|half| {
            [
                place(half[0], excursion),
                place(half[1], 0),
                place(half[2], 0),
                place(half[3], 0),
            ]
        });
        Self {
            diffusers,
            tank,
            size: offset,
            scale,
            lfo: 0.0,
            lfo_step: 1.0 / sample_rate,
            bandwidth: 0.9995,
            input: 0.0,
            damping: 0.0005,
            damped: [0.0; 2],
            decay: 0.5,
            send: 0.0,
            cross: [0.0; 2],
        }
    }

    fn reset(&mut self, mem: &mut [f32]) {
        mem[..self.size].fill(0.0);
        self.input = 0.0;
        self.damped = [0.0; 2];
        self.cross = [0.0; 2];
    }

    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS]) {
        // Decay: a short room to a long hall.
        self.decay = 0.2 + 0.77 * p[0];
        self.bandwidth = 0.1 + 0.8995 * p[1];
        self.damping = 0.8 * p[2];
        self.send = p[3];
    }

    #[inline]
    fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        self.input += ((x[0] + x[1]) * 0.5 - self.input) * self.bandwidth;
        let mut v = self.input;
        for (d, g) in self.diffusers.iter_mut().zip([0.75, 0.75, 0.625, 0.625]) {
            v = d.allpass(mem, v, g);
        }
        // About 1 Hz of chorus in the tank keeps the tail from ringing.
        self.lfo += self.lfo_step;
        if self.lfo >= 1.0 {
            self.lfo -= 1.0;
        }
        let (sin, cos) = (self.lfo * TAU).sin_cos();
        let swing = TANK_EXCURSION * self.scale;
        let mut out = [0.0f32; 2];
        for half in 0..2 {
            let mod_ = if half == 0 { sin } else { cos };
            let [ap1, d1, ap2, d2] = &mut self.tank[half];
            // The decay diffuser: an allpass whose length swings with the LFO.
            let input = v + self.cross[1 - half] * self.decay;
            let base = ap1.len as f32 - swing - 1.0;
            let delayed = ap1.read_frac(mem, base + swing * mod_);
            let w = input + 0.7 * delayed;
            ap1.write(mem, w);
            let a = delayed - 0.7 * w;
            d1.write(mem, a);
            let t = d1.read(mem, d1.len);
            self.damped[half] += (t - self.damped[half]) * (1.0 - self.damping);
            let b = ap2.allpass(mem, self.damped[half] * self.decay, -0.5);
            d2.write(mem, b);
            self.cross[half] = d2.read(mem, d2.len);
        }
        let s = self.scale;
        let at = |l: &Line, n: usize| l.read(mem, ((n as f32 * s) as usize).max(1));
        let [_, l2, l3, l4] = &self.tank[0];
        let [_, r2, r3, r4] = &self.tank[1];
        out[0] = at(r2, 266) + at(r2, 2974) - at(r3, 1913) + at(r4, 1996)
            - at(l2, 1990)
            - at(l3, 187)
            - at(l4, 1066);
        out[1] = at(l2, 353) + at(l2, 3627) - at(l3, 1228) + at(l4, 2673)
            - at(r2, 2111)
            - at(r3, 335)
            - at(r4, 121);
        [
            x[0] + out[0] * 0.6 * self.send,
            x[1] + out[1] * 0.6 * self.send,
        ]
    }
}

/// Resonant low-pass and high-pass in series, like Mixxx's Filter effect,
/// but with state-variable filters that glide instead of jumping each buffer.
struct Filter {
    sample_rate: f32,
    lp: Svf,
    hp: Svf,
}

impl Filter {
    fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            lp: Svf::default(),
            hp: Svf::default(),
        }
    }

    fn reset(&mut self) {
        self.lp = Svf::default();
        self.hp = Svf::default();
    }

    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS]) {
        let [lp, res, hp] = [p[0], p[1], p[2]];
        // Low-pass 20 Hz..20 kHz, high-pass 20 Hz..10 kHz.
        let lp_hz = 20.0 * 1000f32.powf(lp);
        let hp_hz = 20.0 * 500f32.powf(hp);
        let mut q = 0.5 + 4.0 * res * res;
        // Two resonant peaks close together would add up: keep it tamer then.
        if lp < 0.995 && hp > 0.005 && lp_hz < 4.0 * hp_hz {
            q = q.min(1.0);
        }
        self.lp.set(self.sample_rate, lp_hz, q);
        self.hp.set(self.sample_rate, hp_hz, q);
    }

    #[inline]
    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        let y = self.lp.process(x)[0];
        self.hp.process(y)[2]
    }
}

/// A short swept delay with feedback; the channels' sweeps are 90° apart.
struct Flanger {
    len: usize,
    write: usize,
    phase: f32,
    step: f32,
    /// Centre delay in samples.
    centre: f32,
    depth: f32,
    feedback: f32,
}

impl Flanger {
    fn new(sample_rate: f32) -> Self {
        Self {
            len: (FLANGER_MAX_SECONDS * sample_rate) as usize + 4,
            write: 0,
            phase: 0.0,
            step: 0.0,
            centre: 1.0,
            depth: 0.0,
            feedback: 0.0,
        }
    }

    fn reset(&mut self, mem: &mut [f32]) {
        mem[..2 * self.len].fill(0.0);
        self.write = 0;
        self.phase = 0.0;
    }

    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        // Speed: one sweep every 32 beats down to every half beat, in time
        // with the track. Manual: 0.5..8 ms centre delay.
        self.step = 1.0 / (beats(1.0 - p[0], -1, 5) * ctx.beat_seconds * ctx.sample_rate);
        self.depth = p[1];
        self.centre = (0.0005 + 0.0075 * p[2]) * ctx.sample_rate;
        self.feedback = 0.9 * p[3];
    }

    #[inline]
    fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        self.phase += self.step;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        let lfo = (self.phase * TAU).sin_cos();
        let (l, r) = mem[..2 * self.len].split_at_mut(self.len);
        let mut y = [0.0; 2];
        for (c, (buf, sweep)) in [(l, lfo.0), (r, lfo.1)].into_iter().enumerate() {
            let d = tap(buf, self.write, self.centre * (1.0 + 0.9 * sweep));
            buf[self.write] = x[c] + soft(d * self.feedback);
            y[c] = x[c] * (1.0 - 0.5 * self.depth) + d * 0.5 * self.depth;
        }
        self.write += 1;
        if self.write == self.len {
            self.write = 0;
        }
        y
    }
}

/// Reduces bit depth and sample rate.
struct Bitcrusher {
    step: f32,
    ratio: f32,
    acc: f32,
    held: [f32; 2],
}

impl Default for Bitcrusher {
    fn default() -> Self {
        Self {
            step: 0.0,
            ratio: 1.0,
            acc: 1.0,
            held: [0.0; 2],
        }
    }
}

impl Bitcrusher {
    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS]) {
        // Bit depth: 2..16 bits. Downsampling: 1 keeps every sample, 0 keeps one in 50.
        let bits = 2.0 + 14.0 * p[0];
        self.step = 2.0 / 2f32.powf(bits);
        self.ratio = 0.02 + 0.98 * p[1] * p[1];
    }

    #[inline]
    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        self.acc += self.ratio;
        if self.acc >= 1.0 {
            self.acc -= 1.0;
            self.held = x;
        }
        self.held.map(|v| (v / self.step).round() * self.step)
    }
}

const PHASER_MAX_STAGES: usize = 12;
/// Frames between updates of the phaser's sweep.
const PHASER_UPDATE: u32 = 16;

/// A chain of swept first-order allpasses mixed with the input.
#[derive(Default)]
struct Phaser {
    phase: f32,
    step: f32,
    stages: usize,
    depth: f32,
    feedback: f32,
    sample_rate: f32,
    coef: [f32; 2],
    state: [[f32; PHASER_MAX_STAGES]; 2],
    last: [f32; 2],
    countdown: u32,
}

impl Phaser {
    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        // Speed: one sweep every 32 beats down to every half beat, in time
        // with the track. Stages: 2..12.
        self.step = 1.0 / (beats(1.0 - p[0], -1, 5) * ctx.beat_seconds * ctx.sample_rate);
        self.depth = p[1];
        self.feedback = 0.9 * p[2];
        let stages = 2 + 2 * (p[3] * 5.0).round() as usize;
        // Added stages start silent rather than with stale state, so they
        // don't click in.
        for state in &mut self.state {
            for z in state.iter_mut().take(stages).skip(self.stages) {
                *z = 0.0;
            }
        }
        self.stages = stages;
        self.sample_rate = ctx.sample_rate;
    }

    #[inline]
    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        self.phase += self.step;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        if self.countdown == 0 {
            self.countdown = PHASER_UPDATE;
            let (s, c) = (self.phase * TAU).sin_cos();
            for (coef, lfo) in self.coef.iter_mut().zip([s, c]) {
                // Notches sweep 200 Hz..3 kHz.
                let f = 200.0 * 15f32.powf((1.0 + lfo) * 0.5);
                let t = (PI * f / self.sample_rate).tan();
                *coef = (t - 1.0) / (t + 1.0);
            }
        }
        self.countdown -= 1;
        let mut y = [0.0; 2];
        for c in 0..2 {
            let a = self.coef[c];
            let mut v = x[c] + self.last[c] * self.feedback;
            for z in &mut self.state[c][..self.stages] {
                let out = a * v + *z;
                *z = v - a * out;
                v = out;
            }
            self.last[c] = v;
            y[c] = x[c] * (1.0 - 0.5 * self.depth) + v * 0.5 * self.depth;
        }
        y
    }
}

/// Trans / gate: chops the sound in time with the beat, with a short slew
/// on each edge so it doesn't click. Its phase starts on the beat it is
/// turned on.
struct Gate {
    phase: f32,
    step: f32,
    depth: f32,
    width: f32,
    gain: f32,
    slew: f32,
}

impl Default for Gate {
    fn default() -> Self {
        Self {
            phase: 0.0,
            step: 0.0,
            depth: 0.0,
            width: 0.5,
            gain: 1.0,
            slew: 1.0,
        }
    }
}

impl Gate {
    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        // Length: 1/16 to 1 beat per open-and-close.
        self.step = 1.0 / (beats(p[0], -4, 0) * ctx.beat_seconds * ctx.sample_rate);
        self.depth = p[1];
        self.width = 0.1 + 0.8 * p[2];
        // Each edge takes about 2 ms.
        self.slew = 1.0 / (0.002 * ctx.sample_rate);
    }

    #[inline]
    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        self.phase += self.step;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        let target = if self.phase < self.width {
            1.0
        } else {
            1.0 - self.depth
        };
        self.gain += (target - self.gain).clamp(-self.slew, self.slew);
        [x[0] * self.gain, x[1] * self.gain]
    }
}

/// Beat roll: records from the moment it is turned on and repeats the first
/// slice of 1/16 to 1 beat, each repeat faded in and out over 3 ms so the
/// seams don't click (Mixxx's Glitch repeats raw audio buffers instead).
struct Roll {
    len: usize,
    recorded: usize,
    slice: usize,
    pos: usize,
    fade: f32,
}

impl Roll {
    fn new(len: usize) -> Self {
        Self {
            len,
            recorded: 0,
            slice: 1,
            pos: 0,
            fade: 1.0,
        }
    }

    fn reset(&mut self) {
        self.recorded = 0;
        self.pos = 0;
    }

    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        // Rate: 0 rolls whole beats, 1 rolls 1/16 beats.
        let b = beats(1.0 - p[0], -4, 0);
        self.slice = ((b * ctx.beat_seconds * ctx.sample_rate) as usize).clamp(64, self.len - 1);
        self.fade = 0.003 * ctx.sample_rate;
    }

    #[inline]
    fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        let (l, r) = mem[..2 * self.len].split_at_mut(self.len);
        if self.recorded < self.len {
            l[self.recorded] = x[0];
            r[self.recorded] = x[1];
            self.recorded += 1;
        }
        // Until the first slice is in, the roll plays it live.
        if self.recorded < self.slice {
            return x;
        }
        let slice = self.slice.min(self.recorded);
        if self.pos >= slice {
            self.pos = 0;
        }
        let p = self.pos;
        self.pos += 1;
        let env = (p as f32 / self.fade)
            .min((slice - p) as f32 / self.fade)
            .min(1.0);
        [l[p] * env, r[p] * env]
    }
}

/// Echo out: catches the sound for a few beats and lets the echoes ring out
/// while the original fades away, for ending a track or a hard cut.
struct EchoOut {
    echo: Echo,
    /// Samples since it was turned on, and how long it listens for.
    t: f32,
    capture: f32,
}

impl EchoOut {
    fn new(sample_rate: f32) -> Self {
        Self {
            echo: Echo::new(sample_rate),
            t: 0.0,
            capture: 1.0,
        }
    }

    fn reset(&mut self, mem: &mut [f32]) {
        self.echo.reset(mem);
        self.t = 0.0;
    }

    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        // Time: 1/4 to 1 beat. Length: the original fades out over 1/2 to 4 beats.
        self.echo.set_time(beats(p[0], -2, 0), ctx);
        self.echo.feedback_target = 0.5 + 0.45 * p[1];
        self.echo.pingpong = 0.3;
        self.capture = beats(p[2], -1, 2) * ctx.beat_seconds * ctx.sample_rate;
    }

    #[inline]
    fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        let left = 1.0 - (self.t / self.capture).min(1.0);
        self.t += 1.0;
        self.echo.send = left;
        let y = self.echo.process(mem, x);
        // The echo's output carries the dry signal: take away what has faded.
        [y[0] - x[0] * (1.0 - left), y[1] - x[1] * (1.0 - left)]
    }
}

/// Vinyl brake: slows the sound to a stop over 1 to 8 beats, pitch falling
/// with it, by playing back what it records at a falling speed.
struct Brake {
    len: usize,
    write: usize,
    /// How far behind the newest sample the playback has fallen.
    lag: f32,
    speed: f32,
    decel: f32,
}

impl Brake {
    fn new(len: usize) -> Self {
        Self {
            len,
            write: 0,
            lag: 0.0,
            speed: 1.0,
            decel: 0.0,
        }
    }

    fn reset(&mut self, mem: &mut [f32]) {
        mem[..2 * self.len].fill(0.0);
        self.write = 0;
        self.lag = 0.0;
        self.speed = 1.0;
    }

    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        // Playback falls behind by half the stop time, which the buffer
        // (2 s) has to hold.
        let seconds = (beats(p[0], 0, 3) * ctx.beat_seconds).min(2.0 * ECHO_MAX_SECONDS - 0.2);
        self.decel = 1.0 / (seconds * ctx.sample_rate);
    }

    #[inline]
    fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        let (l, r) = mem[..2 * self.len].split_at_mut(self.len);
        l[self.write] = x[0];
        r[self.write] = x[1];
        self.write += 1;
        if self.write == self.len {
            self.write = 0;
        }
        if self.speed <= 0.0 {
            return [0.0; 2];
        }
        let delay = (1.0 + self.lag).min((self.len - 2) as f32);
        // Fade the last moment so the stop doesn't click.
        let g = (self.speed * 20.0).min(1.0);
        let y = [tap(l, self.write, delay) * g, tap(r, self.write, delay) * g];
        self.lag += 1.0 - self.speed;
        self.speed -= self.decel;
        y
    }
}

/// Noise sweep for build-ups: band-passed noise over the music, its level
/// and pitch rising together on the meta knob.
#[derive(Default)]
struct Noise {
    seed: u32,
    level: f32,
    gain: f32,
    filter: Svf,
}

impl Noise {
    fn reset(&mut self) {
        self.filter = Svf::default();
        self.gain = 0.0;
        if self.seed == 0 {
            self.seed = 0x2545_f491;
        }
    }

    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        self.level = 0.3 * p[0];
        // Sweep: the noise band from 300 Hz to 12 kHz. Resonance narrows it.
        self.filter
            .set(ctx.sample_rate, 300.0 * 40f32.powf(p[1]), 0.6 + 4.0 * p[2]);
    }

    #[inline]
    fn next(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        self.seed as f32 / u32::MAX as f32 * 2.0 - 1.0
    }

    #[inline]
    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        self.gain += (self.level - self.gain) * 0.002;
        let n = [self.next(), self.next()];
        let band = self.filter.process(n)[1];
        // The band-pass output peaks at 1/k; scale it back to the noise level.
        let k = self.filter.k;
        [
            x[0] + band[0] * k * self.gain,
            x[1] + band[1] * k * self.gain,
        ]
    }
}

/// Spiral: an echo whose repeats come faster and faster, so the tail climbs
/// in pitch, after Pioneer's effect of that name.
struct Spiral {
    echo: Echo,
    /// The echo time it starts from, in samples, and samples since it was turned on.
    base: f32,
    t: f32,
    /// Samples for the echo time to halve.
    halving: f32,
}

impl Spiral {
    fn new(sample_rate: f32) -> Self {
        Self {
            echo: Echo::new(sample_rate),
            base: 1.0,
            t: 0.0,
            halving: 1.0,
        }
    }

    fn reset(&mut self, mem: &mut [f32]) {
        self.echo.reset(mem);
        self.t = 0.0;
    }

    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        // Time: 1/4 to 1 beat to start from; it halves every 4 beats.
        self.echo.set_time(beats(p[0], -2, 0), ctx);
        self.base = self.echo.target;
        self.echo.feedback_target = 0.4 + 0.5 * p[1];
        self.echo.pingpong = 0.5;
        self.echo.send = p[2];
        self.halving = 4.0 * ctx.beat_seconds * ctx.sample_rate;
    }

    #[inline]
    fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        // Down to an eighth of the starting time, then it holds there.
        self.echo.target = (self.base * (-self.t / self.halving).exp2()).max(self.base / 8.0);
        self.t += 1.0;
        self.echo.process(mem, x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;
    const CTX: Ctx = Ctx {
        sample_rate: SR,
        beat_seconds: 0.5,
    };

    /// An instance of `kind` with its default parameters, fed `input` frames.
    fn run(kind: Kind, input: impl Fn(usize) -> f32, frames: usize) -> Vec<f32> {
        let mut inst = Instance::new(SR);
        inst.kind = Some(kind);
        inst.reset();
        let defaults = MANIFESTS[kind as usize].defaults;
        let p = std::array::from_fn(|k| defaults.get(k).copied().unwrap_or(0.5) as f32);
        inst.prepare(&p, CTX);
        (0..frames)
            .map(|i| {
                let v = input(i);
                inst.process([v, v])[0]
            })
            .collect()
    }

    fn tone(i: usize) -> f32 {
        (TAU * 220.0 * i as f32 / SR).sin() * 0.5
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    #[test]
    fn the_beat_switch_sets_each_effects_length() {
        let reg = ControlRegistry::new();
        let mut slot = SlotControls::create(&reg, 0, 0);
        let set = |k: &str, v: f64| {
            reg.get("[EffectRack1_EffectUnit1_Effect1]", k)
                .unwrap()
                .set(v)
        };
        let get = |k: &str| {
            reg.get("[EffectRack1_EffectUnit1_Effect1]", k)
                .unwrap()
                .get()
        };
        // Echo: 1/16 to 4 beats, all of them.
        set("loaded_effect", 1.0);
        slot.update();
        // A new effect starts at 1/4 beat.
        assert_eq!(get("beats"), 0.25);
        for want in [0.0625, 0.5, 4.0] {
            set("beats", want);
            slot.update();
            assert_eq!(get("beats"), want);
            assert_eq!(beats(get("parameter1") as f32, -4, 2) as f64, want);
        }
        // Flanger sweeps 1/2 beat at the shortest; the switch stops there.
        set("loaded_effect", 4.0);
        slot.update();
        set("beats", 0.125);
        slot.update();
        assert_eq!(get("beats"), 0.5);
        assert_eq!(beats(1.0 - get("parameter1") as f32, -1, 5), 0.5);
        // Reverb has no length.
        set("loaded_effect", 2.0);
        slot.update();
        assert_eq!(get("beats"), 0.0);
    }

    #[test]
    fn beat_fx_taps_a_tempo_and_releases_into_its_tail() {
        let reg = ControlRegistry::new();
        let mut rack = EffectRack::new(&reg, SR as f64);
        let unit = |k: &str| reg.get("[EffectRack1_EffectUnit5]", k).unwrap();
        let slot = |k: &str| reg.get("[EffectRack1_EffectUnit5_Effect1]", k).unwrap();
        let beats = [0.5; TARGETS];
        // Routed to both decks, with Echo loaded.
        assert!(unit("group_[Channel1]_enable").get_bool());
        assert!(unit("group_[Channel2]_enable").get_bool());
        assert_eq!(slot("loaded_effect").get(), 1.0);

        // Taps half a second apart: 120 BPM; AUTO goes back to the decks.
        for i in 0..4 {
            unit("bpm_tap").set(1.0);
            rack.prepare(beats, i as f64 * 0.5);
            unit("bpm_tap").set(0.0);
            rack.prepare(beats, i as f64 * 0.5 + 0.01);
        }
        assert!((unit("bpm").get() - 120.0).abs() < 1e-6);
        unit("bpm_auto").set(1.0);
        rack.prepare(beats, 3.0);
        unit("bpm_auto").set(0.0);
        assert_eq!(unit("bpm").get(), 0.0);

        // Echo on deck 1, then released: the deck plays dry again with the
        // echoes of what came before ringing on, and later they end.
        let buffer = |rack: &mut EffectRack, input: f32, now: f64| {
            rack.prepare(beats, now);
            let out: Vec<f32> = (0..480)
                .map(|i| {
                    let v = input * tone(i);
                    rack.process(0, [v, v], (i + 1) as f32 / 480.0)[0]
                })
                .collect();
            rack.finish();
            out
        };
        slot("meta").set(0.8);
        slot("enabled").set_bool(true);
        for i in 0..100 {
            buffer(&mut rack, 1.0, 4.0 + i as f64 * 0.01);
        }
        unit("release").set(1.0);
        let released = buffer(&mut rack, 0.0, 5.0);
        unit("release").set(0.0);
        assert!(!slot("enabled").get_bool(), "release turns the effect off");
        let mut later = Vec::new();
        for i in 0..50 {
            later.extend(buffer(&mut rack, 0.0, 5.01 + i as f64 * 0.01));
        }
        assert!(rms(&released) + rms(&later) > 0.01, "the echoes ring on");
        // Eight beats at 120 BPM is 4 s: well after that, silence.
        let mut end = Vec::new();
        for i in 0..500 {
            end = buffer(&mut rack, 0.0, 6.0 + i as f64 * 0.01);
        }
        assert!(rms(&end) < 1e-6, "the tail ends: {}", rms(&end));
    }

    #[test]
    fn brake_comes_to_a_stop() {
        // Default length: 2 beats, 1 s at 120 BPM.
        let out = run(Kind::Brake, tone, 72_000);
        assert!(rms(&out[..4_800]) > 0.3);
        assert_eq!(rms(&out[60_000..]), 0.0);
    }

    #[test]
    fn roll_repeats_its_first_slice() {
        // A click at the start, then silence: rolled, the click comes back.
        let out = run(
            Kind::Roll,
            |i| {
                if i < 10 {
                    0.0
                } else if i < 400 {
                    0.8
                } else {
                    0.0
                }
            },
            48_000,
        );
        let late = out[24_000..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(late > 0.5, "{late}");
    }

    #[test]
    fn echo_out_fades_the_original_and_keeps_the_tail() {
        let out = run(
            Kind::EchoOut,
            |i| if i < 24_000 { tone(i) } else { 0.0 },
            96_000,
        );
        // Still echoing after the input stopped.
        assert!(rms(&out[30_000..40_000]) > 0.02);
        assert!(out.iter().all(|v| v.is_finite() && v.abs() < 1.5));
    }

    #[test]
    fn gate_chops_in_time() {
        let out = run(Kind::Gate, |_| 0.5, 48_000);
        let lo = out.iter().fold(1.0f32, |m, v| m.min(v.abs()));
        let hi = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(lo < 0.15 && hi > 0.45, "{lo} {hi}");
    }

    #[test]
    fn reverb_tail_decays_and_stays_finite() {
        let out = run(
            Kind::Reverb,
            |i| if i < 480 { tone(i) } else { 0.0 },
            5 * 48_000,
        );
        assert!(rms(&out[4_800..9_600]) > 0.001, "a tail after the input");
        assert!(
            rms(&out[4 * 48_000..]) < rms(&out[4_800..9_600]),
            "that decays"
        );
        assert!(out.iter().all(|v| v.is_finite()));
    }
}
