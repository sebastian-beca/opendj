//! The sampler: eight one-shot players, `[Sampler1]` to `[Sampler8]`, for
//! the pads' Sampler page. Each holds a short built-in sound, made here when
//! the engine starts (kick, snare, clap, hat, air horn, siren, laser, crash),
//! so the pads work with no sample files. Pressing a pad
//! (`cue_gotoandplay`) plays its sound from the start; it plays out once and
//! stops. Samplers go to the main mix after the crossfader, like Mixxx's.
//!
//! Each player's audio is a fixed buffer made up front, so nothing allocates
//! on the audio thread.

use std::f32::consts::TAU;

use mixxx_control::{Behavior, ControlHandle, ControlRegistry};

use crate::controls::Edge;

pub const NUM_SAMPLERS: usize = 8;

/// The built-in sounds, in pad order.
pub const SAMPLE_NAMES: [&str; NUM_SAMPLERS] = [
    "Kick", "Snare", "Clap", "Hat", "Horn", "Siren", "Laser", "Crash",
];

pub fn sampler_group(n: usize) -> String {
    format!("[Sampler{}]", n + 1)
}

struct Player {
    audio: Vec<f32>,
    pos: usize,
    playing: bool,
    go: ControlHandle,
    go_edge: Edge,
    play: ControlHandle,
    play_indicator: ControlHandle,
    volume: ControlHandle,
    pregain: ControlHandle,
}

pub(crate) struct Samplers {
    players: Vec<Player>,
}

impl Samplers {
    pub(crate) fn new(reg: &ControlRegistry, sample_rate: f64) -> Self {
        let players = (0..NUM_SAMPLERS)
            .map(|n| {
                let g = sampler_group(n);
                let g = g.as_str();
                reg.indicator(g, "track_loaded", 1.0).set_bool(true);
                reg.indicator(g, "duration", 60.0);
                let audio = sound(n, sample_rate as f32);
                reg.get(g, "duration")
                    .unwrap()
                    .set(audio.len() as f64 / sample_rate);
                Player {
                    audio,
                    pos: 0,
                    playing: false,
                    go: reg.push(g, "cue_gotoandplay"),
                    go_edge: Edge::default(),
                    play: reg.toggle(g, "play"),
                    play_indicator: reg.indicator(g, "play_indicator", 1.0),
                    volume: reg.create(g, "volume", 0.8, 0.0, 1.0, Behavior::Linear),
                    pregain: reg.create(g, "pregain", 1.0, 0.0, 4.0, Behavior::CenteredOnDefault),
                }
            })
            .collect();
        Self { players }
    }

    /// Reads the controls before a buffer.
    pub(crate) fn update(&mut self) {
        for p in &mut self.players {
            if p.go_edge.pressed(&p.go) {
                p.pos = 0;
                p.playing = true;
                p.play.set_bool(true);
            } else if p.play.get_bool() != p.playing {
                // Play starts it from wherever it stopped (the start once played out).
                p.playing = p.play.get_bool();
                if p.playing && p.pos >= p.audio.len() {
                    p.pos = 0;
                }
            }
        }
    }

    /// The samplers' next frame, mixed.
    pub(crate) fn next(&mut self) -> [f32; 2] {
        let mut sum = 0.0;
        for p in &mut self.players {
            if !p.playing {
                continue;
            }
            match p.audio.get(p.pos) {
                Some(&v) => {
                    sum += v * (p.volume.get() * p.pregain.get()) as f32;
                    p.pos += 1;
                }
                None => p.playing = false,
            }
        }
        [sum, sum]
    }

    /// Writes the controls after a buffer.
    pub(crate) fn publish(&mut self) {
        for p in &mut self.players {
            if !p.playing && p.play.get_bool() {
                p.play.set_bool(false);
            }
            p.play_indicator.set_bool(p.playing);
        }
    }
}

/// A small deterministic noise source, so the sounds are the same every run.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

