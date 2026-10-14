//! Automix: plays the Automix list on two decks, crossfading from one track
//! into the next, like Mixxx's Auto DJ.
//!
//! Everything goes through the same controls a user or controller moves
//! (crossfader, volume, EQ, filter, play), so the mix shows on screen and
//! moving the crossfader during a mix hands it back to the user. Mixxx's
//! `[AutoDJ]` controls are created too, so controller mappings that start
//! Auto DJ, fade now or skip the next track work.
//!
//! Times are measured on the outgoing track, in its own seconds, so a mix
//! follows the deck's tempo and pauses when the deck does.

use std::sync::Arc;

use mixxx_control::{ControlHandle, ControlRegistry};
use mixxx_engine::{deck_group, eq_group, quick_effect_group, Track, NUM_DECKS};

use crate::AppWindow;

/// What Automix needs from the rest of the app: the Automix list and a way
/// to put tracks on decks.
pub trait Host {
    /// Takes the first track off the Automix list.
    fn take_next(&mut self) -> Option<Arc<Track>>;
    /// Tracks left on the Automix list.
    fn queued(&self) -> usize;
    fn load(&mut self, deck: usize, track: Arc<Track>);
}

/// Mix lengths offered, in the order the panel shows them.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Length {
    Seconds(f64),
    Bars(f64),
}

pub const LENGTHS: [Length; 5] = [
    Length::Seconds(12.0),
    Length::Seconds(15.0),
    Length::Seconds(30.0),
    Length::Bars(16.0),
    Length::Bars(32.0),
];

/// How the crossfade moves, in the order the panel shows them.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Style {
    /// The crossfader moves evenly across the whole mix.
    Smooth,
    /// As Smooth, with the bass of the two tracks swapped halfway.
    BassSwap,
    /// As Smooth, while the outgoing track is high-passed away.
    Filter,
    /// Switches over in one beat, where the mix would start.
    Cut,
}

pub const STYLES: [Style; 4] = [Style::Smooth, Style::BassSwap, Style::Filter, Style::Cut];

/// Waveform level under which a slice counts as silence: about -60 dB
/// relative to the track's peak (slices hold the square root of the level).
/// Higher than that cuts off quiet fade-outs, so a 15 s mix would start
/// well before the last 15 s the deck shows.
const SILENCE: u8 = 8;
/// Crossfader movement by someone else, beyond this, stops a mix.
const TAKEOVER: f64 = 0.02;
/// How fast Automix slides the crossfader outside a mix, in crossfader
/// units per second: from the centre to one side in a second and a half.
const GLIDE_SPEED: f64 = 1.0 / 1.5;

struct DeckHandles {
    play: ControlHandle,
    playposition: ControlHandle,
    volume: ControlHandle,
    rate: ControlHandle,
    rate_range: ControlHandle,
    beatsync: ControlHandle,
    eq_low: ControlHandle,
    filter: ControlHandle,
}

impl DeckHandles {
    fn new(reg: &ControlRegistry, deck: usize) -> Self {
        let g = deck_group(deck);
        let get = |group: &str, key: &str| {
            reg.get(group, key)
                .unwrap_or_else(|| panic!("engine control {group},{key}"))
        };
        Self {
            play: get(&g, "play"),
            playposition: get(&g, "playposition"),
            volume: get(&g, "volume"),
            rate: get(&g, "rate"),
            rate_range: get(&g, "rateRange"),
            beatsync: get(&g, "beatsync"),
            eq_low: get(&eq_group(deck), "parameter1"),
            filter: get(&quick_effect_group(deck), "super1"),
        }
    }

    fn speed(&self) -> f64 {
        1.0 + self.rate.get() * self.rate_range.get()
    }
}

/// A press of a push control since the last look, even one shorter than a
/// UI frame.
struct Press {
    control: ControlHandle,
    version: u32,
}

impl Press {
    fn new(control: ControlHandle) -> Self {
        let version = control.version();
        Self { control, version }
    }

