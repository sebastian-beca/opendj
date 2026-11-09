//! The microphone and auxiliary inputs, Mixxx's `[Microphone]` and
//! `[Auxiliary1]` to `[Auxiliary3]`, and the instrument, `[Instrument1]`.
//! The microphone and auxiliaries take
//! audio from an input device (see [`crate::output`]), the instrument from an
//! instrument plugin playing the notes it is sent (see
//! [`crate::plugins`]). Each goes through its gain, a 3-band EQ and the
//! effect units routed to it, then its fader.
//!
//! The microphone is heard while `talkover` is on. It joins the main mix
//! after the main mix's effects, so they don't touch the voice, and can duck
//! the music while the DJ talks (`[Master],talkover_ducking`). The auxiliary
//! input joins the music while `main_mix` is on. Both can be cued in the
//! headphones, and both are in recordings, which take the main mix.
//!
//! An input device runs on its own clock, so its audio comes through a queue
//! that the engine keeps short: it waits for a callback's worth before
//! playing, drops audio when the queue grows, and pads with silence when
//! it runs dry.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use opendj_control::{Behavior, ControlHandle, ControlRegistry};

use crate::controls::alias;
use crate::dsp::Isolator;
use crate::effects::EffectRack;

pub const NUM_INPUTS: usize = 5;
pub const MICROPHONE: usize = 0;
pub const AUXILIARY: usize = 1;
/// The instrument plugin, joining the music as the auxiliary inputs do.
pub const INSTRUMENT: usize = 2;
/// The second and third auxiliary inputs, after the instrument so the
/// first three keep their numbers.
pub const AUXILIARY_2: usize = 3;
pub const AUXILIARY_3: usize = 4;
/// The auxiliary inputs, in order.
pub const AUXILIARIES: [usize; 3] = [AUXILIARY, AUXILIARY_2, AUXILIARY_3];

/// Knobs the instrument's plugin parameters show on:
/// `[Instrument1],parameter1` to `parameter32`, 0 to 1 across each one's
/// own range. As many as the plugin host shares with a helper.
pub const INSTRUMENT_PARAMETERS: usize = opendj_plugin_host::shm::MAX_PARAMS;

/// The input's control group.
pub const fn input_group(input: usize) -> &'static str {
    [
        "[Microphone]",
        "[Auxiliary1]",
        "[Instrument1]",
        "[Auxiliary2]",
        "[Auxiliary3]",
    ][input]
}

/// The input's EQ, named as the decks' are.
pub fn input_eq_group(input: usize) -> String {
    format!("[EqualizerRack1_{}_Effect1]", input_group(input))
}

/// The engine's end of an input device's queue: stereo frames, interleaved.
pub(crate) struct InputFeed {
    pub(crate) rx: rtrb::Consumer<f32>,
    /// Frames the device delivered in its last callback.
    pub(crate) period: Arc<AtomicUsize>,
}

/// `[Master],talkover_ducking`, as in Mixxx: off, ducking while the
/// microphone picks up a voice, or for as long as talkover is on.
pub const DUCK_OFF: f64 = 0.0;
pub const DUCK_AUTO: f64 = 1.0;
pub const DUCK_MANUAL: f64 = 2.0;
/// How much ducking takes off the music by default: 12 dB.
pub const DEFAULT_DUCK_STRENGTH: f64 = 0.75;

/// A microphone peak above this (about -30 dBFS) counts as talking.
const DUCK_THRESHOLD: f32 = 0.03;
/// Seconds the music stays down after the last loud peak, so it doesn't
/// pump between words.
const DUCK_HOLD: f32 = 0.5;
/// Time constants of the music going down and coming back up, in seconds.
const DUCK_ATTACK: f32 = 0.03;
const DUCK_RELEASE: f32 = 0.6;

/// The Instrument input's pads, `pad_1` to `pad_16`: the white keys from
/// middle C up, moved by `octave`, for the Keyboard pad page and
/// controllers.
pub const INSTRUMENT_PADS: usize = 16;
/// The notes of the pads in the octave from C: a major scale.
const SCALE: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];
/// Middle C.
const MIDDLE_C: i32 = 60;

