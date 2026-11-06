//! The Groove Box: a deck that plays a drum kit in a 16-step pattern, in
//! time with the mix, instead of a track.
//!
//! `[GrooveN],enabled` turns deck N into a Groove Box. Its sound goes into
//! the deck's channel where a track's would, so EQ, filter, fader,
//! crossfader, effects and headphone cue work as on any deck, and
//! `[ChannelN],play` starts and stops it.
//!
//! **Clock.** The groove keeps its own beat count and follows, in order:
//! the leading deck while it plays, Ableton Link's session while Link is
//! on, or its own tempo (`[GrooveN],bpm`). It never copies the leader's
//! position; it bends its speed toward the leader's beat a little each
//! buffer, as sync does, and when it is more than a quarter of a beat off
//! (a hot cue or beat jump on the leader) it jumps to the leader's beat at
//! its next bar line, never mid-bar. Started while a leader plays, it waits
//! for the leader's next downbeat.
//!
//! **Steps.** Step k (of 16 to a bar) falls on beat k/4, every second one
//! late by `swing`. Each buffer covers beats `[b0, b0 + frames × step)`;
//! every step inside it starts its sounds on the frame it falls on, so the
//! timing is exact to the sample whatever the buffer size.
//!
//! **Patterns.** Four grooves (A to D), each nine rows of 16 steps, kept in
//! `[GrooveN],patternG_rowR` as 16-bit numbers (bit s on = step s+1 plays)
//! with `patternG_rowR_soft` and `patternG_rowR_accent` marking the steps
//! that play softly (half as loud) or accented (a quarter louder).
//! `[GrooveN],groove` picks the one playing (0-3); a change while playing
//! waits for the next bar. Row N plays the kits' Nth sound
//! ([`kits::ROLES`]: kick, snare, hat, open hat, clap, rim, cowbell, crash,
//! ride) unless `rowR_sound` picks another.
//!
//! **Library.** `genre` and `pattern` (1-based, 0 none) name a groove of
//! the library ([`crate::patterns`]); `load` puts it into the groove picked.
//! `chain` (1-based, 0 off) names a second groove of the same genre;
//! `load_chain` puts it into the groove after the one picked (A's partner
//! is B, C's is D), and while `chain` is on the two take turns, a bar
//! each. `groove_playing` is the one sounding now. Each row has `rowR_mute`,
//! `rowR_solo`, `rowR_level` (0-1.5), `rowR_pitch` (−12 to +12 semitones)
//! and `rowR_decay` (0-1, 1 = the sound's own length), and `rowR_sound`
//! picks which of the kit's sounds the row plays; `rowR_preview` plays it
//! once, now. `[GrooveN],kit` picks the
//! kit. `[GrooveN],position` is the step playing (0-15), −1 when stopped.
//!
//! **Duck.** With `[GrooveN],duck_deck` set to another deck (1-4), that
//! deck dips by `duck_amount` (dB) on each step of row 1 and recovers over
//! about a step.
//!
//! Kits are synthesised when the engine starts (see [`crate::kits`]); every
//! buffer only reads them, and nothing allocates or frees here.

use std::sync::Arc;

use opendj_control::{Behavior, ControlHandle, ControlRegistry};

use crate::kits::{self, Kit};
use crate::patterns::{self, Level};
use crate::sampler::Sample;
use crate::NUM_DECKS;

pub const ROWS: usize = kits::ROLES.len();
pub const STEPS: usize = 16;
pub const GROOVES: usize = 4;

/// The tempo a Groove Box plays at before anything else sets one.
pub const DEFAULT_BPM: f64 = 120.0;
/// Further off the leader's beat than this, the groove jumps at its next
/// bar instead of bending its speed.
const SNAP_BEATS: f64 = 0.25;
/// The most the groove bends its speed to catch up with the leader.
const MAX_BEND: f64 = 0.05;
/// Frames over which a sound that is cut off fades out.
const CHOKE_FRAMES: usize = 64;

