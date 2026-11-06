//! The Groove Box's built-in kits and named grooves.
//!
//! Every sound is synthesised here when the engine starts, in the style of
//! the classic analogue drum machines: a kick is a sine wave swept down in
//! pitch with a click on top, a snare two tones and filtered noise, a clap
//! a few quick bursts of noise, the hats six square waves at clashing
//! pitches through a high-pass filter (the way the 808 makes its cymbals),
//! the cowbell two square waves through a band-pass. Nothing is sampled
//! from another machine, so the kits need no sample files and carry no
//! licence beyond openDJ's own. Kits are named by their character, never
//! after a manufacturer's model.

use std::f32::consts::TAU;
use std::sync::Arc;

use crate::sampler::Sample;

pub const SOUNDS_PER_KIT: usize = 8;

pub struct KitSound {
    pub name: &'static str,
    pub sample: Arc<Sample>,
}

pub struct Kit {
    pub name: &'static str,
    pub sounds: Vec<KitSound>,
}

/// A kit as the app lists it: its name, a line about it, and its sounds'
/// names in order. Row N starts on sound N, and every kit keeps the same
/// order so a groove sounds right on any of them: kick, the backbeat
/// (snare, clap or rim), closed hat, open hat, a second backbeat sound,
/// tom, rim (or shaker) and one more percussion sound.
pub struct KitInfo {
    pub name: &'static str,
    pub about: &'static str,
    pub sounds: [&'static str; SOUNDS_PER_KIT],
}

pub const KITS: [KitInfo; 4] = [
    KitInfo {
        name: "Boom",
        about: "Deep, long kick and soft analogue drums, 808-style",
        sounds: [
            "Kick",
            "Snare",
            "Closed hat",
            "Open hat",
            "Clap",
            "Tom",
            "Rim",
            "Cowbell",
        ],
    },
    KitInfo {
        name: "Punch",
        about: "Short, punchy kick and bright hats for house and techno, 909-style",
        sounds: [
            "Kick",
            "Clap",
            "Closed hat",
            "Open hat",
            "Snare",
            "Tom",
            "Rim",
            "Ride",
        ],
    },
    KitInfo {
        name: "Tin",
        about: "Small, ticking rhythm-box drums, 606-style",
        sounds: [
            "Kick",
            "Snare",
            "Closed hat",
            "Open hat",
            "Shaker",
            "Tom",
            "Rim",
            "Cowbell",
        ],
    },
    KitInfo {
        name: "Dub",
        about: "Soft kick, rimshot and a chord stab for dub techno",
        sounds: [
            "Kick",
            "Rim",
            "Closed hat",
            "Open hat",
            "Clap",
            "Tom",
            "Shaker",
            "Chord",
        ],
    },
];

/// A named groove: one row of 16 steps per track, `x` a hit and `.` a
/// rest, in the kits' sound order (kick, backbeat, hat, open hat, second
/// backbeat, tom, rim, percussion).
pub struct NamedGroove {
    pub name: &'static str,
    pub rows: [&'static str; 8],
}

pub const GROOVES: [NamedGroove; 6] = [
    NamedGroove {
        name: "Four to the floor",
        rows: [
            "x...x...x...x...",
            "....x.......x...",
            "x.x.x.x.x.x.x.x.",
            "................",
            "................",
            "................",
            "......x.......x.",
            "................",
        ],
    },
    NamedGroove {
        name: "Off-beat hats",
        rows: [
            "x...x...x...x...",
            "....x.......x...",
            "xx.xxx.xxx.xxx.x",
            "..x...x...x...x.",
            "............x...",
            "................",
            "...x......x.....",
            "..x...x...x...x.",
        ],
    },
    NamedGroove {
        name: "Breakbeat",
        rows: [
            "x.x.......x.....",
            "....x..x.x..x...",
            "x.x.x.x.x.x.x.x.",
            "..............x.",
            "................",
            ".............x.x",
            "...x............",
            "................",
        ],
    },
    NamedGroove {
        name: "Half time",
        rows: [
            "x.........x.....",
            "........x.......",
            "x.x.x.x.x.x.x.x.",
            "...x.......x....",
            "........x.......",
            "..............x.",
            "................",
            "x.......x.......",
        ],
    },
    NamedGroove {
        name: "Electro",
        rows: [
            "x.....x...x.....",
            "....x.......x...",
            "x.x.x.x.x.x.x.xx",
            ".......x......x.",
            "....x.......x...",
            "................",
            "..x.....x.....x.",
            "...x....x.......",
        ],
    },
    NamedGroove {
        name: "Minimal",
        rows: [
            "x...x...x...x...",
            "................",
            "..x...x...x...x.",
            "..............x.",
            "................",
            "................",
            "...x...x...x..x.",
            ".x.....x.x......",
        ],
    },
];

/// A row's steps as bits: bit s set when step s+1 is `x` (or `X`, `o`).
pub fn parse_steps(steps: &str) -> u32 {
    steps
        .chars()
        .take(16)
        .enumerate()
        .filter(|(_, c)| matches!(c, 'x' | 'X' | 'o'))
        .fold(0, |bits, (i, _)| bits | 1 << i)
}

/// Synthesises every kit for an engine running at `rate`.
pub fn build(rate: f64) -> Vec<Kit> {
    let sr = rate as f32;
    KITS.iter()
        .enumerate()
        .map(|(k, info)| Kit {
            name: info.name,
            sounds: info
                .sounds
                .iter()
                .enumerate()
                .map(|(n, &name)| KitSound {
                    name,
                    sample: Arc::new(Sample::from_mono(&sound(k, n, sr), rate)),
                })
                .collect(),
        })
        .collect()
}

/// Kit `k`'s sound `n`.
fn sound(k: usize, n: usize, sr: f32) -> Vec<f32> {
    let name = KITS[k].sounds[n];
    let seed = (k * 16 + n) as u32 + 1;
    match (k, name) {
        (0, "Kick") => kick(sr, 150.0, 48.0, 0.035, 0.9, 0.25, 1.0),
        (1, "Kick") => kick(sr, 260.0, 52.0, 0.022, 0.34, 0.6, 2.2),
        (2, "Kick") => kick(sr, 200.0, 70.0, 0.015, 0.2, 0.4, 1.4),
        (_, "Kick") => kick(sr, 110.0, 44.0, 0.04, 0.55, 0.1, 1.0),
        (0, "Snare") => snare(sr, [180.0, 330.0], 0.12, 0.16, 0.5, seed),
        (1, "Snare") => snare(sr, [200.0, 360.0], 0.08, 0.2, 0.75, seed),
        (_, "Snare") => snare(sr, [240.0, 420.0], 0.05, 0.1, 0.65, seed),
        (_, "Clap") => clap(sr, if k == 1 { 0.22 } else { 0.16 }, seed),
        (1, "Closed hat") => hat(sr, 0.05, 8000.0, seed),
        (2, "Closed hat") => hat(sr, 0.03, 9000.0, seed),
        (3, "Closed hat") => hat(sr, 0.035, 7000.0, seed),
        (_, "Closed hat") => hat(sr, 0.045, 7500.0, seed),
        (1, "Open hat") => hat(sr, 0.35, 7500.0, seed),
        (_, "Open hat") => hat(sr, 0.28, 7000.0, seed),
        (_, "Ride") => ride(sr, seed),
        (2, "Cowbell") => cowbell(sr, [800.0, 1180.0], 0.12),
        (_, "Cowbell") => cowbell(sr, [540.0, 800.0], 0.2),
        (3, "Rim") => rim(sr, 0.05, seed),
        (_, "Rim") => rim(sr, 0.03, seed),
        (0, "Tom") => tom(sr, 110.0, 0.45),
        (_, "Tom") => tom(sr, 140.0, 0.3),
        (_, "Shaker") => shaker(sr, seed),
        (_, "Chord") => chord(sr),
        _ => Vec::new(),
    }
}

/// A small, repeatable noise source.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

/// A two-pole filter (the RBJ cookbook's).
struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    x: [f32; 2],
    y: [f32; 2],
}

enum Shape {
    Low,
    High,
    Band,
}

impl Biquad {
    fn new(shape: Shape, freq: f32, q: f32, sr: f32) -> Self {
        let w = TAU * (freq / sr).min(0.49);
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2.0 * q);
        let a0 = 1.0 + alpha;
        let b = match shape {
            Shape::Low => [(1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0],
            Shape::High => [(1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0],
            Shape::Band => [alpha, 0.0, -alpha],
        };
        Self {
            b: b.map(|v| v / a0),
            a: [-2.0 * cos / a0, (1.0 - alpha) / a0],
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0]
            - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

fn frames(sr: f32, seconds: f32) -> usize {
    (sr * seconds) as usize
}

/// Scales to a peak of `peak`, and fades the last 2 ms so nothing clicks.
fn finish(mut v: Vec<f32>, peak: f32, sr: f32) -> Vec<f32> {
    let max = v.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if max > 0.0 {
        v.iter_mut().for_each(|s| *s *= peak / max);
    }
    let fade = frames(sr, 0.002).min(v.len());
    let len = v.len();
    for (i, s) in v[len - fade..].iter_mut().enumerate() {
        *s *= 1.0 - i as f32 / fade as f32;
    }
    v
}

/// A sine swept from `from` to `to` Hz (time constant `sweep` s), dying away
/// over `decay` s, with a click of `click` and some `drive`.
fn kick(sr: f32, from: f32, to: f32, sweep: f32, decay: f32, click: f32, drive: f32) -> Vec<f32> {
    let n = frames(sr, decay * 4.0);
    let mut phase = 0.0f32;
    let mut noise = Noise(7);
    let mut hp = Biquad::new(Shape::High, 2000.0, 0.7, sr);
    let out = (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            let f = to + (from - to) * (-t / sweep).exp();
            phase += TAU * f / sr;
            let body = phase.sin() * (-t / decay).exp();
            let tick = hp.process(noise.next()) * (-t / 0.003).exp() * click;
            ((body + tick) * drive).tanh()
        })
        .collect();
    finish(out, 0.95, sr)
}

/// Two tones and band-passed noise.
fn snare(
    sr: f32,
    tones: [f32; 2],
    tone_decay: f32,
    noise_decay: f32,
    snap: f32,
    seed: u32,
) -> Vec<f32> {
    let n = frames(sr, noise_decay * 4.0);
    let mut noise = Noise(seed);
    let mut bp = Biquad::new(Shape::High, 1800.0, 0.6, sr);
    let out = (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            let tone = ((TAU * tones[0] * t).sin() + 0.6 * (TAU * tones[1] * t).sin())
                * (-t / tone_decay).exp();
            let rattle = bp.process(noise.next()) * (-t / noise_decay).exp();
            tone * (1.0 - snap) + rattle * snap * 1.6
        })
        .collect();
    finish(out, 0.8, sr)
}

/// Three quick bursts of band-passed noise, then a tail.
fn clap(sr: f32, tail: f32, seed: u32) -> Vec<f32> {
    let n = frames(sr, tail * 4.0);
    let mut noise = Noise(seed);
    let mut bp = Biquad::new(Shape::Band, 1100.0, 1.2, sr);
    let out = (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            let burst = [0.0, 0.011, 0.023]
                .iter()
                .map(|&at| {
                    let d = t - at;
                    if (0.0..0.03).contains(&d) {
                        (-d / 0.004).exp()
                    } else {
                        0.0
                    }
                })
                .fold(0.0f32, f32::max);
            let env = burst.max(if t > 0.023 {
                0.6 * (-(t - 0.023) / tail).exp()
            } else {
                0.0
            });
            bp.process(noise.next()) * env
        })
        .collect();
    finish(out, 0.8, sr)
}

/// Six square waves at clashing pitches, high-passed: metallic cymbals.
fn metal(sr: f32, n: usize, tone: f32, seed: u32) -> Vec<f32> {
    const PITCHES: [f32; 6] = [205.3, 304.4, 369.6, 522.7, 540.0, 800.0];
    let mut noise = Noise(seed);
    let mut hp1 = Biquad::new(Shape::High, tone, 0.7, sr);
    let mut hp2 = Biquad::new(Shape::High, tone, 0.7, sr);
    (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            let squares: f32 = PITCHES
                .iter()
                .map(|f| {
                    if (f * 1.7 * t).fract() < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                })
                .sum::<f32>()
                / 6.0;
            hp2.process(hp1.process(squares * 0.8 + noise.next() * 0.25))
        })
        .collect()
}