/// The MIDI note pad `n` (from 0) plays, `octave` octaves up or down.
pub fn pad_note(n: usize, octave: i32) -> u8 {
    let note = MIDDLE_C + 12 * (octave + (n / SCALE.len()) as i32) + SCALE[n % SCALE.len()] as i32;
    note.clamp(0, 127) as u8
}

/// The instrument's pads and the notes they hold.
struct Pads {
    pads: [ControlHandle; INSTRUMENT_PADS],
    octave: ControlHandle,
    /// `keyboard_pads`: the decks' KEYBOARD pads play the first eight.
    keyboard_pads: ControlHandle,
    /// Each deck's `keyboard_N_activate`.
    decks: [[ControlHandle; 8]; crate::NUM_DECKS],
    /// The note each held pad started, so it stops that note even when the
    /// octave moved meanwhile.
    held: [Option<u8>; INSTRUMENT_PADS],
}

impl Pads {
    fn create(reg: &ControlRegistry, g: &str) -> Self {
        Self {
            pads: std::array::from_fn(|n| reg.push(g, &format!("pad_{}", n + 1))),
            octave: reg.create(g, "octave", 0.0, -3.0, 3.0, Behavior::Free),
            keyboard_pads: reg.toggle(g, "keyboard_pads"),
            decks: std::array::from_fn(|d| {
                let dg = crate::controls::deck_group(d);
                std::array::from_fn(|n| reg.push(&dg, &format!("keyboard_{}_activate", n + 1)))
            }),
            held: [None; INSTRUMENT_PADS],
        }
    }

    /// Sends a note on for each pad pressed and a note off for each let go.
    fn play(&mut self, fx: &mut EffectRack) {
        let octave = self.octave.get().round() as i32;
        let keyboard = self.keyboard_pads.get_bool();
        for (n, (pad, held)) in self.pads.iter().zip(&mut self.held).enumerate() {
            let deck_pad = keyboard && n < 8 && self.decks.iter().any(|d| d[n].get_bool());
            match (pad.get_bool() || deck_pad, *held) {
                (true, None) => {
                    let note = pad_note(n, octave);
                    fx.queue_note([0x90, note, 100]);
                    *held = Some(note);
                }
                (false, Some(note)) => {
                    fx.queue_note([0x80, note, 0]);
                    *held = None;
                }
                _ => {}
            }
        }
    }
}

/// How fast meters fall, in meter units per second, as the decks' do.
const VU_FALL: f32 = 1.5;
/// Seconds a clip light stays on.
const PEAK_HOLD: f32 = 0.5;

struct InputControls {
    /// `talkover` for the microphone, `main_mix` for the auxiliary input.
    on: ControlHandle,
    pregain: ControlHandle,
    volume: ControlHandle,
    mute: ControlHandle,
    pfl: ControlHandle,
    configured: ControlHandle,
    eq: [ControlHandle; 3],
    eq_kill: [ControlHandle; 3],
    vu: ControlHandle,
    vu_lr: [ControlHandle; 2],
    peak: ControlHandle,
}