    fn pressed(&mut self) -> bool {
        let v = self.control.version();
        let changes = v.wrapping_sub(self.version);
        self.version = v;
        let pressed = changes > 0 && (self.control.get_bool() || changes >= 2);
        if self.control.get_bool() {
            // Leave it released so the next press counts again.
            self.control.set(0.0);
            self.version = self.control.version();
        }
        pressed
    }
}

/// The next track, put on the silent deck and cued to where its sound starts.
#[derive(Clone, Copy)]
struct Next {
    id: usize,
    cued: bool,
}

struct Mix {
    from: usize,
    to: usize,
    style: Style,
    /// The mix's span on the outgoing track, in its seconds.
    start: f64,
    end: f64,
    /// Where the crossfader was when the mix started.
    xfade_from: f64,
    /// What the mix last set the crossfader to, to notice someone else moving it.
    xfade_set: f64,
    /// EQ low and filter of both decks before the mix, put back after it.
    saved_low: [f64; NUM_DECKS],
    saved_filter: [f64; NUM_DECKS],
    /// Whether the incoming deck still needs starting (a takeover mixes
    /// into a deck that is already playing).
    start_incoming: bool,
    beat_match: bool,
}

enum Phase {
    Off,
    /// Nothing was playing: the first track is going on this deck.
    Opening {
        deck: usize,
        next: Option<Next>,
    },
    /// `live` plays; the next track waits on the other deck.
    Playing {
        live: usize,
        next: Option<Next>,
    },
    Mixing(Mix),
}

/// The crossfader sliding to `target` by itself, outside a mix: to the live
/// deck when Automix starts, to the centre when it is switched off.
struct Glide {
    target: f64,
    /// What the glide last set the crossfader to, to notice someone else moving it.
    set: f64,
}

pub struct AutoMix {
    decks: [DeckHandles; NUM_DECKS],
    crossfader: ControlHandle,
    enabled: ControlHandle,
    fade_now: Press,
    skip_next: Press,
    phase: Phase,
    glide: Option<Glide>,
    length: usize,
    style: usize,
    /// A note for the panel when Automix stops by itself or can't start.
    note: String,
}

fn side(deck: usize) -> f64 {
    if deck == 0 {
        -1.0
    } else {
        1.0
    }
}

/// Where the track's sound starts, in seconds, once that much is analysed.
fn sound_start(t: &Track) -> Option<f64> {
    let w = &t.waveform;
    let bins = w.levels.first()?;
    let rate = w.bins_per_second;
    if rate <= 0.0 {
        return None;
    }
    match bins.iter().position(|b| b[3] >= SILENCE) {
        Some(i) => Some(i as f64 / rate),
        // Silent so far: wait unless the whole track is analysed.
        None if bins.len() as f64 / rate >= t.duration - 0.5 => Some(0.0),
        None => None,
    }
}

/// Where the track's sound ends, in seconds; the track's end until it is
/// fully analysed.
fn sound_end(t: &Track) -> f64 {
    let w = &t.waveform;
    let Some(bins) = w.levels.first() else {
        return t.duration;
    };
    let rate = w.bins_per_second;
    if rate <= 0.0 || (bins.len() as f64 / rate) < t.duration - 0.5 {
        return t.duration;
    }
    match bins.iter().rposition(|b| b[3] >= SILENCE) {
        Some(i) => ((i + 1) as f64 / rate).min(t.duration),
        None => t.duration,
    }
}