fn hat(sr: f32, decay: f32, tone: f32, seed: u32) -> Vec<f32> {
    let n = frames(sr, decay * 5.0);
    let mut v = metal(sr, n, tone, seed);
    for (i, s) in v.iter_mut().enumerate() {
        *s *= (-(i as f32 / sr) / decay).exp();
    }
    finish(v, 0.6, sr)
}

fn ride(sr: f32, seed: u32) -> Vec<f32> {
    let n = frames(sr, 1.6);
    let mut v = metal(sr, n, 5000.0, seed);
    for (i, s) in v.iter_mut().enumerate() {
        let t = i as f32 / sr;
        *s *= 0.4 * (-t / 0.02).exp() + 0.6 * (-t / 0.5).exp();
    }
    finish(v, 0.5, sr)
}

/// Two square waves through a band-pass.
fn cowbell(sr: f32, pitches: [f32; 2], decay: f32) -> Vec<f32> {
    let n = frames(sr, decay * 4.0);
    let mut bp = Biquad::new(Shape::Band, pitches[1] * 1.2, 2.0, sr);
    let out = (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            let sq = |f: f32| if (f * t).fract() < 0.5 { 1.0 } else { -1.0 };
            let env = 0.6 * (-t / 0.01).exp() + 0.4 * (-t / decay).exp();
            bp.process(sq(pitches[0]) + sq(pitches[1])) * env
        })
        .collect();
    finish(out, 0.6, sr)
}