/// Built-in sound `n` at `sr`, mono, peaking around 0.8.
fn sound(n: usize, sr: f32) -> Vec<f32> {
    let len = |seconds: f32| (seconds * sr) as usize;
    let t = |i: usize| i as f32 / sr;
    let mut noise = Noise(0x9e37_79b9 ^ n as u32);
    let mut phase = 0.0f32;
    let mut lp = 0.0f32;
    let mut hp_prev = 0.0f32;
    let mut hp = 0.0f32;
    match n {
        // Kick: a sine sweeping from 150 Hz down to 45 Hz.
        0 => (0..len(0.45))
            .map(|i| {
                let t = t(i);
                phase += TAU * (45.0 + 105.0 * (-t * 28.0).exp()) / sr;
                phase.sin() * (-t * 7.0).exp() * 0.9
            })
            .collect(),
        // Snare: a 190 Hz body and a burst of noise.
        1 => (0..len(0.3))
            .map(|i| {
                let t = t(i);
                phase += TAU * 190.0 / sr;
                (phase.sin() * (-t * 25.0).exp() * 0.5 + noise.next() * (-t * 14.0).exp() * 0.45)
                    * 0.9
            })
            .collect(),
        // Clap: three quick noise bursts and a tail, band-limited.
        2 => (0..len(0.35))
            .map(|i| {
                let t = t(i);
                let burst = [0.0, 0.011, 0.023]
                    .iter()
                    .map(|&s| {
                        if t >= s {
                            (-(t - s) * 180.0).exp()
                        } else {
                            0.0
                        }
                    })
                    .sum::<f32>()
                    + 0.6 * (-t * 12.0).exp();
                let x = noise.next();
                lp += 0.35 * (x - lp);
                (x - lp) * burst * 0.7
            })
            .collect(),
        // Closed hi-hat: high-passed noise, short.
        3 => (0..len(0.12))
            .map(|i| {
                let x = noise.next();
                hp = 0.6 * (hp + x - hp_prev);
                hp_prev = x;
                hp * (-t(i) * 45.0).exp() * 0.7
            })
            .collect(),
        // Air horn: three detuned saws in a minor third, with a little decay.
        4 => (0..len(1.2))
            .map(|i| {
                let t = t(i);
                let saw = |hz: f32| 2.0 * (t * hz).fract() - 1.0;
                let env = (t * 40.0).min(1.0) * if t > 1.0 { (1.2 - t) * 5.0 } else { 1.0 };
                let x = (saw(466.0) + saw(469.0) + saw(554.0)) / 3.0;
                lp += 0.25 * (x - lp);
                lp * env * 0.75
            })
            .collect(),
        // Siren: a square wave sweeping up and down.
        5 => (0..len(1.6))
            .map(|i| {
                let t = t(i);
                let hz = 700.0 + 350.0 * (TAU * 1.25 * t).sin();
                phase += TAU * hz / sr;
                let x = phase.sin().signum();
                lp += 0.2 * (x - lp);
                lp * (t * 30.0).min(1.0) * ((1.6 - t) * 6.0).min(1.0) * 0.45
            })
            .collect(),
        // Laser: a fast downward sweep.
        6 => (0..len(0.4))
            .map(|i| {
                let t = t(i);
                phase += TAU * (180.0 + 2800.0 * (-t * 14.0).exp()) / sr;
                phase.sin() * (-t * 6.0).exp() * 0.6
            })
            .collect(),
        // Crash: long, bright noise.
        _ => (0..len(1.8))
            .map(|i| {
                let x = noise.next();
                hp = 0.75 * (hp + x - hp_prev);
                hp_prev = x;
                hp * (-t(i) * 2.6).exp() * 0.5
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sounds_are_short_and_in_range() {
        for (n, &name) in SAMPLE_NAMES.iter().enumerate() {
            let s = sound(n, 48_000.0);
            let peak = s.iter().fold(0.0f32, |p, v| p.max(v.abs()));
            assert!(s.len() < 48_000 * 2, "{name}");
            assert!(peak > 0.1 && peak <= 1.0, "{name} peaks at {peak}");
        }
    }

    #[test]
    fn a_pad_plays_its_sound_once() {
        let reg = ControlRegistry::new();
        let mut s = Samplers::new(&reg, 48_000.0);
        let go = reg.get("[Sampler1]", "cue_gotoandplay").unwrap();
        let playing = reg.get("[Sampler1]", "play_indicator").unwrap();
        go.set(1.0);
        s.update();
        let loud = (0..4_800).map(|_| s.next()[0].abs()).fold(0.0, f32::max);
        assert!(loud > 0.3);
        s.publish();
        assert!(playing.get_bool());
        for _ in 0..48_000 {
            s.next();
        }
        s.publish();
        assert!(!playing.get_bool());
    }
}