/// The controls of one Groove Box.
pub struct GrooveControls {
    pub enabled: ControlHandle,
    pub kit: ControlHandle,
    pub groove: ControlHandle,
    pub groove_queued: ControlHandle,
    pub groove_playing: ControlHandle,
    pub genre: ControlHandle,
    pub pattern: ControlHandle,
    pub load: ControlHandle,
    pub chain: ControlHandle,
    pub load_chain: ControlHandle,
    pub bpm: ControlHandle,
    pub swing: ControlHandle,
    pub position: ControlHandle,
    /// On while the groove follows a playing deck or Link.
    pub following: ControlHandle,
    pub duck_deck: ControlHandle,
    pub duck_amount: ControlHandle,
    pub patterns: [[ControlHandle; ROWS]; GROOVES],
    pub soft: [[ControlHandle; ROWS]; GROOVES],
    pub accent: [[ControlHandle; ROWS]; GROOVES],
    pub mute: [ControlHandle; ROWS],
    pub solo: [ControlHandle; ROWS],
    pub level: [ControlHandle; ROWS],
    pub pitch: [ControlHandle; ROWS],
    pub decay: [ControlHandle; ROWS],
    pub sound: [ControlHandle; ROWS],
    /// Plays the row's sound once, now (a step clicked while stopped).
    pub preview: [ControlHandle; ROWS],
}

pub fn groove_group(deck: usize) -> String {
    format!("[Groove{}]", deck + 1)
}

impl GrooveControls {
    pub fn new(reg: &ControlRegistry, deck: usize) -> Self {
        let g = groove_group(deck);
        let g = g.as_str();
        let rows = |f: &dyn Fn(usize) -> ControlHandle| std::array::from_fn(f);
        let kit_count = kits::KITS.len() as f64;
        let lib = patterns::library();
        let genres = lib.len().max(1) as f64;
        let most = lib.iter().map(|g| g.grooves.len()).max().unwrap_or(0) as f64;
        let c = Self {
            enabled: reg.toggle(g, "enabled"),
            kit: reg.create(g, "kit", 0.0, 0.0, kit_count - 1.0, Behavior::Free),
            groove: reg.create(g, "groove", 0.0, 0.0, (GROOVES - 1) as f64, Behavior::Free),
            groove_queued: reg.indicator(g, "groove_queued", (GROOVES - 1) as f64),
            groove_playing: reg.indicator(g, "groove_playing", (GROOVES - 1) as f64),
            genre: reg.create(g, "genre", 0.0, 0.0, genres - 1.0, Behavior::Free),
            pattern: reg.create(g, "pattern", 1.0, 0.0, most, Behavior::Free),
            load: reg.push(g, "load"),
            chain: reg.create(g, "chain", 0.0, 0.0, most, Behavior::Free),
            load_chain: reg.push(g, "load_chain"),
            bpm: reg.create(g, "bpm", DEFAULT_BPM, 40.0, 220.0, Behavior::Linear),
            swing: reg.create(g, "swing", 0.0, 0.0, 0.75, Behavior::Linear),
            position: reg.create(
                g,
                "position",
                -1.0,
                -1.0,
                (STEPS - 1) as f64,
                Behavior::Indicator,
            ),
            following: reg.indicator(g, "following", 1.0),
            duck_deck: reg.create(g, "duck_deck", 0.0, 0.0, NUM_DECKS as f64, Behavior::Free),
            duck_amount: reg.create(g, "duck_amount", 6.0, 0.0, 24.0, Behavior::Linear),
            patterns: std::array::from_fn(|p| {
                rows(&|r| {
                    reg.create(
                        g,
                        &format!("pattern{}_row{}", p + 1, r + 1),
                        0.0,
                        0.0,
                        65535.0,
                        Behavior::Free,
                    )
                })
            }),
            soft: std::array::from_fn(|p| {
                rows(&|r| {
                    reg.create(
                        g,
                        &format!("pattern{}_row{}_soft", p + 1, r + 1),
                        0.0,
                        0.0,
                        65535.0,
                        Behavior::Free,
                    )
                })
            }),
            accent: std::array::from_fn(|p| {
                rows(&|r| {
                    reg.create(
                        g,
                        &format!("pattern{}_row{}_accent", p + 1, r + 1),
                        0.0,
                        0.0,
                        65535.0,
                        Behavior::Free,
                    )
                })
            }),
            mute: rows(&|r| reg.toggle(g, &format!("row{}_mute", r + 1))),
            solo: rows(&|r| reg.toggle(g, &format!("row{}_solo", r + 1))),
            level: rows(&|r| {
                reg.create(
                    g,
                    &format!("row{}_level", r + 1),
                    1.0,
                    0.0,
                    1.5,
                    Behavior::Linear,
                )
            }),
            pitch: rows(&|r| {
                reg.create(
                    g,
                    &format!("row{}_pitch", r + 1),
                    0.0,
                    -12.0,
                    12.0,
                    Behavior::Linear,
                )
            }),
            decay: rows(&|r| {
                reg.create(
                    g,
                    &format!("row{}_decay", r + 1),
                    1.0,
                    0.0,
                    1.0,
                    Behavior::Linear,
                )
            }),
            sound: rows(&|r| {
                reg.create(
                    g,
                    &format!("row{}_sound", r + 1),
                    r as f64,
                    0.0,
                    (kits::MAX_SOUNDS - 1) as f64,
                    Behavior::Free,
                )
            }),
            preview: rows(&|r| reg.push(g, &format!("row{}_preview", r + 1))),
        };
        // A new Groove Box opens with the library's first groove.
        c.load_groove(0, 0, 1);
        c
    }