impl InputControls {
    fn create(reg: &ControlRegistry, input: usize) -> Self {
        let g = input_group(input);
        let eqg = input_eq_group(input);
        alias(reg, g, "peak_indicator", g, "PeakIndicator");
        alias(reg, g, "vu_meter", g, "VuMeter");
        alias(reg, g, "vu_meter_left", g, "VuMeterL");
        alias(reg, g, "vu_meter_right", g, "VuMeterR");
        // Mixxx's microphones and auxiliaries can be put on a crossfader
        // side; here they always bypass it, so this is stored only.
        reg.create(g, "orientation", 1.0, 0.0, 2.0, Behavior::Free);
        reg.indicator(&eqg, "num_parameters", 3.0).set(3.0);
        reg.indicator(&eqg, "num_button_parameters", 3.0).set(3.0);
        let on = if input == MICROPHONE {
            reg.toggle(g, "talkover")
        } else {
            // Mixxx 2.4 renamed `master` to `main_mix`; mappings use both.
            // Off until the DJ turns it on, as the microphone is.
            let on = reg.toggle(g, "main_mix");
            alias(reg, g, "master", g, "main_mix");
            on
        };
        if input == INSTRUMENT {
            // Which instrument plays: 1 + its place in the plugin host's
            // instrument list, 0 for none.
            reg.create(
                g,
                "instrument",
                0.0,
                0.0,
                crate::effects::MAX_PLUGINS as f64,
                Behavior::Linear,
            );
            // The instrument's own window, as an effect slot's `show_editor`.
            reg.toggle(g, "show_editor");
            // Its first parameters (see `INSTRUMENT_PARAMETERS`), and how
            // many it has, as an effect slot's.
            for k in 1..=INSTRUMENT_PARAMETERS {
                reg.create(g, &format!("parameter{k}"), 0.0, 0.0, 1.0, Behavior::Linear);
            }
            reg.indicator(g, "num_parameters", INSTRUMENT_PARAMETERS as f64);
            // Scale mode (see `crate::scale`): Auto by default.
            reg.create(g, "scale_mode", 1.0, 0.0, 2.0, Behavior::Free);
            reg.create(g, "scale_root", 0.0, 0.0, 11.0, Behavior::Free);
            let types = (crate::scale::SCALES.len() - 1) as f64;
            reg.create(g, "scale_type", 0.0, 0.0, types, Behavior::Free);
            reg.indicator(g, "scale_mask", 4095.0);
            reg.indicator(g, "scale_now", (12 * crate::scale::SCALES.len()) as f64);
            // Chord mode: off by default; Mine is a major triad until one
            // is learned.
            let chords = (crate::scale::CHORDS.len() - 1) as f64;
            reg.create(g, "chord_mode", 0.0, 0.0, chords, Behavior::Free);
            let shape = crate::scale::DEFAULT_SHAPE as f64;
            reg.create(
                g,
                "chord_shape",
                shape,
                0.0,
                u32::MAX as f64,
                Behavior::Free,
            );
            reg.create(g, "chord_learn", 0.0, 0.0, 1.0, Behavior::Free);
            // The arpeggiator (see `crate::arp`): off, Up, 1/16, half gate.
            use crate::arp::{DEFAULT_RATE, MODES, RATES};
            reg.create(g, "arp", 0.0, 0.0, 1.0, Behavior::Free);
            reg.create(
                g,
                "arp_mode",
                0.0,
                0.0,
                (MODES.len() - 1) as f64,
                Behavior::Free,
            );
            let rate = DEFAULT_RATE as f64;
            reg.create(
                g,
                "arp_rate",
                rate,
                0.0,
                (RATES.len() - 1) as f64,
                Behavior::Free,
            );
            reg.create(g, "arp_gate", 0.5, 0.05, 1.0, Behavior::Linear);
            reg.create(g, "arp_octaves", 1.0, 1.0, 4.0, Behavior::Free);
            reg.create(g, "arp_hold", 0.0, 0.0, 1.0, Behavior::Free);
        }
        let configured = reg.indicator(g, "input_configured", 1.0);
        alias(reg, g, "enabled", g, "input_configured");
        Self {
            on,
            pregain: reg.create(g, "pregain", 1.0, 0.0, 4.0, Behavior::CenteredOnDefault),
            volume: reg.create(g, "volume", 1.0, 0.0, 1.0, Behavior::Linear),
            mute: reg.toggle(g, "mute"),
            pfl: reg.toggle(g, "pfl"),
            configured,
            eq: [1, 2, 3].map(|n| {
                reg.create(
                    &eqg,
                    &format!("parameter{n}"),
                    1.0,
                    0.0,
                    4.0,
                    Behavior::CenteredOnDefault,
                )
            }),
            eq_kill: [1, 2, 3].map(|n| reg.toggle(&eqg, &format!("button_parameter{n}"))),
            vu: reg.indicator(g, "VuMeter", 1.0),
            vu_lr: ["L", "R"].map(|s| reg.indicator(g, &format!("VuMeter{s}"), 1.0)),
            peak: reg.indicator(g, "PeakIndicator", 1.0),
        }
    }
}