fn fmt_countdown(seconds: f64) -> String {
    let s = seconds.max(0.0).ceil() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

impl AutoMix {
    pub fn new(reg: &ControlRegistry) -> Self {
        let length = crate::settings::automix_length().min(LENGTHS.len() - 1);
        let style = crate::settings::automix_style().min(STYLES.len() - 1);
        Self {
            decks: std::array::from_fn(|d| DeckHandles::new(reg, d)),
            crossfader: reg.get("[Master]", "crossfader").expect("crossfader"),
            enabled: reg.toggle("[AutoDJ]", "enabled"),
            fade_now: Press::new(reg.push("[AutoDJ]", "fade_now")),
            skip_next: Press::new(reg.push("[AutoDJ]", "skip_next")),
            phase: Phase::Off,
            glide: None,
            length,
            style,
            note: String::new(),
        }
    }

    pub fn toggle(&mut self) {
        self.enabled.toggle();
    }

    pub fn mix_now(&mut self) {
        self.fade_now.control.set(1.0);
    }

    pub fn skip(&mut self) {
        self.skip_next.control.set(1.0);
    }

    pub fn set_length(&mut self, i: usize) {
        if i < LENGTHS.len() {
            self.length = i;
            crate::settings::set_automix_length(i);
        }
    }

    pub fn set_style(&mut self, i: usize) {
        if i < STYLES.len() {
            self.style = i;
            crate::settings::set_automix_style(i);
        }
    }

    /// Runs Automix for one UI frame, `dt` seconds after the last one.
    pub fn tick(&mut self, host: &mut dyn Host, tracks: &[Option<Arc<Track>>; NUM_DECKS], dt: f64) {
        let on = self.enabled.get_bool();
        match (&self.phase, on) {
            (Phase::Off, true) => self.start(host, tracks),
            (Phase::Off, false) => {}
            (_, false) => {
                // Switched off by the user: bring both decks back in.
                self.stop("");
                self.glide_to(0.0);
            }
            _ => self.step(host, tracks),
        }
        self.slide(dt);
        // Presses while nothing could use them don't wait for later.
        self.fade_now.pressed();
        self.skip_next.pressed();
    }

    /// Switches Automix off. A mix in progress stays where it is, with the
    /// EQ and filter Automix moved put back.
    fn stop(&mut self, note: &str) {
        if let Phase::Mixing(m) = &self.phase {
            self.restore(m);
        }
        self.glide = None;
        self.phase = Phase::Off;
        self.enabled.set_bool(false);
        self.note = note.to_string();
    }

    /// Automix was switched on.
    fn start(&mut self, host: &mut dyn Host, tracks: &[Option<Arc<Track>>; NUM_DECKS]) {
        self.note.clear();
        self.glide = None;
        let playing: Vec<usize> = (0..NUM_DECKS)
            .filter(|&d| self.decks[d].play.get_bool() && tracks[d].is_some())
            .collect();
        if playing.is_empty() {
            if host.queued() == 0 {
                self.stop("Add tracks to the Automix list first.");
                return;
            }
            self.phase = Phase::Opening {
                deck: 0,
                next: None,
            };
            return;
        }
        // The live deck is the loudest one playing, counting the crossfader.
        let x = self.crossfader.get();
        let level = |d: usize| {
            // The engine's constant-power curve.
            let a = (x + 1.0) * std::f64::consts::FRAC_PI_4;
            let gain = if d == 0 { a.cos() } else { a.sin() };
            self.decks[d].volume.get() * (gain * std::f64::consts::SQRT_2).min(1.0)
        };
        let live = *playing
            .iter()
            .max_by(|&&a, &&b| level(a).total_cmp(&level(b)))
            .unwrap();
        let other = 1 - live;
        if self.decks[other].play.get_bool() {
            // Fade the other deck out, then carry on from the live one.
            let pos = self.position(other, tracks);
            let len = self
                .length_on(other, tracks)
                .min(self.remaining(other, tracks));
            self.phase = Phase::Mixing(Mix {
                from: other,
                to: live,
                style: Style::Smooth,
                start: pos,
                end: pos + len,
                xfade_from: x,
                xfade_set: x,
                saved_low: self.decks.each_ref().map(|d| d.eq_low.get()),
                saved_filter: self.decks.each_ref().map(|d| d.filter.get()),
                start_incoming: false,
                beat_match: false,
            });
        } else {
            self.glide_to(side(live));
            self.phase = Phase::Playing { live, next: None };
        }
    }

    /// Starts the crossfader sliding to `target`.
    fn glide_to(&mut self, target: f64) {
        self.glide = Some(Glide {
            target,
            set: self.crossfader.get(),
        });
    }

    /// Moves a glide on by `dt` seconds. Moving the crossfader by hand ends it.
    fn slide(&mut self, dt: f64) {
        let Some(g) = &mut self.glide else {
            return;
        };
        let x = self.crossfader.get();
        if (x - g.set).abs() > TAKEOVER {
            self.glide = None;
            return;
        }
        let step = GLIDE_SPEED * dt.max(0.0);
        let next = if (g.target - x).abs() <= step {
            g.target
        } else {
            x + step * (g.target - x).signum()
        };
        self.crossfader.set(next);
        g.set = self.crossfader.get();
        if next == g.target {
            self.glide = None;
        }
    }

    fn step(&mut self, host: &mut dyn Host, tracks: &[Option<Arc<Track>>; NUM_DECKS]) {
        match std::mem::replace(&mut self.phase, Phase::Off) {
            Phase::Off => {}
            Phase::Opening { deck, next } => {
                let next = self.prepare(deck, next, host, tracks);
                match next {
                    None => self.stop("Add tracks to the Automix list first."),
                    Some(Next { cued: true, .. }) => {
                        self.glide_to(side(deck));
                        let d = &self.decks[deck];
                        if d.volume.get() < 0.05 {
                            d.volume.set(1.0);
                        }
                        d.play.set_bool(true);
                        self.phase = Phase::Playing {
                            live: deck,
                            next: None,
                        };
                    }
                    next => self.phase = Phase::Opening { deck, next },
                }
            }
            Phase::Playing { live, next } => self.wait(live, next, host, tracks),
            Phase::Mixing(mut m) => {
                self.glide = None;
                if (self.crossfader.get() - m.xfade_set).abs() > TAKEOVER {
                    self.phase = Phase::Mixing(m);
                    self.stop("Stopped: the crossfader was moved.");
                    return;
                }
                let done =
                    !self.decks[m.from].play.get_bool() || self.position(m.from, tracks) >= m.end;
                if done {
                    self.finish(&m);
                    self.decks[m.from].play.set_bool(false);
                    self.phase = Phase::Playing {
                        live: m.to,
                        next: None,
                    };
                } else {
                    self.apply(&mut m, tracks);
                    self.phase = Phase::Mixing(m);
                }
            }
        }
    }

    /// While `live` plays: keeps the next track ready on the other deck and
    /// starts the mix when it is due.
    fn wait(
        &mut self,
        live: usize,
        next: Option<Next>,
        host: &mut dyn Host,
        tracks: &[Option<Arc<Track>>; NUM_DECKS],
    ) {
        let other = 1 - live;
        let mut next = next;
        if self.skip_next.pressed() && host.queued() > 0 {
            next = None;
        }
        let ended = self.remaining(live, tracks) < 0.3;
        if ended || !self.decks[live].play.get_bool() {
            if !ended {
                self.stop("Stopped: the deck was paused.");
            } else if next.is_some() || host.queued() > 0 {
                // The track ran out before the next one was ready: start that
                // one as soon as it is.
                self.decks[live].play.set_bool(false);
                self.phase = Phase::Opening { deck: other, next };
            } else {
                self.decks[live].play.set_bool(false);
                self.stop("The Automix list has finished.");
            }
            return;
        }
        // Don't touch a deck the user has started playing.
        if !self.decks[other].play.get_bool() {
            next = self.prepare(other, next, host, tracks);
        }
        let Some(n) = next else {
            self.phase = Phase::Playing { live, next };
            return;
        };
        let pos = self.position(live, tracks);
        let end = tracks[live].as_deref().map_or(pos, sound_end);
        let len = self.length_on(live, tracks);
        let style = STYLES[self.style];
        let mut start = (end - len).max(0.0);
        if let (Length::Bars(_), Some(t)) = (LENGTHS[self.length], tracks[live].as_deref()) {
            if t.bpm > 0.0 {
                // Start on a bar of the outgoing track.
                let bar = 4.0 * t.beat_length();
                start = (t.first_beat + ((start - t.first_beat) / bar).floor() * bar).max(0.0);
            }
        }
        let now = self.fade_now.pressed();
        if !n.cued || !(now || pos >= start) {
            self.phase = Phase::Playing { live, next };
            return;
        }
        let (start, mut end) = if now || pos > start + 0.5 {
            (pos, (pos + len).min(end.max(pos + 1.0)))
        } else {
            (start, (start + len).min(end.max(start + 1.0)))
        };
        if style == Style::Cut {
            let beat = tracks[live].as_deref().map_or(0.5, Track::beat_length);
            end = start + beat;
        }
        let beat_match = matches!(LENGTHS[self.length], Length::Bars(_))
            && match (tracks[live].as_deref(), tracks[other].as_deref()) {
                (Some(a), Some(b)) if a.bpm > 0.0 && b.bpm > 0.0 => {
                    let target = a.bpm * self.decks[live].speed();
                    (target / b.bpm - 1.0).abs() <= self.decks[other].rate_range.get()
                }
                _ => false,
            };
        self.glide = None;
        let x = self.crossfader.get();
        let mut m = Mix {
            from: live,
            to: other,
            style,
            start,
            end,
            xfade_from: x,
            xfade_set: x,
            saved_low: self.decks.each_ref().map(|d| d.eq_low.get()),
            saved_filter: self.decks.each_ref().map(|d| d.filter.get()),
            start_incoming: true,
            beat_match,
        };
        self.apply(&mut m, tracks);
        self.phase = Phase::Mixing(m);
    }

    /// Makes sure the next track from the list is on `deck`, cued to where
    /// its sound starts. None when the list is empty.
    fn prepare(
        &mut self,
        deck: usize,
        next: Option<Next>,
        host: &mut dyn Host,
        tracks: &[Option<Arc<Track>>; NUM_DECKS],
    ) -> Option<Next> {
        let mut next = match next {
            Some(n) => n,
            None => {
                let t = host.take_next()?;
                host.load(deck, t.clone());
                Next {
                    id: t.id,
                    cued: false,
                }
            }
        };
        if !next.cued {
            if let Some(t) = tracks[deck].as_deref().filter(|t| t.id == next.id) {
                if let Some(start) = sound_start(t).filter(|_| t.duration > 0.0) {
                    let d = &self.decks[deck];
                    d.play.set_bool(false);
                    d.playposition.set(start / t.duration);
                    next.cued = true;
                }
            }
        }
        Some(next)
    }

    /// Moves the controls to where the mix is now.
    fn apply(&mut self, m: &mut Mix, tracks: &[Option<Arc<Track>>; NUM_DECKS]) {
        let pos = self.position(m.from, tracks);
        let p = ((pos - m.start) / (m.end - m.start).max(1e-3)).clamp(0.0, 1.0);
        let (from, to) = (&self.decks[m.from], &self.decks[m.to]);
        if m.start_incoming {
            m.start_incoming = false;
            to.volume.set(from.volume.get().max(0.05));
            if m.style == Style::BassSwap {
                to.eq_low.set(0.0);
            }
            to.play.set_bool(true);
            if m.beat_match {
                to.beatsync.set(1.0);
            }
        } else if m.beat_match && to.beatsync.get_bool() {
            to.beatsync.set(0.0);
        }
        // Even a cut slides, over the one beat it lasts.
        let x = m.xfade_from + (side(m.to) - m.xfade_from) * p;
        self.crossfader.set(x);
        m.xfade_set = self.crossfader.get();
        match m.style {
            Style::BassSwap => {
                // Swapped over the middle tenth of the mix.
                let k = ((p - 0.45) / 0.1).clamp(0.0, 1.0);
                from.eq_low.set(m.saved_low[m.from] * (1.0 - k));
                to.eq_low.set(m.saved_low[m.to] * k);
            }
            Style::Filter => from.filter.set(0.5 + 0.4 * p),
            Style::Smooth | Style::Cut => {}
        }
    }

    /// Ends a mix: the crossfader on its way to the incoming deck (it is
    /// there already unless the mix was cut short), the EQ and filter
    /// Automix moved put back.
    fn finish(&mut self, m: &Mix) {
        self.restore(m);
        self.glide_to(side(m.to));
    }

    fn restore(&self, m: &Mix) {
        for d in [m.from, m.to] {
            self.decks[d].eq_low.set(m.saved_low[d]);
            self.decks[d].filter.set(m.saved_filter[d]);
        }
    }

    /// Where `deck` is in its track, in the track's seconds.
    fn position(&self, deck: usize, tracks: &[Option<Arc<Track>>; NUM_DECKS]) -> f64 {
        tracks[deck]
            .as_deref()
            .map_or(0.0, |t| self.decks[deck].playposition.get() * t.duration)
    }

    /// Seconds of sound left on `deck`'s track, in the track's seconds.
    fn remaining(&self, deck: usize, tracks: &[Option<Arc<Track>>; NUM_DECKS]) -> f64 {
        tracks[deck]
            .as_deref()
            .map_or(0.0, |t| sound_end(t) - self.position(deck, tracks))
    }

    /// The chosen mix length, in seconds of `deck`'s track.
    fn length_on(&self, deck: usize, tracks: &[Option<Arc<Track>>; NUM_DECKS]) -> f64 {
        match LENGTHS[self.length] {
            Length::Seconds(s) => s * self.decks[deck].speed().max(0.1),
            Length::Bars(n) => {
                let beat = tracks[deck].as_deref().map_or(0.5, Track::beat_length);
                n * 4.0 * beat
            }
        }
    }

    /// Shows Automix's state on the panel.
    pub fn show(&self, app: &AppWindow, queued: usize, tracks: &[Option<Arc<Track>>; NUM_DECKS]) {
        let title = |d: usize| {
            tracks[d]
                .as_deref()
                .map(|t| t.title.clone())
                .unwrap_or_default()
        };
        let status = match &self.phase {
            Phase::Off => self.note.clone(),
            Phase::Opening { .. } => "Starting…".to_string(),
            Phase::Playing { live, next } => match next {
                Some(Next { cued: true, .. }) => {
                    let left = self.remaining(*live, tracks) - self.length_on(*live, tracks);
                    let speed = self.decks[*live].speed().max(0.1);
                    format!(
                        "Mixing into {} in {}",
                        title(1 - live),
                        fmt_countdown(left / speed)
                    )
                }
                Some(_) => "Loading the next track…".to_string(),
                None if queued == 0 => "Last track. Add more to keep going.".to_string(),
                None => String::new(),
            },
            Phase::Mixing(m) if m.start_incoming || !self.decks[m.to].play.get_bool() => {
                String::new()
            }
            Phase::Mixing(m) => format!("Mixing into {}", title(m.to)),
        };
        let on = !matches!(self.phase, Phase::Off);
        if app.get_automix_on() != on {
            app.set_automix_on(on);
        }
        if app.get_automix_status() != status.as_str() {
            app.set_automix_status(status.into());
        }
        if app.get_automix_length() != self.length as i32 {
            app.set_automix_length(self.length as i32);
        }
        if app.get_automix_style() != self.style as i32 {
            app.set_automix_style(self.style as i32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mixxx_engine::engine::{Command, Engine, EngineInput};
    use mixxx_engine::loader::prepare;
    use mixxx_engine::synth::sample_library;

    const SR: f64 = 48_000.0;
    const FPS: f64 = 60.0;

    struct TestHost {
        queue: Vec<Arc<Track>>,
        input: EngineInput,
        tracks: [Option<Arc<Track>>; NUM_DECKS],
    }

    impl Host for TestHost {
        fn take_next(&mut self) -> Option<Arc<Track>> {
            (!self.queue.is_empty()).then(|| self.queue.remove(0))
        }
        fn queued(&self) -> usize {
            self.queue.len()
        }
        fn load(&mut self, deck: usize, track: Arc<Track>) {
            let t = Arc::new(prepare(&track).unwrap());
            self.input
                .send(Command::Load {
                    deck,
                    track: t.clone(),
                })
                .ok()
                .unwrap();
            self.tracks[deck] = Some(t);
        }
    }

    fn track(n: usize, seconds: f64) -> Arc<Track> {
        let mut t = (*sample_library()[n]).clone();
        t.id = 1000 + n;
        t.bpm = 120.0;
        t.duration = seconds;
        Arc::new(t)
    }

    struct Rig {
        reg: Arc<ControlRegistry>,
        engine: Engine,
        host: TestHost,
        automix: AutoMix,
    }

    impl Rig {
        fn new(queue: Vec<Arc<Track>>) -> Self {
            let reg = ControlRegistry::new();
            let (engine, input) = Engine::new(&reg, SR);
            let mut automix = AutoMix::new(&reg);
            automix.length = 0;
            automix.style = 0;
            Self {
                reg,
                engine,
                host: TestHost {
                    queue,
                    input,
                    tracks: Default::default(),
                },
                automix,
            }
        }

        fn ctl(&self, group: &str, key: &str) -> ControlHandle {
            self.reg.get(group, key).unwrap()
        }

        fn deck(&self, d: usize, key: &str) -> ControlHandle {
            self.ctl(&deck_group(d), key)
        }

        fn xfade(&self) -> f64 {
            self.ctl("[Master]", "crossfader").get()
        }

        /// Runs the engine and Automix together, a UI frame at a time.
        fn run(&mut self, seconds: f64) {
            let frames = (SR / FPS) as usize;
            let mut buf = vec![0.0f32; frames * 2];
            for _ in 0..(seconds * FPS).round() as usize {
                self.engine.process(&mut buf, 2);
                let tracks = self.host.tracks.clone();
                self.automix.tick(&mut self.host, &tracks, 1.0 / FPS);
            }
        }

        fn loaded_id(&self, d: usize) -> Option<usize> {
            self.host.tracks[d].as_ref().map(|t| t.id)
        }
    }

    #[test]
    fn plays_the_list_with_crossfades() {
        let mut r = Rig::new(vec![track(0, 20.0), track(1, 40.0), track(2, 20.0)]);
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(2.0);
        assert!(r.deck(0, "play").get_bool(), "first track plays on deck A");
        assert_eq!(r.xfade(), -1.0, "crossfader slid over to deck A");
        assert_eq!(r.loaded_id(0), Some(1000));
        assert_eq!(r.loaded_id(1), Some(1001), "next track waits on deck B");
        assert!(!r.deck(1, "play").get_bool());
        assert_eq!(r.host.queued(), 1);

        // A 12 s mix ends where deck A's sound ends, so it is under way by 14 s.
        r.run(12.0);
        assert!(r.deck(1, "play").get_bool(), "deck B has come in");
        let x = r.xfade();
        assert!(x > -0.9 && x < 0.9, "crossfader mid-way: {x}");

        r.run(7.0);
        assert_eq!(r.xfade(), 1.0);
        assert!(
            !r.deck(0, "play").get_bool(),
            "deck A stopped after the mix"
        );
        assert_eq!(r.loaded_id(0), Some(1002), "third track on deck A");
        assert_eq!(r.host.queued(), 0);
        assert!(r.ctl("[AutoDJ]", "enabled").get_bool());

        // The last track mixes in, plays out, and Automix stops.
        r.run(50.0);
        assert!(!r.ctl("[AutoDJ]", "enabled").get_bool());
        assert_eq!(r.automix.note, "The Automix list has finished.");
    }

    #[test]
    fn carries_on_from_the_loudest_deck() {
        let mut r = Rig::new(vec![track(2, 40.0)]);
        r.host.load(0, track(0, 40.0));
        r.host.load(1, track(1, 40.0));
        r.run(0.1);
        r.deck(0, "play").set(1.0);
        r.deck(1, "play").set(1.0);
        r.deck(0, "volume").set(0.3);
        r.ctl("[Master]", "crossfader").set(0.0);
        r.run(0.5);
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(6.0);
        assert!(r.xfade() > 0.0, "fading towards deck B: {}", r.xfade());
        r.run(8.0);
        assert_eq!(r.xfade(), 1.0);
        assert!(r.deck(1, "play").get_bool(), "deck B keeps playing");
        assert!(!r.deck(0, "play").get_bool(), "deck A stopped");
        assert_eq!(r.loaded_id(0), Some(1002), "first of the list on deck A");
    }

    #[test]
    fn moving_the_crossfader_takes_over() {
        let mut r = Rig::new(vec![track(0, 20.0), track(1, 20.0)]);
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(1.0);
        r.automix.fade_now.control.set(1.0);
        r.run(2.0);
        assert!(r.deck(1, "play").get_bool());
        r.ctl("[Master]", "crossfader").set(-0.5);
        r.run(0.1);
        assert!(!r.ctl("[AutoDJ]", "enabled").get_bool());
        assert_eq!(r.xfade(), -0.5, "left where the user put it");
    }

    #[test]
    fn bass_swap_puts_the_eq_back() {
        let mut r = Rig::new(vec![track(0, 20.0), track(1, 20.0)]);
        r.automix.style = 1;
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(1.0);
        r.automix.fade_now.control.set(1.0);
        r.run(1.0);
        let low = |r: &Rig, d| r.ctl(&eq_group(d), "parameter1").get();
        assert_eq!(low(&r, 1), 0.0, "incoming bass out before the swap");
        assert_eq!(low(&r, 0), 1.0);
        r.run(12.0);
        assert_eq!(r.xfade(), 1.0);
        assert_eq!(low(&r, 0), 1.0);
        assert_eq!(low(&r, 1), 1.0);
    }

    #[test]
    fn slides_to_the_live_deck_and_back_to_the_centre() {
        let mut r = Rig::new(vec![track(1, 40.0)]);
        r.host.load(0, track(0, 40.0));
        r.run(0.1);
        r.deck(0, "play").set(1.0);
        r.ctl("[Master]", "crossfader").set(0.0);
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(0.5);
        let x = r.xfade();
        assert!(x < -0.1 && x > -0.9, "sliding to deck A, not jumping: {x}");
        r.run(1.5);
        assert_eq!(r.xfade(), -1.0);

        // Switching Automix off slides the crossfader back to the middle.
        r.ctl("[AutoDJ]", "enabled").set(0.0);
        r.run(0.5);
        let x = r.xfade();
        assert!(x > -0.9 && x < -0.1, "sliding back: {x}");
        r.run(1.5);
        assert_eq!(r.xfade(), 0.0);
        assert!(r.deck(0, "play").get_bool(), "the music keeps playing");
    }

    #[test]
    fn a_cut_still_slides() {
        let mut r = Rig::new(vec![track(0, 20.0), track(1, 20.0)]);
        r.automix.style = 3;
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(2.0);
        r.automix.fade_now.control.set(1.0);
        r.run(2.0 / FPS);
        let x = r.xfade();
        assert!(x > -1.0 && x < 0.9, "part way after a frame: {x}");
        r.run(2.0);
        assert_eq!(r.xfade(), 1.0);
    }
}