    /// Puts groove `index` (1-based) of genre `genre` into groove `slot`.
    pub fn load_groove(&self, slot: usize, genre: usize, index: usize) {
        let Some(p) = index
            .checked_sub(1)
            .and_then(|i| patterns::groove(genre, i))
        else {
            return;
        };
        let slot = slot.min(GROOVES - 1);
        for (row, steps) in p.rows.iter().enumerate() {
            self.patterns[slot][row].set(steps.hits as f64);
            self.soft[slot][row].set(steps.soft as f64);
            self.accent[slot][row].set(steps.accent as f64);
        }
    }

    /// The groove the library's `load` and `chain` go into: the one picked,
    /// and its partner (A and B, C and D).
    fn slots(&self) -> (usize, usize) {
        let chosen = (self.groove.get().round() as usize).min(GROOVES - 1);
        (chosen, chosen ^ 1)
    }
}

/// Where the groove's beat comes from this buffer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Source {
    /// The tempo heard, in beats per minute.
    pub bpm: f64,
    /// The leader's beat at the buffer's first frame (beat 0 a downbeat);
    /// None when the groove keeps its own time.
    pub beat: Option<f64>,
}

/// The groove's sense of time, separate from the sound so it can be tested
/// on its own.
#[derive(Clone, Debug, Default)]
pub struct Clock {
    /// The beat at the next buffer's first frame.
    pub beat: f64,
    pub running: bool,
    /// No step before this beat plays (waiting for the leader's downbeat).
    start_at: f64,
    /// A jump onto the leader's beat, taken at the next bar line.
    pending: Option<f64>,
}

impl Clock {
    /// Starts the groove: on the leader's next downbeat, or at once.
    pub fn start(&mut self, source: Source) {
        self.running = true;
        self.pending = None;
        match source.beat {
            Some(b) => {
                self.beat = b;
                // A hair of tolerance so a start right on the bar plays it.
                self.start_at = ((b - 1e-6) / 4.0).ceil() * 4.0;
            }
            None => {
                self.beat = 0.0;
                self.start_at = 0.0;
            }
        }
    }

    pub fn stop(&mut self) {
        self.running = false;
        self.pending = None;
    }

    /// Advances over a buffer of `frames` at `rate`; returns the beat at its
    /// first frame and the beats per frame. A jump waiting for the bar line
    /// is taken here when the bar line falls in this buffer: the returned
    /// span then starts on the bar line (`b0` is moved so the bar's steps
    /// land on the same frames).
    pub fn advance(&mut self, frames: usize, rate: f64, source: Source) -> Span {
        let base = source.bpm.max(1.0) / 60.0 / rate;
        let mut per_frame = base;
        if let Some(target) = source.beat {
            // Off by how much, within a bar either way.
            let mut off = target - self.beat;
            off -= (off / 4.0).round() * 4.0;
            if off.abs() > SNAP_BEATS {
                self.pending = Some(off);
            } else {
                self.pending = None;
                // Catch up over about a beat.
                let span = base * frames as f64;
                per_frame = base * (1.0 + (off / span.max(1.0)).clamp(-MAX_BEND, MAX_BEND));
            }
        } else {
            self.pending = None;
        }
        let b0 = self.beat;
        let b1 = b0 + per_frame * frames as f64;
        let mut span = Span {
            beat: b0,
            per_frame,
            jump: None,
        };
        if let Some(off) = self.pending {
            let bar = (b0 / 4.0).floor() * 4.0 + 4.0;
            if bar < b1 {
                // At the bar line the count moves by `off`: from the first
                // frame on or after it, the frames play the leader's beats.
                let at = (((bar - b0) / per_frame).ceil() as usize).min(frames);
                span.jump = Some((at, off));
                self.pending = None;
                self.beat = b1 + off;
                self.start_at = self.start_at.min(bar + off);
                return span;
            }
        }
        self.beat = b1;
        span
    }