/// One input channel.
pub(crate) struct Input {
    c: InputControls,
    index: usize,
    /// The instrument's pads; none for the device inputs.
    pads: Option<Pads>,
    /// The instrument's `parameterN` knobs; empty for the device inputs.
    params: Vec<ControlHandle>,
    /// The instrument's scale mode, root, type and the mask shown (see
    /// `crate::scale`); none for the device inputs.
    scale: Option<[ControlHandle; 5]>,
    /// Its chord mode, learned chord and learn switch.
    chord: Option<[ControlHandle; 3]>,
    /// Its arpeggiator: on, mode, rate, gate, octaves and hold.
    arp: Option<[ControlHandle; 6]>,
    /// The master deck's key, for Auto.
    master_key: Option<crate::key::Key>,
    feed: Option<InputFeed>,
    /// Waiting for the queue to fill before playing from it.
    filling: bool,
    eq: Isolator,
    eq_gains: [f32; 3],
    pregain: f32,
    level: f32,
    vu: [f32; 2],
    peak_hold: f32,
    /// This buffer's audio after gain, EQ and effects, before the fader.
    pub(crate) buf: Vec<[f32; 2]>,
    /// This buffer's audio as it came in, for a deck playing the input.
    pub(crate) raw: Vec<[f32; 2]>,
    /// The fader level at the start and end of this buffer: 0 while the
    /// input is off or muted.
    pub(crate) level_ramp: (f32, f32),
    /// Heard in the headphones.
    pub(crate) pfl: bool,
    /// On and connected: the microphone is talking over the music.
    pub(crate) live: bool,
}

impl Input {
    pub(crate) fn new(reg: &ControlRegistry, input: usize, sample_rate: f64) -> Self {
        Self {
            c: InputControls::create(reg, input),
            index: input,
            pads: (input == INSTRUMENT).then(|| Pads::create(reg, input_group(input))),
            scale: (input == INSTRUMENT).then(|| {
                [
                    "scale_mode",
                    "scale_root",
                    "scale_type",
                    "scale_mask",
                    "scale_now",
                ]
                .map(|k| {
                    reg.get(input_group(input), k)
                        .expect("the instrument's scale controls exist")
                })
            }),
            chord: (input == INSTRUMENT).then(|| {
                ["chord_mode", "chord_shape", "chord_learn"].map(|k| {
                    reg.get(input_group(input), k)
                        .expect("the instrument's chord controls exist")
                })
            }),
            arp: (input == INSTRUMENT).then(|| {
                [
                    "arp",
                    "arp_mode",
                    "arp_rate",
                    "arp_gate",
                    "arp_octaves",
                    "arp_hold",
                ]
                .map(|k| {
                    reg.get(input_group(input), k)
                        .expect("the instrument's arpeggiator controls exist")
                })
            }),
            master_key: None,
            params: if input == INSTRUMENT {
                (1..=INSTRUMENT_PARAMETERS)
                    .filter_map(|k| reg.get(input_group(input), &format!("parameter{k}")))
                    .collect()
            } else {
                Vec::new()
            },
            feed: None,
            filling: true,
            eq: Isolator::new(sample_rate),
            eq_gains: [1.0; 3],
            pregain: 1.0,
            level: 0.0,
            vu: [0.0; 2],
            peak_hold: 0.0,
            buf: vec![[0.0; 2]; 4096],
            raw: vec![[0.0; 2]; 4096],
            level_ramp: (0.0, 0.0),
            pfl: false,
            live: false,
        }
    }