/// A short knock: a high tone and a click.
fn rim(sr: f32, decay: f32, seed: u32) -> Vec<f32> {
    let n = frames(sr, decay * 5.0);
    let mut noise = Noise(seed);
    let mut bp = Biquad::new(Shape::Band, 1700.0, 3.0, sr);
    let out = (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            let tone = (TAU * 1700.0 * t).sin() * 0.5 + (TAU * 500.0 * t).sin() * 0.3;
            (tone + bp.process(noise.next())) * (-t / decay).exp()
        })
        .collect();
    finish(out, 0.7, sr)
}

fn tom(sr: f32, pitch: f32, decay: f32) -> Vec<f32> {
    kick(sr, pitch * 1.6, pitch, 0.05, decay, 0.15, 1.0)
        .into_iter()
        .map(|s| s * 0.85)
        .collect()
}

fn shaker(sr: f32, seed: u32) -> Vec<f32> {
    let n = frames(sr, 0.18);
    let mut noise = Noise(seed);
    let mut hp = Biquad::new(Shape::High, 6000.0, 0.7, sr);
    let out = (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            let env = (t / 0.015).min(1.0) * (-t / 0.05).exp();
            hp.process(noise.next()) * env
        })
        .collect();
    finish(out, 0.45, sr)
}

/// A minor chord of saw waves through a closing low-pass: dub techno's stab.
fn chord(sr: f32) -> Vec<f32> {
    // C minor 7 around middle C, with the root an octave down.
    const NOTES: [f32; 5] = [130.81, 261.63, 311.13, 392.0, 466.16];
    let n = frames(sr, 1.2);
    let mut lp = Biquad::new(Shape::Low, 1200.0, 0.9, sr);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        if i % 64 == 0 {
            // The filter closes as the stab dies away.
            let cutoff = 300.0 + 2200.0 * (-t / 0.12).exp();
            let fresh = Biquad::new(Shape::Low, cutoff, 0.9, sr);
            lp.b = fresh.b;
            lp.a = fresh.a;
        }
        let saw: f32 = NOTES
            .iter()
            .enumerate()
            .map(|(k, f)| {
                // A little detune per note, as a real synth drifts.
                let f = f * (1.0 + 0.002 * k as f32);
                (f * t).fract() * 2.0 - 1.0
            })
            .sum::<f32>()
            / NOTES.len() as f32;
        out.push(lp.process(saw) * (-t / 0.35).exp());
    }
    finish(out, 0.55, sr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kit_has_its_sounds_short_and_in_range() {
        let kits = build(48_000.0);
        assert_eq!(kits.len(), KITS.len());
        for kit in &kits {
            assert_eq!(kit.sounds.len(), SOUNDS_PER_KIT);
            for s in &kit.sounds {
                assert!(!s.sample.is_empty(), "{} {}", kit.name, s.name);
                assert!(s.sample.seconds() < 5.0, "{} {}", kit.name, s.name);
                let peak = (0..s.sample.len())
                    .map(|i| s.sample.frame(i)[0].abs())
                    .fold(0.0, f32::max);
                assert!(
                    peak > 0.2 && peak <= 1.0,
                    "{} {} peak {peak}",
                    kit.name,
                    s.name
                );
            }
        }
    }

    #[test]
    fn grooves_parse_to_sixteen_steps() {
        assert_eq!(parse_steps("x...x...x...x..."), 0b0001_0001_0001_0001);
        for g in &GROOVES {
            for r in g.rows {
                assert_eq!(r.len(), 16, "{}", g.name);
            }
            assert!(parse_steps(g.rows[0]) != 0, "{} has a kick", g.name);
        }
    }
}