    /// Whether a step at `beat` may play.
    pub fn plays(&self, beat: f64) -> bool {
        self.running && beat >= self.start_at - 1e-9
    }
}

/// One buffer's stretch of beats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub beat: f64,
    pub per_frame: f64,
    /// From frame `at` on, the beat count is `off` further along.
    pub jump: Option<(usize, f64)>,
}

impl Span {
    /// The beat at frame `f` of the buffer.
    pub fn beat_at(&self, f: usize) -> f64 {
        let b = self.beat + self.per_frame * f as f64;
        match self.jump {
            Some((at, off)) if f >= at => b + off,
            _ => b,
        }
    }

    /// The steps that start in this buffer of `frames`: (frame, step) in
    /// order, with every second step `swing` (0-0.75) of a 16th late.
    pub fn steps(&self, frames: usize, swing: f64, mut each: impl FnMut(usize, usize, f64)) {
        let parts = match self.jump {
            Some((at, off)) => [(0, at, 0.0), (at, frames, off)],
            None => [(0, frames, 0.0), (frames, frames, 0.0)],
        };
        let late = swing.clamp(0.0, 0.75) * 0.25;
        for (from, to, off) in parts {
            if from >= to {
                continue;
            }
            let start = self.beat + off + self.per_frame * from as f64;
            let end = self.beat + off + self.per_frame * to as f64;
            // Steps are 16ths: step k at beat k/4 (plus swing when odd).
            let first = ((start - late) * 4.0).floor() as i64;
            let last = (end * 4.0).ceil() as i64;
            for k in first..=last {
                let t = k as f64 / 4.0 + if k.rem_euclid(2) == 1 { late } else { 0.0 };
                if t < start - 1e-9 || t >= end - 1e-9 {
                    continue;
                }
                let f = from + ((t - start) / self.per_frame).round() as usize;
                each(f.min(to - 1), k.rem_euclid(STEPS as i64) as usize, t);
            }
        }
    }
}

/// A sound playing on a row.
#[derive(Default)]
struct Voice {
    sample: Option<Arc<Sample>>,
    pos: f64,
    rate: f64,
    gain: f32,
    /// The decay envelope and how fast it falls each frame.
    env: f32,
    fall: f32,
    /// Frames of fade left when cut off (0 while it plays on).
    fade: usize,
}

impl Voice {
    fn next(&mut self) -> [f32; 2] {
        let Some(s) = &self.sample else {
            return [0.0; 2];
        };
        let i = self.pos as usize;
        if i + 1 >= s.len() || self.env < 1e-4 {
            self.sample = None;
            return [0.0; 2];
        }
        let t = (self.pos - i as f64) as f32;
        let (a, b) = (s.frame(i), s.frame(i + 1));
        let mut g = self.gain * self.env;
        if self.fade > 0 {
            g *= self.fade as f32 / CHOKE_FRAMES as f32;
            self.fade -= 1;
            if self.fade == 0 {
                self.sample = None;
            }
        }
        self.env *= self.fall;
        self.pos += self.rate;
        [
            (a[0] + (b[0] - a[0]) * t) * g,
            (a[1] + (b[1] - a[1]) * t) * g,
        ]
    }
}

/// A hit due in this buffer.
#[derive(Clone, Copy, Default)]
struct Hit {
    frame: usize,
    row: usize,
    gain: f32,
}

const MAX_HITS: usize = 64;

pub struct GrooveBox {
    pub c: GrooveControls,
    pub clock: Clock,
    kits: Arc<[Kit]>,
    /// Each row's sound now and the one fading out after a choke.
    voices: [[Voice; 2]; ROWS],
    /// The groove playing and the one waiting for the bar.
    playing: usize,
    queued: Option<usize>,
    was_running: bool,
    /// A bar has started since PLAY, so the chain may turn at the next.
    bar_played: bool,
    /// The last tempo heard, kept when the leader stops.
    pub last_bpm: f64,
    rate: f64,
    hits: [Hit; MAX_HITS],
}