    /// The master deck's key now, which scale mode Auto plays in.
    pub(crate) fn set_master_key(&mut self, key: Option<crate::key::Key>) {
        self.master_key = key;
    }

    /// Plays from `feed` from now on, or from nothing. Returns the old feed,
    /// to be dropped off the audio thread.
    pub(crate) fn set_feed(&mut self, feed: Option<InputFeed>) -> Option<InputFeed> {
        self.filling = true;
        self.c.configured.set_bool(feed.is_some());
        std::mem::replace(&mut self.feed, feed)
    }

    /// The Instrument input's first half of a buffer of `frames`: takes the
    /// scale, chord, arpeggiator, pads and knobs and hands the plugin its
    /// block, so it plays while the decks are worked out; [`Input::render`]
    /// picks the sound up. Nothing for the other inputs.
    pub(crate) fn start(
        &mut self,
        frames: usize,
        fx: &mut EffectRack,
        target: usize,
        on_deck: bool,
    ) {
        if self.index != INSTRUMENT {
            return;
        }
        if let Some([mode, root, kind, mask_shown, now_shown]) = &self.scale {
            let scale = crate::scale::scale_for(
                mode.get().round() as u8,
                root.get().round() as u8,
                kind.get().round() as usize,
                self.master_key,
            );
            let mask = scale.map_or(0, |(r, s)| crate::scale::mask(r, s));
            mask_shown.set(mask as f64);
            now_shown.set(scale.map_or(0.0, |(r, s)| (1 + s * 12 + r as usize) as f64));
            fx.set_scale(mask);
        }
        if let Some([mode, shape, learn]) = &self.chord {
            if let Some(learned) = fx.take_learned_chord() {
                shape.set(learned as f64);
                learn.set(0.0);
                mode.set(crate::scale::MINE as f64);
            }
            fx.set_chord(
                mode.get().round() as u8,
                shape.get().max(0.0) as u32,
                learn.get() > 0.5,
            );
        }
        if let Some([on, mode, rate, gate, octaves, hold]) = &self.arp {
            fx.set_arp(crate::arp::ArpSettings {
                on: on.get() > 0.5,
                mode: mode.get().round() as u8,
                rate: rate.get().round().max(0.0) as usize,
                gate: gate.get(),
                octaves: octaves.get().round() as u8,
                hold: hold.get() > 0.5,
            });
        }
        if let Some(pads) = &mut self.pads {
            pads.play(fx);
        }
        let mut params = [0.0f32; INSTRUMENT_PARAMETERS];
        for (p, c) in params.iter_mut().zip(&self.params) {
            *p = c.get() as f32;
        }
        fx.start_instrument(frames, target, &params, on_deck);
    }