impl GrooveBox {
    pub fn new(reg: &ControlRegistry, deck: usize, kits: Arc<[Kit]>, rate: f64) -> Self {
        let c = GrooveControls::new(reg, deck);
        Self {
            c,
            clock: Clock::default(),
            kits,
            voices: Default::default(),
            playing: 0,
            queued: None,
            was_running: false,
            bar_played: false,
            last_bpm: DEFAULT_BPM,
            rate,
            hits: [Hit::default(); MAX_HITS],
        }
    }

    pub fn enabled(&self) -> bool {
        self.c.enabled.get_bool()
    }

    /// Whether any sound is still ringing.
    pub fn sounding(&self) -> bool {
        self.voices.iter().flatten().any(|v| v.sample.is_some())
    }

    /// Renders one buffer into `out` (cleared first). `play` is the deck's
    /// play button and `source` where the beat comes from; `kick` hears
    /// the frame of every row 1 hit, for ducking.
    pub fn render(
        &mut self,
        play: bool,
        source: Source,
        out: &mut [[f32; 2]],
        mut kick: impl FnMut(usize),
    ) {
        out.iter_mut().for_each(|s| *s = [0.0; 2]);
        let frames = out.len();
        let genre = self.c.genre.get().round() as usize;
        let (chosen, partner) = self.c.slots();
        if self.c.load.get_bool() {
            self.c.load.set(0.0);
            self.c
                .load_groove(chosen, genre, self.c.pattern.get().round() as usize);
        }
        if self.c.load_chain.get_bool() {
            self.c.load_chain.set(0.0);
            self.c
                .load_groove(partner, genre, self.c.chain.get().round() as usize);
        }
        let chained = self.c.chain.get() >= 0.5;
        if play && !self.was_running {
            self.clock.start(source);
            self.playing = chosen;
            self.queued = None;
            self.bar_played = false;
        } else if !play && self.was_running {
            self.clock.stop();
        }
        self.was_running = play;
        if !play {
            self.playing = chosen;
        }
        let in_turn = self.playing == chosen || (chained && self.playing == partner);
        self.queued = (!in_turn).then_some(chosen);
        self.c
            .groove_queued
            .set(self.queued.unwrap_or(self.playing) as f64);
        self.last_bpm = source.bpm;
        let mut n = 0;
        if self.clock.running {
            let span = self.clock.advance(frames, self.rate, source);
            let swing = self.c.swing.get();
            let any_solo = self.c.solo.iter().any(|s| s.get_bool());
            let mut queued = self.queued;
            let mut playing = self.playing;
            let mut bar_played = self.bar_played;
            let mut last_step = None;
            let clock = &self.clock;
            let c = &self.c;
            let hits = &mut self.hits;
            span.steps(frames, swing, |frame, step, beat| {
                if step == 0 {
                    if let Some(q) = queued.take() {
                        playing = q;
                    } else if chained && bar_played {
                        // The chain's two grooves take turns, a bar each.
                        playing = if playing == chosen { partner } else { chosen };
                    }
                }
                if !clock.plays(beat) {
                    return;
                }
                bar_played = true;
                last_step = Some(step);
                for row in 0..ROWS {
                    let bits = c.patterns[playing][row].get() as u32;
                    if bits & (1 << step) == 0 {
                        continue;
                    }
                    let soft = c.soft[playing][row].get() as u32;
                    let accent = c.accent[playing][row].get() as u32;
                    let gain = Level::of(step, soft, accent).gain();
                    let audible = if any_solo {
                        c.solo[row].get_bool()
                    } else {
                        !c.mute[row].get_bool()
                    };
                    if audible && n < MAX_HITS {
                        hits[n] = Hit { frame, row, gain };
                        n += 1;
                    }
                }
            });
            self.playing = playing;
            self.queued = queued;
            self.bar_played = bar_played;
            // The step under the playhead, for the screen and LEDs.
            let now = span.beat_at(frames / 2);
            let step = ((now * 4.0).floor() as i64).rem_euclid(STEPS as i64);
            self.c.position.set(if self.clock.plays(now) {
                step as f64
            } else {
                last_step.map_or(-1.0, |s| s as f64)
            });
        } else {
            self.c.position.set(-1.0);
        }
        self.c.groove_playing.set(self.playing as f64);
        let kit = (self.c.kit.get().round() as usize).min(self.kits.len().saturating_sub(1));
        for row in 0..ROWS {
            if self.c.preview[row].get_bool() {
                self.c.preview[row].set(0.0);
                self.trigger(kit, row, 1.0);
            }
        }
        let mut next_hit = 0;
        for (f, frame) in out.iter_mut().enumerate() {
            while next_hit < n && self.hits[next_hit].frame == f {
                let Hit { row, gain, .. } = self.hits[next_hit];
                next_hit += 1;
                if row == 0 {
                    kick(f);
                }
                self.trigger(kit, row, gain);
            }
            if !self.sounding_fast() {
                continue;
            }
            for voices in &mut self.voices {
                for v in voices.iter_mut() {
                    if v.sample.is_some() {
                        let s = v.next();
                        frame[0] += s[0];
                        frame[1] += s[1];
                    }
                }
            }
        }
    }