    /// Takes in `frames` from the device and runs them through the gain,
    /// the EQ and the effects routed to `target`; the result is in `buf`.
    /// `dt` is the buffer's length in seconds. While `on_deck`, a deck
    /// plays the input (from `raw`) and its own channel is quiet.
    pub(crate) fn render(
        &mut self,
        frames: usize,
        fx: &mut EffectRack,
        target: usize,
        dt: f32,
        on_deck: bool,
    ) {
        if self.buf.len() < frames {
            self.buf.resize(frames, [0.0; 2]);
        }
        if self.raw.len() < frames {
            self.raw.resize(frames, [0.0; 2]);
        }
        let buf = &mut self.buf[..frames];
        let source = if self.index == INSTRUMENT {
            let playing = fx.finish_instrument(buf);
            self.c.configured.set_bool(fx.has_instrument());
            playing
        } else {
            let got = match &mut self.feed {
                Some(feed) => pull(feed, &mut self.filling, buf),
                None => 0,
            };
            buf[got..].fill([0.0; 2]);
            self.feed.is_some()
        };

        self.raw[..frames].copy_from_slice(buf);
        let on = self.c.on.get_bool() && !self.c.mute.get_bool() && source && !on_deck;
        let level = if on { self.c.volume.get() as f32 } else { 0.0 };
        self.level_ramp = (self.level, level);
        self.level = level;
        self.live = self.c.on.get_bool() && source && !on_deck;
        self.pfl = self.c.pfl.get_bool() && !on_deck;
        let pre = (self.pregain, self.c.pregain.get() as f32);
        self.pregain = pre.1;
        let eq_from = self.eq_gains;
        let eq_to: [f32; 3] = std::array::from_fn(|k| {
            if self.c.eq_kill[k].get_bool() {
                0.0
            } else {
                self.c.eq[k].get() as f32
            }
        });
        self.eq_gains = eq_to;

        for (f, s) in buf.iter_mut().enumerate() {
            let r = (f + 1) as f32 / frames as f32;
            let g = pre.0 + (pre.1 - pre.0) * r;
            let gains = std::array::from_fn(|k| eq_from[k] + (eq_to[k] - eq_from[k]) * r);
            *s = self.eq.process([s[0] * g, s[1] * g], gains);
        }
        fx.process_block(target, buf);
        let mut peak = [0.0f32; 2];
        for s in buf.iter() {
            for c in 0..2 {
                peak[c] = peak[c].max(s[c].abs());
            }
        }

        self.peak_hold = if peak.iter().any(|&p| p > 1.0) {
            PEAK_HOLD
        } else {
            (self.peak_hold - dt).max(0.0)
        };
        self.c.peak.set_bool(self.peak_hold > 0.0);
        for ((vu, p), control) in self.vu.iter_mut().zip(peak).zip(&self.c.vu_lr) {
            *vu = crate::engine::meter(p).max(*vu - VU_FALL * dt);
            control.set(*vu as f64);
        }
        self.c.vu.set(self.vu[0].max(self.vu[1]) as f64);
    }
}

/// Reads up to `out.len()` frames from `feed`, keeping its queue short.
/// Returns how many it read.
fn pull(feed: &mut InputFeed, filling: &mut bool, out: &mut [[f32; 2]]) -> usize {
    let frames = out.len();
    // Enough to cover one input callback and one output buffer: the queue
    // waits for this much, and is cut back to it when it grows past twice.
    let enough = feed.period.load(Ordering::Relaxed).max(frames);
    let queued = feed.rx.slots() / 2;
    if *filling {
        if queued < enough {
            return 0;
        }
        *filling = false;
    }
    if queued > 2 * enough + frames {
        let drop = queued - enough;
        if let Ok(chunk) = feed.rx.read_chunk(drop * 2) {
            chunk.commit_all();
        }
    }
    let n = (feed.rx.slots() / 2).min(frames);
    if let Ok(chunk) = feed.rx.read_chunk(n * 2) {
        let (a, b) = chunk.as_slices();
        for (k, v) in a.iter().chain(b).enumerate() {
            out[k / 2][k % 2] = *v;
        }
        chunk.commit_all();
    }
    if n < frames {
        *filling = true;
    }
    n
}

/// Turns the music down while the microphone talks over it.
pub(crate) struct Ducker {
    mode: ControlHandle,
    strength: ControlHandle,
    /// The music's gain now.
    gain: f32,
    /// Samples left before the music comes back up.
    hold: f32,
    hold_len: f32,
    attack: f32,
    release: f32,
}

impl Ducker {
    pub(crate) fn new(reg: &ControlRegistry, sample_rate: f64) -> Self {
        let sr = sample_rate as f32;
        let coef = |seconds: f32| 1.0 - (-1.0 / (seconds * sr)).exp();
        Self {
            mode: reg.create(
                "[Master]",
                "talkover_ducking",
                DUCK_AUTO,
                0.0,
                2.0,
                Behavior::Free,
            ),
            strength: reg.create(
                "[Master]",
                "duckStrength",
                DEFAULT_DUCK_STRENGTH,
                0.0,
                1.0,
                Behavior::Linear,
            ),
            gain: 1.0,
            hold: 0.0,
            hold_len: DUCK_HOLD * sr,
            attack: coef(DUCK_ATTACK),
            release: coef(DUCK_RELEASE),
        }
    }

    /// The mode and how far down the music goes (as a gain), read once per
    /// buffer.
    pub(crate) fn settings(&self) -> (f64, f32) {
        (
            self.mode.get().round(),
            1.0 - self.strength.get().clamp(0.0, 1.0) as f32,
        )
    }

    /// The music's gain for one frame, given the microphone's frame (before
    /// its fader) and whether it is talking over the music.
    #[inline]
    pub(crate) fn next(&mut self, (mode, low): (f64, f32), live: bool, mic: [f32; 2]) -> f32 {
        let loud = mic[0].abs().max(mic[1].abs()) > DUCK_THRESHOLD;
        if live && mode == DUCK_AUTO && loud {
            self.hold = self.hold_len;
        } else {
            self.hold = (self.hold - 1.0).max(0.0);
        }
        let ducked = live && ((mode == DUCK_MANUAL) || (mode == DUCK_AUTO && self.hold > 0.0));
        let target = if ducked { low } else { 1.0 };
        let k = if target < self.gain {
            self.attack
        } else {
            self.release
        };
        self.gain += (target - self.gain) * k;
        self.gain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(period: usize) -> (InputFeed, rtrb::Producer<f32>) {
        let (tx, rx) = rtrb::RingBuffer::new(16_384);
        let period = Arc::new(AtomicUsize::new(period));
        (InputFeed { rx, period }, tx)
    }

    fn push(tx: &mut rtrb::Producer<f32>, frames: usize, v: f32) {
        for _ in 0..frames * 2 {
            tx.push(v).unwrap();
        }
    }

    #[test]
    fn the_queue_stays_short_and_waits_out_gaps() {
        let (mut feed, mut tx) = feed(128);
        let mut filling = true;
        let mut out = [[0.0f32; 2]; 256];
        // Waits for a buffer's worth before playing.
        push(&mut tx, 128, 1.0);
        assert_eq!(pull(&mut feed, &mut filling, &mut out), 0);
        push(&mut tx, 128, 1.0);
        assert_eq!(pull(&mut feed, &mut filling, &mut out), 256);
        assert_eq!(out[255], [1.0, 1.0]);
        // A device that ran ahead is cut back to one buffer queued.
        push(&mut tx, 2048, 2.0);
        assert_eq!(pull(&mut feed, &mut filling, &mut out), 256);
        assert_eq!(feed.rx.slots() / 2, 0);
        // Running dry plays what is there and waits again.
        push(&mut tx, 100, 3.0);
        assert_eq!(pull(&mut feed, &mut filling, &mut out), 100);
        assert!(filling);
    }

    #[test]
    fn ducking_follows_the_voice() {
        let reg = ControlRegistry::new();
        let sr = 48_000.0;
        let mut d = Ducker::new(&reg, sr);
        let settings = d.settings();
        assert_eq!(settings.0, DUCK_AUTO);
        let run = |d: &mut Ducker, seconds: f32, live: bool, v: f32| {
            let mut g = 1.0;
            for _ in 0..(seconds * sr as f32) as usize {
                g = d.next(settings, live, [v, v]);
            }
            g
        };
        // Talking takes the music down 12 dB within a fifth of a second.
        assert!((run(&mut d, 0.2, true, 0.2) - 0.25).abs() < 0.01);
        // It stays down between words, then comes back.
        assert!(run(&mut d, 0.3, true, 0.0) < 0.26);
        assert!(run(&mut d, 3.0, true, 0.0) > 0.99);
        // A voice with the mic off doesn't duck.
        assert!(run(&mut d, 0.5, false, 0.2) > 0.99);
        // Manual ducks while the mic is on, voice or not.
        reg.get("[Master]", "talkover_ducking")
            .unwrap()
            .set(DUCK_MANUAL);
        let manual = d.settings();
        let mut g = 1.0;
        for _ in 0..9600 {
            g = d.next(manual, true, [0.0; 2]);
        }
        assert!((g - 0.25).abs() < 0.01);
    }
}