    #[inline]
    fn sounding_fast(&self) -> bool {
        self.voices.iter().flatten().any(|v| v.sample.is_some())
    }

    /// Plays `row`'s sound from the start, fading out what it played.
    fn trigger(&mut self, kit: usize, row: usize, gain: f32) {
        let Some(kit) = self.kits.get(kit) else {
            return;
        };
        let sound = (self.c.sound[row].get().round() as usize).min(kit.sounds.len() - 1);
        let sample = kit.sounds[sound].sample.clone();
        let decay = self.c.decay[row].get().clamp(0.0, 1.0);
        // Full decay: the sound as it is; less: an envelope from 20 ms up.
        let fall = if decay >= 0.999 {
            1.0
        } else {
            let tau = 0.02 + decay * decay * 1.2;
            (-1.0 / (tau * self.rate)).exp() as f32
        };
        let [now, tail] = &mut self.voices[row];
        if now.sample.is_some() {
            std::mem::swap(now, tail);
            tail.fade = CHOKE_FRAMES;
        }
        *now = Voice {
            sample: Some(sample),
            pos: 0.0,
            rate: 2f64.powf(self.c.pitch[row].get() / 12.0),
            gain: self.c.level[row].get() as f32 * gain,
            env: 1.0,
            fall,
            fade: 0,
        };
    }
}

/// The groove's source: the leading deck's tempo and beat while it plays,
/// else Link's, else the groove's own tempo.
pub fn pick_source(leader: Option<(f64, f64)>, link: Option<(f64, f64)>, own_bpm: f64) -> Source {
    match leader.or(link) {
        Some((bpm, beat)) if bpm > 0.0 => Source {
            bpm,
            beat: Some(beat),
        },
        _ => Source {
            bpm: own_bpm,
            beat: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 48_000.0;

    fn own(bpm: f64) -> Source {
        Source { bpm, beat: None }
    }

    /// The frames (counted from the start) on which each step starts, over
    /// `total` frames rendered in buffers of `buffer`.
    fn step_frames(
        buffer: usize,
        total: usize,
        swing: f64,
        source: impl Fn(f64) -> Source,
    ) -> Vec<(usize, usize)> {
        let mut clock = Clock::default();
        clock.start(source(0.0));
        let mut out = Vec::new();
        let mut done = 0;
        while done < total {
            let n = buffer.min(total - done);
            let span = clock.advance(n, RATE, source(done as f64));
            span.steps(n, swing, |f, step, beat| {
                if clock.plays(beat) {
                    out.push((done + f, step));
                }
            });
            done += n;
        }
        out
    }

    #[test]
    fn steps_land_on_the_same_frames_whatever_the_buffer_size() {
        let a = step_frames(64, 96_000, 0.0, |_| own(120.0));
        let b = step_frames(333, 96_000, 0.0, |_| own(120.0));
        let c = step_frames(2048, 96_000, 0.0, |_| own(120.0));
        assert_eq!(a, b);
        assert_eq!(a, c);
        // 120 BPM: a 16th is 6000 frames at 48 kHz; two seconds hold 16.
        assert_eq!(a.len(), 16);
        for (i, &(f, step)) in a.iter().enumerate() {
            assert_eq!(step, i);
            assert!((f as i64 - i as i64 * 6000).abs() <= 1, "step {i} at {f}");
        }
    }

    #[test]
    fn swing_delays_every_second_step() {
        let s = step_frames(512, 24_000, 0.5, |_| own(120.0));
        // Half of a 16th's quarter: 0.5 × 0.25 beats late = 1/8 beat = 3000 frames.
        assert_eq!(s[0], (0, 0));
        assert!((s[1].0 as i64 - 9000).abs() <= 1, "{:?}", s);
        assert!((s[2].0 as i64 - 12000).abs() <= 1, "{:?}", s);
    }

    #[test]
    fn follows_the_leader_and_starts_on_its_next_downbeat() {
        // The leader plays at 124 BPM and is on beat 2.5 when PLAY is pressed.
        let bpm = 124.0;
        let leader = |frame: f64| Source {
            bpm,
            beat: Some(2.5 + frame * bpm / 60.0 / RATE),
        };
        let s = step_frames(256, 96_000, 0.0, leader);
        // The first step heard is step 0, on the leader's beat 4.
        assert_eq!(s[0].1, 0);
        let beat4 = (1.5 * 60.0 / bpm * RATE) as i64;
        assert!((s[0].0 as i64 - beat4).abs() <= 1, "{} vs {beat4}", s[0].0);
        // Every later step stays on the leader's 16ths.
        for &(f, step) in &s {
            let beat = 2.5 + f as f64 * bpm / 60.0 / RATE;
            let off = (beat * 4.0 - (beat * 4.0).round()).abs();
            assert!(off < 0.002, "step {step} at beat {beat}");
        }
    }

    #[test]
    fn stays_on_the_leader_through_a_tempo_ramp() {
        // The leader speeds up from 120 to 130 BPM over 4 seconds.
        let beat_at = |t: f64| 120.0 / 60.0 * t + 10.0 / 60.0 * t * t / 8.0;
        let bpm_at = |t: f64| 120.0 + 10.0 * t / 4.0;
        let leader = |frame: f64| {
            let t = frame / RATE;
            Source {
                bpm: bpm_at(t),
                beat: Some(beat_at(t)),
            }
        };
        let s = step_frames(512, 4 * 48_000, 0.0, leader);
        assert!(s.len() > 30, "{}", s.len());
        for &(f, step) in s.iter().skip(4) {
            let beat = beat_at(f as f64 / RATE);
            // Within about a frame's worth of beat (well under a millisecond).
            let off = (beat * 4.0 - (beat * 4.0).round()).abs() / 4.0;
            assert!(off < 0.02, "step {step} off by {off} beats");
        }
    }

    #[test]
    fn a_jump_on_the_leader_moves_the_groove_at_the_next_bar() {
        // The leader plays at 120 BPM; half a second (one beat) in, it jumps
        // back by a beat and a half (a hot cue).
        let leader = |frame: f64| {
            let beat = frame * 2.0 / RATE;
            Source {
                bpm: 120.0,
                beat: Some(if frame >= 24_000.0 { beat - 1.5 } else { beat }),
            }
        };
        let s = step_frames(480, 144_000, 0.0, leader);
        // Through the first bar the steps run on: 0..15 every 6000 frames.
        for (i, &(f, step)) in s.iter().take(16).enumerate() {
            assert_eq!(step, i, "{:?}", &s[..18]);
            assert!((f as i64 - i as i64 * 6000).abs() <= 1);
        }
        // From the bar line on, the groove plays the leader's beats.
        assert!(s.len() > 20);
        for &(f, step) in s.iter().skip(16) {
            let beat = f as f64 * 2.0 / RATE - 1.5;
            let expected = ((beat * 4.0).round() as i64).rem_euclid(16) as usize;
            assert_eq!(step, expected, "frame {f}");
        }
    }

    #[test]
    fn own_tempo_without_a_leader_or_link() {
        assert_eq!(pick_source(None, None, 100.0), own(100.0));
        let link = Some((100.0, 7.0));
        assert_eq!(pick_source(None, link, 120.0).beat, Some(7.0));
        let leader = Some((124.0, 3.0));
        assert_eq!(pick_source(leader, link, 120.0).bpm, 124.0);
    }

    fn groove_box() -> (Arc<ControlRegistry>, GrooveBox) {
        let reg = Arc::new(ControlRegistry::default());
        let kits: Arc<[Kit]> = kits::build(RATE).into();
        let g = GrooveBox::new(&reg, 0, kits, RATE);
        (reg, g)
    }

    #[test]
    fn plays_the_kick_on_the_beat_and_mutes() {
        let (_reg, mut g) = groove_box();
        // One kit, only the kick, four to the bar.
        g.c.kit.set(1.0);
        for r in 1..ROWS {
            g.c.patterns[0][r].set(0.0);
        }
        g.c.patterns[0][0].set(kits::parse_steps("x...x...x...x...") as f64);
        let mut out = vec![[0.0; 2]; 6000 * 4];
        let mut kicks = Vec::new();
        g.render(true, own(120.0), &mut out, |f| kicks.push(f));
        assert_eq!(kicks, vec![0]);
        assert!(
            out[..200].iter().any(|s| s[0].abs() > 0.05),
            "the kick sounds"
        );
        // Muted, the next beat is silent once the first kick has died away.
        g.c.mute[0].set_bool(true);
        let mut out = vec![[0.0; 2]; 6000 * 4 * 12];
        let mut kicks = Vec::new();
        g.render(true, own(120.0), &mut out, |f| kicks.push(f));
        assert!(kicks.is_empty());
        assert!(out[out.len() - 100..].iter().all(|s| s[0].abs() < 1e-3));
    }

    #[test]
    fn a_new_groove_waits_for_the_bar() {
        let (_reg, mut g) = groove_box();
        g.c.groove.set(0.0);
        let mut out = vec![[0.0; 2]; 6000];
        g.render(true, own(120.0), &mut out, |_| {});
        g.c.groove.set(1.0);
        g.render(true, own(120.0), &mut out, |_| {});
        assert_eq!(g.c.groove_queued.get(), 1.0);
        assert_eq!(g.playing, 0, "still the first groove mid-bar");
        // Render through to the bar line (16 steps of 6000 frames).
        for _ in 0..15 {
            g.render(true, own(120.0), &mut out, |_| {});
        }
        assert_eq!(g.playing, 1);
    }

    #[test]
    fn grooves_load_from_the_library() {
        let (_reg, g) = groove_box();
        let first = patterns::groove(0, 0).unwrap();
        assert_eq!(g.c.patterns[0][0].get() as u32, first.rows[0].hits);
        g.c.groove.set(2.0);
        g.c.load_groove(2, 0, 3);
        let third = patterns::groove(0, 2).unwrap();
        assert_eq!(g.c.patterns[2][1].get() as u32, third.rows[1].hits);
        assert_eq!(g.c.slots(), (2, 3));
    }

    #[test]
    fn a_chain_takes_turns_a_bar_each() {
        let (_reg, mut g) = groove_box();
        g.c.chain.set(2.0);
        g.c.load_chain.set(1.0);
        let mut out = vec![[0.0; 2]; 6000];
        let mut heard = Vec::new();
        // Three bars of 16 steps at 120 BPM (6000 frames a step at 48 kHz).
        for _ in 0..48 {
            g.render(true, own(120.0), &mut out, |_| {});
            heard.push(g.c.groove_playing.get() as usize);
        }
        assert_eq!(&heard[..15], &[0; 15], "A plays the first bar");
        assert!(heard[17..31].iter().all(|&p| p == 1), "then B: {heard:?}");
        assert!(heard[33..47].iter().all(|&p| p == 0), "then A again");
        assert_ne!(g.c.patterns[1][0].get(), 0.0, "B holds the chained groove");
    }

    #[test]
    fn soft_steps_play_quieter() {
        let (_reg, mut g) = groove_box();
        for r in 0..ROWS {
            g.c.patterns[0][r].set(0.0);
        }
        g.c.patterns[0][0].set(1.0);
        let mut loud = vec![[0.0; 2]; 2000];
        g.render(true, own(120.0), &mut loud, |_| {});
        let (_reg, mut g) = groove_box();
        for r in 0..ROWS {
            g.c.patterns[0][r].set(0.0);
        }
        g.c.patterns[0][0].set(1.0);
        g.c.soft[0][0].set(1.0);
        let mut soft = vec![[0.0; 2]; 2000];
        g.render(true, own(120.0), &mut soft, |_| {});
        let peak = |v: &[[f32; 2]]| v.iter().fold(0.0f32, |m, s| m.max(s[0].abs()));
        assert!((peak(&soft) / peak(&loud) - 0.5).abs() < 0.01);
    }
}
