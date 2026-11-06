//! The real-time part: runs inside the audio callback, reads controls, plays
//! the decks through the EQ, filter and effect units, mixes them with the
//! microphone and auxiliary inputs and updates the meters.
//!
//! Nothing here allocates or locks. Tracks arrive over a wait-free queue and
//! replaced or ejected tracks go back over another, so they are freed off this
//! thread.

use std::sync::Arc;
use std::time::{Duration, Instant};

use opendj_control::{ControlHandle, ControlRegistry};

use crate::beats::GridEdit;
use crate::controls::{
    create_global_controls, Action, DeckControls, Edge, MasterControls, HOTCUE_CUE, HOTCUE_LOOP,
    SCULPT_DEFAULTS,
};
use crate::dsp::{Isolator, Limiter, Sculpt};
use crate::effects::{
    input_target, ColorFx, EffectRack, StemFx, BLOCK, COLOR_FX, MAIN, TAPS, TAP_GAP, TARGETS,
};
use crate::groove::{pick_source, GrooveBox};
use crate::inputs::{
    Ducker, Input, InputFeed, AUXILIARY, AUXILIARY_2, AUXILIARY_3, INSTRUMENT, MICROPHONE,
    NUM_INPUTS,
};
use crate::key::{self, Key};
use crate::keyshift::{transpose_factor, KeyShifter, MAX_RATE};
use crate::link::{phase_error, Link, Moment};
use crate::preview::{Preview, PreviewInput};
use crate::recorder::{self, Feed, RecordTap};
use crate::sampler::{SamplerInput, Samplers};
use crate::stems::{Stems, MAX_STEMS};
use crate::track::{Audio, Track};
use crate::visual::VisualPositions;
use crate::{NUM_DECKS, NUM_HOTCUES};

pub enum Command {
    /// Puts a track on a deck. Its audio may still be decoding, and its tempo
    /// and first beat may be unknown until [`Command::Analysed`] follows.
    Load { deck: usize, track: Arc<Track> },
    /// The same track (same audio) with its analysis done: tempo, first beat
    /// and exact length. Ignored if the deck has moved on to another track.
    Analysed { deck: usize, track: Arc<Track> },
    /// The same track with an edited beat grid. Nothing else changes.
    Regrid { deck: usize, track: Arc<Track> },
}

/// What the engine hands back to the loader thread.
pub enum Returned {
    /// A track the engine has finished with, to be freed off the audio thread.
    Done(Arc<Track>),
    /// A track ejected from `deck` (`[ChannelN],eject`).
    Ejected { deck: usize, track: Arc<Track> },
    /// A beat grid change asked for on `deck`, at `seconds` into `track`.
    GridEdit {
        deck: usize,
        track: Arc<Track>,
        edit: GridEdit,
        seconds: f64,
    },
}

/// Room for commands and for tracks waiting to be freed.
const QUEUE: usize = 16;

/// The non-real-time side of the engine's queues.
pub struct EngineInput {
    commands: rtrb::Producer<Command>,
    returned: rtrb::Consumer<Returned>,
}

impl EngineInput {
    pub fn send(&mut self, command: Command) -> Result<(), Command> {
        self.commands
            .push(command)
            .map_err(|rtrb::PushError::Full(c)| c)
    }

    /// Frees tracks the engine has finished with, and hands back ejected
    /// tracks and beat grid edits.
    pub fn collect_garbage(&mut self) -> Vec<Returned> {
        let mut out = Vec::new();
        while let Ok(r) = self.returned.pop() {
            if !matches!(r, Returned::Done(_)) {
                out.push(r);
            }
        }
        out
    }
}

/// Track seconds one unit of `jog` moves the play position by.
const JOG_SECONDS: f64 = 0.01;
/// Seconds over which a jog movement is spread, so a turning wheel bends smoothly.
const JOG_SPREAD: f64 = 0.05;
/// Jog speeds above this many times normal speed are a fast search (SHIFT +
/// jog on Pioneer controllers): the deck goes quiet, as in rekordbox.
const JOG_SEARCH_SPEED: f64 = 8.0;
/// How long a deck stays quiet after its last fast-search jog, in seconds,
/// so it doesn't flutter between wheel steps.
const JOG_SEARCH_HOLD: f64 = 0.15;
/// `jog_bend` comes in ticks of a jog's rim, 720 to a turn as on Pioneer
/// controllers. As in Mixxx, where a rim tick is a `jog` of 0.8 and a jog
/// adds a tenth of itself to the rate: each tick a second the rim turns adds
/// 0.08% of normal speed (a quarter turn a second is about +14%).
const BEND_PER_TICK_PER_SECOND: f64 = 0.0008;
/// Seconds the rim's speed is averaged over, like Mixxx's jog filter: the
/// bend follows the hand smoothly and springs back once the rim stops.
const BEND_SMOOTH: f64 = 0.15;
/// Track seconds one rim tick moves a paused deck by.
const BEND_PAUSED_SECONDS: f64 = 0.008;
/// How long a clip lights a PeakIndicator, in seconds.
const PEAK_HOLD: f32 = 0.5;
/// A control set to a value that is used once, put back to 0.
fn take_value(c: &ControlHandle) -> f64 {
    let v = c.get();
    if v != 0.0 {
        c.set(0.0);
    }
    v
}

/// Rate offsets of rate_temp_up, _down, _up_small and _down_small while held
/// (Mixxx's default temporary pitch bends: 4% coarse, 2% fine).
const RATE_TEMP: [f64; 4] = [0.04, -0.04, 0.02, -0.02];
/// How fast pitch reset brings the tempo back to 0%, as a fraction of
/// normal speed per second (0.005 = 0.5% a second).
const RATE_RESET_SPEED: f64 = 0.005;

/// Left and right deck gains for crossfader position `x` (-1 left, +1
/// right). Shapes: 0 smooth, constant power as in Mixxx's default curve; 1
/// linear, dipping in the middle; 2 scratch, both full until the last 5%
/// of travel.
pub(crate) fn xfader_gains(x: f32, shape: u8) -> [f32; 2] {
    let t = (x.clamp(-1.0, 1.0) + 1.0) * 0.5;
    match shape {
        1 => [1.0 - t, t],
        2 => [((1.0 - t) / 0.05).min(1.0), (t / 0.05).min(1.0)],
        _ => {
            let a = t * std::f32::consts::FRAC_PI_2;
            [a.cos(), a.sin()].map(|g| (g * std::f32::consts::SQRT_2).min(1.0))
        }
    }
}

/// Tap tempo: taps in time give a tempo, averaged over the last few.
#[derive(Default)]
struct TapTempo {
    last: Option<f64>,
    intervals: [f64; TAPS],
    taps: usize,
}

impl TapTempo {
    /// A tap at `now` seconds; the tapped BPM once there are two in time.
    fn tap(&mut self, now: f64) -> Option<f64> {
        let gap = self.last.map_or(f64::INFINITY, |last| now - last);
        self.last = Some(now);
        if gap > TAP_GAP || gap <= 0.0 {
            self.taps = 0;
            return None;
        }
        self.intervals.rotate_right(1);
        self.intervals[0] = gap;
        self.taps = (self.taps + 1).min(TAPS);
        let mean = self.intervals[..self.taps].iter().sum::<f64>() / self.taps as f64;
        Some(60.0 / mean)
    }
}

struct Deck {
    c: DeckControls,
    track: Option<Arc<Track>>,
    audio: Option<Arc<Audio>>,
    /// Play position in source frames.
    pos: f64,
    last_written_position: f64,
    loop_start: f64,
    loop_end: f64,
    /// loop_in was pressed and loop_out not yet.
    loop_in_pending: bool,
    /// A beat grid edit pressed this buffer, to hand to the loader.
    grid_edit: Option<(GridEdit, f64)>,
    /// The engine clock this buffer, in seconds, and the deck's `bpm_tap`
    /// taps against it.
    clock: f64,
    taps: TapTempo,
    /// The `bpmlock` version last seen, so the DJ turning it is told apart
    /// from the engine showing a loaded track's lock.
    bpmlock_seen: u32,
    /// The metronome: the last grid beat it passed, and the click playing
    /// (samples into it, and its pitch).
    metronome_beat: f64,
    click: Option<(u32, f32)>,
    /// Playback speed reached at the end of the last buffer, in multiples of
    /// normal speed; negative plays backwards.
    velocity: f64,
    /// Jog movement still to be applied, in track seconds.
    jog_pending: f64,
    /// Seconds the deck stays quiet for after a fast-search jog.
    search_quiet: f64,
    /// How fast the jog's rim is turning, smoothed, in ticks per second.
    bend_rate: f64,
    /// While slipping (a trick in slip mode, reverseroll, a loop roll):
    /// where playback would have been without it, in source frames.
    slip: Option<f64>,
    /// The silent playhead runs on: the deck was playing when the slip
    /// began (or has played since), so it moves on even while paused.
    slip_moving: bool,
    /// The hot cue held in slip mode: it plays from its point while held and
    /// the track carries on from the silent playhead when it is let go.
    slip_cue: Option<usize>,
    rolling: bool,
    /// Whether play was on last buffer: starting and stopping are instant,
    /// other speed changes ramp.
    was_playing: bool,
    cue_edge: Edge,
    cup_edge: Edge,
    zero_edge: Edge,
    /// cue_play was pressed and has not been released yet.
    cup_held: bool,
    /// CUE is held while stopped at the cue point: playing until it is
    /// released, then back to the cue point. Holds play's version when the
    /// preview started, so pressing play meanwhile keeps it playing.
    previewing: Option<u32>,
    eject_edge: Edge,
    sync_edge: Edge,
    was_synced: bool,
    /// Followed a synced leader last buffer.
    following: bool,
    /// While following a sync leader: the speed that matches it, which may
    /// go past the tempo fader's range.
    sync_speed: Option<f64>,
    /// Extra speed, as a fraction, that bends a synced deck back onto the
    /// leader's beat.
    sync_bend: f64,
    /// The synced tempo is the leader's times this (½, 1 or 2), picked when
    /// sync starts and kept while the same two tracks play, so it can't flip.
    sync_ratio: f64,
    /// The two tracks (by address) `sync_ratio` was picked for.
    sync_pair: (usize, usize),
    /// Beats a synced deck stays ahead of its leader, set by nudging either.
    phase_offset: f64,
    /// The rate sync last wrote, to tell a fader move from it.
    rate_written: f64,
    eq: Isolator,
    /// The filter knob's Color FX.
    filter: ColorFx,
    sculpt: Sculpt,
    /// Gains reached at the end of the last buffer; each buffer ramps from these.
    eq_gains: [f32; 3],
    /// The pregain knob times the auto gain, reached at the end of the last buffer.
    pregain: f32,
    /// The gain knob was set from the loaded track's own gain, so the next
    /// track without one puts it back to unity.
    track_gain: bool,
    /// The auto gain alone, as of the last buffer.
    auto_gain: f32,
    volume: f32,
    quick: f32,
    vu: [f32; 2],
    /// Seconds the PeakIndicator stays lit.
    peak_hold: f32,
    /// The loaded track's key, from its tag or analysis.
    file_key: Option<Key>,
    /// Plays the deck through the time-stretcher with key lock or a key shift.
    shifter: KeyShifter,
    /// This buffer's audio from the shifter, when it is in use.
    shifted: Vec<[f32; 2]>,
    /// Low-pass state that softens fast waveform scrubbing.
    scrub_tone: [f32; 2],
    /// Where the play position would be now had nothing jumped, while the
    /// deck plays; a jump from it fades over, see [`Declick`].
    expected_pos: Option<f64>,
    declick: Option<Declick>,
    /// Stem gains reached at the end of the last buffer.
    stem_gains: [f32; MAX_STEMS],
    /// How far the crossfade from the stems made on load to high-quality
    /// ones has got (0 to 1), at the end of the last buffer.
    stem_upgrade: f32,
    /// The stems reported in `stem_count` and the stem colours (by address).
    stems_shown: usize,
    /// The effect the stems with their quick effect on are sent into.
    stem_fx: StemFx,
    /// Each stem's send into `stem_fx` (its gain while its effect is on,
    /// else 0) at the end of the last buffer.
    stem_send: [f32; MAX_STEMS],
    /// Seconds each stem has been sent since its effect went on, and
    /// whether it was muted last buffer: a mute that comes with the effect
    /// waits until the effect holds what was just played (see
    /// [`StemFx::capture_seconds`]).
    stem_fed: [f64; MAX_STEMS],
    stem_muted: [bool; MAX_STEMS],
    /// The send through its own time-stretcher while the deck is key-locked
    /// or shifted, and this buffer's audio from it.
    stem_shifter: KeyShifter,
    stem_shifted: Vec<[f32; 2]>,
    /// Gapless playback: this deck's track ends during this buffer and the
    /// deck plays on past its end into the next track (see `join_after`),
    /// which carries on from there at the start of the next buffer.
    hand_over: Option<usize>,
    /// Gapless playback, the deck whose track carries on from this one's
    /// end; reads past the end (the interpolation, the time-stretcher's
    /// look-ahead) hear its start rather than silence.
    join_after: Option<usize>,
    /// The deck whose track this one carried on from, and that track's
    /// audio (by address), while reads before this one's start hear its end.
    join_before: Option<(usize, usize)>,
}

/// How long after a gapless start reads before the track's start still
/// hear the track before it, in seconds: past the time-stretcher's seed.
const JOIN_SECONDS: f64 = 1.0;

impl Deck {
    fn source_rate(&self) -> f64 {
        self.audio
            .as_ref()
            .map_or(44_100.0, |a| a.sample_rate as f64)
    }

    fn seconds(&self) -> f64 {
        self.pos / self.source_rate()
    }

    fn set_seconds(&mut self, seconds: f64) {
        let Some(t) = &self.track else { return };
        let s = seconds.clamp(0.0, t.duration);
        self.pos = s * self.source_rate();
        self.c.playposition.set(s / t.duration);
        self.last_written_position = self.c.playposition.get();
    }

    fn speed(&self) -> f64 {
        self.sync_speed
            .unwrap_or_else(|| 1.0 + self.c.rate.get() * self.c.rate_range.get() * self.rate_dir())
    }

    /// -1 or 1; see `rate_dir`.
    fn rate_dir(&self) -> f64 {
        if self.c.rate_dir.get() < 0.0 {
            -1.0
        } else {
            1.0
        }
    }

    /// Normal playing speed, including temporary rate changes while held.
    fn forward_speed(&self) -> f64 {
        let temp: f64 = (self.c.rate_temp.iter().zip(RATE_TEMP))
            .filter(|(c, _)| c.get_bool())
            .map(|(_, r)| r)
            .sum();
        self.speed() * (1.0 + self.sync_bend) + temp
    }

    /// Being moved by hand (jog, pitch bend, scratching, searching), which
    /// sync leaves alone.
    fn nudging(&self) -> bool {
        self.jog_pending != 0.0
            || self.bend_rate != 0.0
            || self.c.rate_temp.iter().any(|c| c.get_bool())
            || self.c.scratch2_enable.get_bool()
            || self.c.scrub_enable.get_bool()
            || self.c.seek_hold.get_bool()
            || self.c.rate_search.get() != 0.0
            || self.c.reverse.get_bool()
            || self.c.reverseroll.get_bool()
    }

    /// The quantize step, in beats, for setting cue points and loops: the
    /// deck's quantize size, but never more than a beat, so a loop or cue
    /// isn't pulled back to the start of the bar.
    fn point_step(&self) -> f64 {
        self.quantize_beats().min(1.0)
    }

    /// The deck's quantize size in beats, 1/16 to 4.
    fn quantize_beats(&self) -> f64 {
        let b = self.c.quantize_beats.get();
        if b > 0.0 {
            b.clamp(1.0 / 16.0, 4.0)
        } else {
            1.0
        }
    }

    /// The tempo at the play position, as playing.
    fn effective_bpm(&self) -> f64 {
        self.track
            .as_ref()
            .map_or(0.0, |t| t.bpm_at(self.seconds()) * self.speed())
    }

    /// Tempo, beat and play state, for plugins on this deck.
    fn transport(&self) -> opendj_plugin_host::Transport {
        let playing = self.c.play.get_bool();
        match self.track.as_ref() {
            Some(t) if t.bpm > 0.0 => opendj_plugin_host::Transport {
                bpm: self.effective_bpm(),
                beat: (self.seconds() - t.first_beat) / t.beat_length(),
                playing,
            },
            _ => opendj_plugin_host::Transport {
                bpm: 120.0,
                beat: 0.0,
                playing,
            },
        }
    }

    /// Current beat length in seconds, for tempo-synced effects.
    fn beat_seconds(&self) -> f64 {
        match self.track.as_ref() {
            Some(t) if t.bpm > 0.0 => 60.0 / self.effective_bpm().max(1.0),
            _ => 0.5,
        }
    }

    /// Jumps by `seconds`, taking an active loop along.
    fn jump(&mut self, seconds: f64) {
        if self.c.loop_enabled.get_bool() {
            self.loop_start += seconds;
            self.loop_end += seconds;
        }
        let now = self.seconds();
        self.set_seconds(now + seconds);
    }

    /// Takes the track off the deck; it goes back to the loader to be freed.
    /// The input played through the deck: 0 none, else 1 + the input.
    fn live_input(&self) -> usize {
        (self.c.live_input.get().round().max(0.0) as usize).min(NUM_INPUTS)
    }

    fn unload(&mut self) -> Option<Arc<Track>> {
        let track = self.track.take()?;
        self.audio = None;
        self.pos = 0.0;
        self.velocity = 0.0;
        self.jog_pending = 0.0;
        self.bend_rate = 0.0;
        self.slip = None;
        self.slip_moving = false;
        self.slip_cue = None;
        self.c.slip_position.set(-1.0);
        self.rolling = false;
        self.c.track_loaded.set_bool(false);
        self.c.duration.set(0.0);
        self.c.bpm.set(0.0);
        self.c.playposition.set(0.0);
        self.last_written_position = 0.0;
        self.c.cue_point.set(0.0);
        self.c.loop_enabled.set_bool(false);
        for h in &self.c.hotcues {
            h.enabled.set_bool(false);
            h.status.set(0.0);
            h.kind.set(0.0);
            h.end_position.set(-1.0);
        }
        self.c.hotcue_focus.set(-1.0);
        Some(track)
    }

    /// Handles button presses (cue, hotcues, loops, beat jumps) once per buffer.
    /// Pitch reset: `rate_set_zero` puts the tempo at 0% at once, and while
    /// `rate_reset` is on the tempo glides there over `dt` seconds' worth
    /// of RATE_RESET_SPEED. Works with or without a track loaded.
    fn reset_rate(&mut self, dt: f64) {
        if self.zero_edge.pressed(&self.c.rate_set_zero) {
            self.c.rate.set(0.0);
            self.c.rate_reset.set_bool(false);
        }
        if !self.c.rate_reset.get_bool() {
            return;
        }
        // A deck following sync takes its tempo from the leader.
        if self.sync_speed.is_some() {
            self.c.rate_reset.set_bool(false);
            return;
        }
        let rate = self.c.rate.get();
        let step = RATE_RESET_SPEED * dt / self.c.rate_range.get();
        if rate.abs() <= step {
            self.c.rate.set(0.0);
            self.c.rate_reset.set_bool(false);
        } else {
            self.c.rate.set(rate - step * rate.signum());
        }
    }

    /// Shows `locked` (the loaded track's grid lock) on `bpmlock`, without
    /// taking it for the DJ turning it.
    fn show_grid_lock(&mut self, locked: bool) {
        self.c.bpmlock.set_bool(locked);
        self.bpmlock_seen = self.c.bpmlock.version();
    }

    fn handle_controls(&mut self) {
        let Some(track) = self.track.clone() else {
            return;
        };
        // Seeks from the UI (e.g. clicking the overview) arrive as playposition changes.
        let p = self.c.playposition.get();
        if (p - self.last_written_position).abs() > 1e-9 {
            self.set_seconds(p * track.duration);
            // A seek while slipping takes the silent playhead along.
            if self.slip.is_some() {
                self.slip = Some(self.pos);
            }
        }
        // Where the track is before this buffer's jumps: a slip that starts
        // now runs on from here.
        let unslipped = self.pos;

        if self.c.bpmlock.version() != self.bpmlock_seen {
            self.bpmlock_seen = self.c.bpmlock.version();
            let on = self.c.bpmlock.get_bool();
            if on != track.info.grid_locked {
                self.grid_edit = Some((GridEdit::Lock(on), self.seconds()));
            }
        }

        let playing = self.c.play.get_bool();
        if self.cue_edge.pressed(&self.c.cue_default) {
            if playing {
                self.c.play.set_bool(false);
                let cue = self.c.cue_point.get();
                self.set_seconds(cue);
            } else {
                self.c.cue_point.set(self.seconds());
                if self.c.cue_mode.get() != 3.0 {
                    self.c.play.set_bool(true);
                    self.previewing = Some(self.c.play.version());
                }
            }
        }
        if let Some(version) = self.previewing {
            if self.c.play.version() != version {
                // Play pressed (or stopped) during the preview: leave it so.
                self.previewing = None;
            } else if !self.c.cue_default.get_bool() {
                self.previewing = None;
                self.c.play.set_bool(false);
                let cue = self.c.cue_point.get();
                self.set_seconds(cue);
            }
        }
        if self.cup_edge.pressed(&self.c.cue_play) {
            if playing {
                self.c.play.set_bool(false);
                let cue = self.c.cue_point.get();
                self.set_seconds(cue);
            } else {
                self.c.cue_point.set(self.seconds());
            }
            self.cup_held = true;
        }
        if self.cup_held && !self.c.cue_play.get_bool() {
            self.cup_held = false;
            self.c.play.set_bool(true);
        }
        for h in 0..NUM_HOTCUES {
            let mut edges = self.c.hotcues[h].edges;
            let hc = &self.c.hotcues[h];
            let activate = edges[0].pressed(&hc.activate);
            let goto_play = edges[1].pressed(&hc.goto_play);
            let goto_stop = edges[2].pressed(&hc.goto_stop);
            let clear = edges[3].pressed(&hc.clear);
            let (set, position) = (hc.enabled.get_bool(), hc.position.get());
            let saved_loop = (set && hc.kind.get() == HOTCUE_LOOP)
                .then(|| hc.end_position.get())
                .filter(|&end| end > position);
            self.c.hotcues[h].edges = edges;
            if activate || (set && (goto_play || goto_stop)) {
                self.c.hotcue_focus.set((h + 1) as f64);
            }
            if activate {
                match saved_loop {
                    // Pressing the loop that is playing turns it off, as in Mixxx.
                    Some(end)
                        if self.c.loop_enabled.get_bool()
                            && (self.loop_start - position).abs() < 1e-3
                            && (self.loop_end - end).abs() < 1e-3 =>
                    {
                        self.c.loop_enabled.set_bool(false)
                    }
                    Some(end) => {
                        self.loop_in_pending = false;
                        self.loop_start = position;
                        self.loop_end = end;
                        self.c.loop_enabled.set_bool(true);
                        self.set_seconds(position);
                    }
                    None if set => {
                        // In slip mode a playing deck plays the cue while
                        // it is held, then carries on as if it hadn't.
                        if playing && self.c.slip_enabled.get_bool() {
                            self.slip_cue = Some(h);
                        }
                        self.set_seconds(position)
                    }
                    None => {
                        let hc = &self.c.hotcues[h];
                        // An empty pad pressed while a loop plays saves the loop.
                        if self.c.loop_enabled.get_bool() && self.loop_end > self.loop_start {
                            hc.position.set(self.loop_start.max(0.0));
                            hc.end_position.set(self.loop_end);
                            hc.kind.set(HOTCUE_LOOP);
                        } else {
                            let at = self.seconds();
                            let snapped = if self.c.quantize.get_bool() && track.bpm > 0.0 {
                                track.snap_to(at, self.point_step(), false)
                            } else {
                                at
                            };
                            hc.position.set(snapped.max(0.0));
                            hc.end_position.set(-1.0);
                            hc.kind.set(HOTCUE_CUE);
                        }
                        hc.enabled.set_bool(true);
                    }
                }
            }
            if set && (goto_play || goto_stop) {
                self.set_seconds(position);
                self.c.play.set_bool(goto_play);
            }
            let hc = &self.c.hotcues[h];
            if clear {
                hc.enabled.set_bool(false);
                hc.kind.set(0.0);
                hc.end_position.set(-1.0);
            }
            if self.slip_cue == Some(h) && !hc.activate.get_bool() {
                self.slip_cue = None;
            }
            hc.status
                .set(match (hc.enabled.get_bool(), hc.activate.get_bool()) {
                    (false, _) => 0.0,
                    (true, false) => 1.0,
                    (true, true) => 2.0,
                });
        }

        let quantize = self.c.quantize.get_bool();
        let step = self.point_step();
        let snap = |t: f64, floor: bool| {
            if !quantize || track.bpm <= 0.0 {
                return t;
            }
            track.snap_to(t, step, floor)
        };
        // Seconds from `t` to `beats` beats on, along the grid.
        let beats_from = |t: f64, beats: f64| track.beats_from(t, beats) - t;
        // A fixed buffer keeps this allocation-free; nobody presses 32 buttons
        // within one buffer.
        let mut actions = [None; 32];
        let mut n = 0;
        for p in self.c.pushes.iter_mut() {
            if p.edge.pressed(&p.control) && n < actions.len() {
                actions[n] = Some(p.action);
                n += 1;
            }
        }
        // beatjump and loop_scale are set to a value, which is used once.
        let jump = self.c.beatjump.get();
        if jump != 0.0 {
            self.c.beatjump.set(0.0);
            let by = beats_from(self.seconds(), jump);
            self.jump(by);
        }
        // A playing loop takes the position with it, as a beat jump does.
        let beats = take_value(&self.c.loop_move[0]);
        let loops = take_value(&self.c.loop_move[1]);
        if (beats != 0.0 || loops != 0.0) && self.loop_end > self.loop_start {
            let by = beats_from(self.loop_start, beats) + (self.loop_end - self.loop_start) * loops;
            if self.c.loop_enabled.get_bool() {
                self.jump(by);
            } else {
                self.loop_start += by;
                self.loop_end += by;
            }
        }
        // As in Mixxx, loop_scale moves the loop's end and leaves beatloop_size.
        let scale = self.c.loop_scale.get();
        if scale != 0.0 {
            self.c.loop_scale.set(0.0);
            if self.loop_end > self.loop_start {
                self.loop_end = self.loop_start + (self.loop_end - self.loop_start) * scale;
            }
        }
        let by = take_value(&self.c.grid_move);
        if by != 0.0 {
            self.grid_edit = Some((GridEdit::Drag(by), self.seconds()));
        }
        for action in actions.into_iter().flatten() {
            let enabled = self.c.loop_enabled.get_bool();
            let size = self.c.beatloop_size.get();
            let now = self.seconds();
            match action {
                Action::BeatLoop(n) if enabled && (size - n).abs() < 1e-9 => {
                    self.c.loop_enabled.set_bool(false)
                }
                Action::BeatLoop(n) => {
                    if !enabled {
                        self.loop_start = snap(now, true);
                    }
                    self.c.beatloop_size.set(n);
                    self.loop_end = self.loop_start + beats_from(self.loop_start, n);
                    self.c.loop_enabled.set_bool(true);
                }
                Action::BeatLoopToggle if enabled => self.c.loop_enabled.set_bool(false),
                Action::BeatLoopToggle => {
                    self.loop_start = snap(now, true);
                    self.loop_end = self.loop_start + beats_from(self.loop_start, size);
                    self.c.loop_enabled.set_bool(true);
                }
                Action::LoopHalve | Action::LoopDouble => {
                    let n = if matches!(action, Action::LoopHalve) {
                        size / 2.0
                    } else {
                        size * 2.0
                    };
                    self.c.beatloop_size.set(n);
                    if enabled {
                        self.loop_end = self.loop_start
                            + beats_from(self.loop_start, self.c.beatloop_size.get());
                    }
                }
                Action::LoopIn => {
                    self.c.loop_enabled.set_bool(false);
                    self.loop_start = snap(now, false);
                    self.loop_end = self.loop_start;
                    self.loop_in_pending = true;
                }
                // beatloop_size stays as it was (a power of two), so the
                // length box goes back to it once this loop ends.
                Action::LoopOut => {
                    let end = snap(now, false);
                    if end > self.loop_start + 0.01 {
                        self.loop_end = end;
                        self.c.loop_enabled.set_bool(true);
                    }
                }
                Action::Reloop if enabled => self.c.loop_enabled.set_bool(false),
                Action::Reloop => {
                    if self.loop_end > self.loop_start {
                        self.c.loop_enabled.set_bool(true);
                        if now > self.loop_end || now < self.loop_start {
                            let start = self.loop_start;
                            self.set_seconds(start);
                        }
                    }
                }
                Action::ReloopAndStop => {
                    if self.loop_end > self.loop_start {
                        self.c.loop_enabled.set_bool(true);
                        let start = self.loop_start;
                        self.set_seconds(start);
                    }
                    self.c.play.set_bool(false);
                }
                Action::BeatJump(n) => self.jump(beats_from(now, n)),
                Action::BeatJumpSized(dir) => {
                    self.jump(beats_from(now, dir * self.c.beatjump_size.get()))
                }
                Action::RateStep(step) => {
                    let range = self.c.rate_range.get();
                    let step = step * self.rate_dir();
                    self.c.rate.set(self.c.rate.get() + step / range);
                }
                Action::Start(play) => {
                    self.set_seconds(0.0);
                    if let Some(play) = play {
                        self.c.play.set_bool(play);
                    }
                }
                Action::End => self.set_seconds(track.duration),
                Action::CueGoto(play) => {
                    let cue = self.c.cue_point.get();
                    self.set_seconds(cue);
                    self.c.play.set_bool(play);
                }
                Action::Grid(edit) => self.grid_edit = Some((edit, now)),
                // Taps come at the playing speed; the grid is at the track's.
                Action::BpmTap => {
                    if let Some(bpm) = self.taps.tap(self.clock) {
                        let speed = self.speed().abs();
                        if speed > 0.0 {
                            let bpm = (bpm / speed).clamp(40.0, 300.0);
                            self.grid_edit = Some((GridEdit::SetTempo(bpm), now));
                        }
                    }
                }
                Action::KeyStep(n) => {
                    let p = (self.c.pitch.get() + n).clamp(-6.0, 6.0);
                    // Whole steps land on whole semitones.
                    self.c.pitch.set(if n.abs() >= 1.0 { p.round() } else { p });
                }
                Action::KeyReset => self.c.pitch.set(0.0),
                // The instrument's pads read these themselves.
                Action::Keyboard(_) if self.c.instrument_pads.get_bool() => {}
                Action::Keyboard(semitones) => {
                    self.c.pitch.set(semitones);
                    let set = |h: usize| self.c.hotcues[h].enabled.get_bool();
                    let focus = self.c.hotcue_focus.get() as i64 - 1;
                    let cue = (0..NUM_HOTCUES as i64)
                        .filter(|&h| h == focus)
                        .chain(0..NUM_HOTCUES as i64)
                        .map(|h| h as usize)
                        .find(|&h| set(h));
                    let at = match cue {
                        Some(h) => {
                            self.c.hotcue_focus.set((h + 1) as f64);
                            self.c.hotcues[h].position.get()
                        }
                        None => self.c.cue_point.get(),
                    };
                    self.set_seconds(at);
                    self.c.play.set_bool(true);
                }
            }
        }

        // Loop rolls: a loop while the pad is held, then back to where the
        // track would have been (slip).
        let mut roll = None;
        for (size, c, edge) in self.c.rolls.iter_mut() {
            if edge.pressed(c) {
                roll = Some(*size);
            }
        }
        let held = self.c.rolls.iter().any(|(_, c, _)| c.get_bool());
        if let Some(size) = roll {
            let n = if size > 0.0 {
                size
            } else {
                self.c.beatloop_size.get()
            };
            self.loop_start = snap(self.seconds(), true);
            self.loop_end = self.loop_start + beats_from(self.loop_start, n);
            self.c.loop_enabled.set_bool(true);
            self.rolling = true;
        } else if self.rolling && !held {
            self.rolling = false;
            self.c.loop_enabled.set_bool(false);
        }
        let slipping = self.c.reverseroll.get_bool() || self.rolling || self.slip_trick(unslipped);
        match (self.slip, slipping) {
            (None, true) => {
                self.slip = Some(unslipped);
                self.slip_moving = self.was_playing || self.c.play.get_bool();
            }
            (Some(at), false) => {
                self.slip = None;
                self.slip_moving = false;
                self.slip_cue = None;
                let s = at / self.source_rate();
                // Back outside a loop that is still on (slip switched off
                // mid-loop): the loop ends rather than pulling it back.
                if self.c.loop_enabled.get_bool() && (s < self.loop_start || s >= self.loop_end) {
                    self.c.loop_enabled.set_bool(false);
                }
                self.set_seconds(s);
            }
            (Some(_), true) => self.slip_moving |= self.c.play.get_bool(),
            _ => {}
        }

        let jog = self.c.jog.get();
        if jog != 0.0 {
            self.c.jog.set(0.0);
            self.jog_pending += jog * JOG_SECONDS;
        }

        let enabled = self.c.loop_enabled.get_bool();
        // Any loop starting ends marking one out.
        self.loop_in_pending &= !enabled;
        self.c.loop_in_pending.set_bool(self.loop_in_pending);
        let size = self.c.beatloop_size.get();
        for (n, light) in &self.c.beatloop_lights {
            light.set_bool(enabled && (size - n).abs() < 1e-9);
        }
        self.c.loop_start.set(self.loop_start);
        self.c.loop_end.set(self.loop_end);
        self.c.bpm.set(track.bpm_at(self.seconds()) * self.speed());
        self.file_key = Key::parse(&track.key);
        let key = self
            .file_key
            .map(|k| k.playing(self.c.pitch.get(), self.speed(), self.c.keylock.get_bool()));
        self.c
            .file_key
            .set(self.file_key.map_or(0.0, Key::mixxx_number));
        self.c.key.set(key.map_or(0.0, Key::mixxx_number));
    }

    /// In slip mode, whether a trick is taking the track away from where it
    /// would be: scratching, reverse, a loop, a hot cue held, or a pause.
    /// Only a deck that is (or was just) playing slips: a paused deck's
    /// scratching and cue finding stay where they are. `unslipped` is the
    /// position before this buffer's jumps, which tells a pause from a
    /// stop at the cue point.
    fn slip_trick(&self, unslipped: f64) -> bool {
        if !self.c.slip_enabled.get_bool() {
            return false;
        }
        let playing = self.c.play.get_bool();
        let running = self.slip.is_some() && self.slip_moving;
        if !playing && !running && !self.was_playing {
            return false;
        }
        // Not the stop at the end of the track.
        let in_track = self
            .audio
            .as_ref()
            .is_some_and(|a| self.pos < a.len() as f64 - 1.0);
        let paused = !playing
            && (running || (self.was_playing && in_track && (self.pos - unslipped).abs() < 1.0));
        self.c.scratch2_enable.get_bool()
            || self.c.scrub_enable.get_bool()
            || self.c.reverse.get_bool()
            || self.c.loop_enabled.get_bool()
            || self.slip_cue.is_some()
            || paused
    }

    /// The speed that closes most of the gap to the waveform drag's position
    /// over the next [`SCRUB_GLIDE`] seconds, so the deck follows the
    /// pointer smoothly instead of jumping. A gap too wide to cover at
    /// [`SCRUB_MAX_SPEED`] in a few glides is jumped over.
    fn scrub_velocity(&mut self, dt: f64) -> f64 {
        let Some(track) = &self.track else { return 0.0 };
        let target = self.c.scrub_position.get().clamp(0.0, 1.0) * track.duration;
        let gap = target - self.seconds();
        if gap.abs() > SCRUB_MAX_SPEED * SCRUB_GLIDE * 4.0 {
            // Land just short of the target and glide the rest.
            self.set_seconds(target - gap.signum() * SCRUB_MAX_SPEED * SCRUB_GLIDE);
            return self.scrub_velocity(dt);
        }
        (gap / SCRUB_GLIDE.max(dt)).clamp(-SCRUB_MAX_SPEED, SCRUB_MAX_SPEED)
    }

    /// The speed to reach by the end of this buffer of `dt` seconds.
    fn target_velocity(&mut self, dt: f64) -> f64 {
        self.search_quiet = (self.search_quiet - dt).max(0.0);
        let bend = self.bend_step(dt);
        if self.audio.is_none() {
            self.jog_pending = 0.0;
            return 0.0;
        }
        if self.c.seek_hold.get_bool() {
            self.jog_pending = 0.0;
            return 0.0;
        }
        if self.c.scrub_enable.get_bool() {
            self.jog_pending = 0.0;
            return self.scrub_velocity(dt);
        }
        if self.c.scratch2_enable.get_bool() {
            // Jog still counts while the wheel is touched: mappings such as
            // the DDJ-FLX4's send SHIFT + turn as fast `jog` steps.
            return self.c.scratch2.get() + self.jog_step(dt);
        }
        let search = self.c.rate_search.get();
        if search != 0.0 {
            return search;
        }
        let mut v = if self.c.play.get_bool() {
            // Turning the rim bends the speed: faster clockwise, slower
            // counterclockwise, but never backwards.
            (self.forward_speed() + bend).max(0.0)
        } else {
            0.0
        };
        if self.c.reverse.get_bool() || self.c.reverseroll.get_bool() {
            v = -v;
        }
        // A jog movement bends the speed while playing and scrubs while stopped.
        v + self.jog_step(dt)
    }

    /// Takes the rim ticks (`jog_bend`) that came in over the last `dt`
    /// seconds and returns the bend they make, in multiples of normal speed
    /// to add to the playing speed. A paused deck is nudged instead, like a jog.
    fn bend_step(&mut self, dt: f64) -> f64 {
        let ticks = self.c.jog_bend.get();
        if ticks != 0.0 {
            self.c.jog_bend.set(0.0);
        }
        if !self.c.play.get_bool() || dt <= 0.0 {
            self.jog_pending += ticks * BEND_PAUSED_SECONDS;
            self.bend_rate = 0.0;
            return 0.0;
        }
        self.bend_rate += (ticks / dt - self.bend_rate) * (dt / BEND_SMOOTH).min(1.0);
        if ticks == 0.0 && (self.bend_rate * BEND_PER_TICK_PER_SECOND).abs() < 1e-4 {
            self.bend_rate = 0.0;
        }
        self.bend_rate * BEND_PER_TICK_PER_SECOND
    }

    /// The speed that moves on a share of the pending jog this buffer.
    fn jog_step(&mut self, dt: f64) -> f64 {
        let moved = self.jog_pending * (dt / JOG_SPREAD).min(1.0);
        self.jog_pending -= moved;
        if self.jog_pending.abs() < 1e-7 {
            self.jog_pending = 0.0;
        }
        let speed = moved / dt;
        if speed.abs() > JOG_SEARCH_SPEED {
            self.search_quiet = JOG_SEARCH_HOLD;
        }
        speed
    }
}

/// What a deck plays from: a track's audio, or the mix of its stems.
pub(crate) trait Frames {
    /// Frame `i`, or silence where there is none.
    fn frame(&self, i: usize) -> [f32; 2];
    /// Frame `k`, which may be before the start.
    fn frame_at(&self, k: isize) -> [f32; 2] {
        if k >= 0 {
            self.frame(k as usize)
        } else {
            [0.0; 2]
        }
    }
    /// The next frames read run on from `start` (see [`StemMix`]).
    fn begin(&self, _start: f64) {}
}

/// The audio played gaplessly before and after a deck's own.
type Joins = (Option<Arc<Audio>>, Option<Arc<Audio>>);

fn joined<'a>(audio: &'a Audio, joins: &'a Joins) -> Joined<'a> {
    Joined {
        audio,
        before: joins.0.as_deref(),
        after: joins.1.as_deref(),
    }
}

/// A track's audio joined to the tracks played gaplessly before and after
/// it, so reads across either end hear the other track instead of silence.
pub(crate) struct Joined<'a> {
    audio: &'a Audio,
    before: Option<&'a Audio>,
    after: Option<&'a Audio>,
}

impl Joined<'_> {
    /// Reads `other` at `k` frames of `self.audio` from its start.
    fn other(&self, other: &Audio, k: f64) -> [f32; 2] {
        let k = k * other.sample_rate as f64 / self.audio.sample_rate.max(1) as f64;
        if other.sample_rate == self.audio.sample_rate {
            Frames::frame_at(other, k.round() as isize)
        } else {
            read(other, k)
        }
    }
}

impl Frames for Joined<'_> {
    #[inline]
    fn frame(&self, i: usize) -> [f32; 2] {
        self.frame_at(i as isize)
    }

    fn frame_at(&self, k: isize) -> [f32; 2] {
        let len = self.audio.len() as isize;
        match (self.before, self.after) {
            (Some(b), _) if k < 0 => {
                let at = b.len() as f64 * self.audio.sample_rate as f64
                    / b.sample_rate.max(1) as f64
                    + k as f64;
                self.other(b, at)
            }
            (_, Some(a)) if k >= len => self.other(a, (k - len) as f64),
            _ if k >= 0 => self.audio.frame(k as usize),
            _ => [0.0; 2],
        }
    }
}

impl Frames for Audio {
    #[inline]
    fn frame(&self, i: usize) -> [f32; 2] {
        Audio::frame(self, i)
    }
}

/// A deck's stems mixed at their gains. The gains glide from `from` to `to`
/// over the `span` frames read after [`Frames::begin`], so a mute or a fader
/// move doesn't click, whether the frames go straight out or through the
/// time-stretcher (which reads ahead of the play position).
pub(crate) struct StemMix<'a> {
    stems: &'a Stems,
    /// The high-quality stems taking over from `stems`, once complete, and
    /// how far the crossfade to them has got at the start and end of the
    /// span (see [`Stems::mix_blend`]).
    up: Option<&'a Stems>,
    up_from: f32,
    up_to: f32,
    /// The track itself, which separated stems play under (see [`Stems::mix`]).
    audio: &'a Audio,
    from: [f32; MAX_STEMS],
    to: [f32; MAX_STEMS],
    /// Separated stems all at full gain play the track itself, so no stem
    /// frames are read at all.
    unity: bool,
    /// Only the stems, each at its gain, without the original: a deck's
    /// stem effect send (see [`StemFx`]).
    solo: bool,
    span: f64,
    start: std::cell::Cell<f64>,
}

impl<'a> StemMix<'a> {
    /// The mix with the crossfade to the high-quality stems going from
    /// `up.0` to `up.1` over the span.
    fn blending(
        stems: &'a Stems,
        audio: &'a Audio,
        from: [f32; MAX_STEMS],
        to: [f32; MAX_STEMS],
        up: (f32, f32),
        span: f64,
    ) -> Self {
        let upgrade = stems.ready_upgrade();
        let separated = stems.is_separated() && upgrade.is_none_or(Stems::is_separated);
        Self {
            stems,
            up: upgrade,
            up_from: up.0,
            up_to: up.1,
            audio,
            from,
            to,
            unity: separated && from.iter().chain(&to).all(|&g| g == 1.0),
            solo: false,
            span: span.max(1.0),
            start: std::cell::Cell::new(0.0),
        }
    }

    /// Only the stems with a weight, at that weight (gliding from `from`
    /// to `to` over the span), without the original under them.
    fn solo(
        stems: &'a Stems,
        audio: &'a Audio,
        from: [f32; MAX_STEMS],
        to: [f32; MAX_STEMS],
        up: (f32, f32),
        span: f64,
    ) -> Self {
        Self {
            unity: false,
            solo: true,
            ..Self::blending(stems, audio, from, to, up, span)
        }
    }
}

impl Frames for StemMix<'_> {
    #[inline]
    fn frame(&self, i: usize) -> [f32; 2] {
        if self.unity {
            return self.audio.frame(i);
        }
        let t = ((i as f64 - self.start.get()).abs() / self.span).min(1.0) as f32;
        let gains = std::array::from_fn(|k| self.from[k] + (self.to[k] - self.from[k]) * t);
        let w = self.up_from + (self.up_to - self.up_from) * t;
        if self.solo {
            return self.stems.solo_blend(self.up, w, i, &gains);
        }
        self.stems
            .mix_blend(self.up, w, i, &gains, || self.audio.frame(i))
    }

    fn begin(&self, start: f64) {
        self.start.set(start);
    }
}

/// How long a jump in the play position fades over: long enough to hide the
/// step in the waveform, short enough not to be heard as a fade.
pub(crate) const DECLICK_SECONDS: f64 = 0.006;

/// How long the stems made on load crossfade into high-quality ones once
/// those are ready, so a muted stem doesn't click at the switch.
pub(crate) const STEM_UPGRADE_SECONDS: f64 = 0.02;

/// A short crossfade as a deck goes into the time-stretcher (from playing
/// as a record) or out of it, over this many output frames. The two differ
/// in pitch and phase once the speed isn't 1, so a hard switch clicks.
#[derive(Clone, Copy)]
enum ShiftFade {
    In(usize),
    Out(usize),
}

/// A short crossfade over a jump in a deck's play position (a beatjump,
/// hotcue, seek, or a loop wrapping), as DJ software does to keep jumps from
/// clicking: the audio from before the jump plays on for a few milliseconds,
/// fading out while the audio from after it fades in.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Declick {
    /// Where the audio from before the jump is read, in source frames.
    pub(crate) pos: f64,
    /// Output frames still to fade, of `len`.
    pub(crate) left: u32,
    len: u32,
}

impl Declick {
    pub(crate) fn new(pos: f64, sample_rate: f64) -> Self {
        Self::frames(pos, (DECLICK_SECONDS * sample_rate).round().max(1.0) as u32)
    }

    pub(crate) fn frames(pos: f64, len: u32) -> Self {
        Self {
            pos,
            left: len,
            len,
        }
    }

    /// Mixes the next frame: `new` from after the jump, `old` from before it.
    #[inline]
    pub(crate) fn mix(&mut self, new: [f32; 2], old: [f32; 2]) -> [f32; 2] {
        self.left = self.left.saturating_sub(1);
        crossfade(old, new, 1.0 - self.left as f32 / self.len as f32)
    }
}

/// The test tone at frame `n`: a 440 Hz beep, a quarter of full scale,
/// 0.4 s on and 0.2 s off, faded in and out so it doesn't click.
/// The metronome's next sample for deck `d`: a click as the play position
/// passes each beat of its grid going forwards, higher on bar 1.
fn metronome(d: &mut Deck, source_rate: f64, moving: bool, rate: f64) -> f32 {
    const LENGTH: f64 = 0.05;
    let Some(track) = &d.track else {
        return 0.0;
    };
    let at = track.grid.beat_at(d.pos / source_rate);
    if !moving {
        // Starting right on a beat (from a cue, say) clicks it.
        d.metronome_beat = (at - 0.001).floor();
        return 0.0;
    }
    let beat = at.floor();
    // Only a step to the next beat clicks: not a jump, a loop or reverse.
    if beat == d.metronome_beat + 1.0 {
        let bar_one = beat.rem_euclid(4.0) == 0.0;
        d.click = Some((0, if bar_one { 1_760.0 } else { 1_320.0 }));
    }
    d.metronome_beat = beat;
    let Some((n, hz)) = d.click else {
        return 0.0;
    };
    let t = n as f64 / rate;
    if t >= LENGTH {
        d.click = None;
        return 0.0;
    }
    d.click = Some((n + 1, hz));
    let env = (t / 0.001).min(1.0) * (-t * 80.0).exp();
    (0.4 * env * (std::f64::consts::TAU * hz as f64 * t).sin()) as f32
}

fn test_tone(n: u64, rate: f64) -> f32 {
    let t = n as f64 / rate;
    let at = t % 0.6;
    if at >= 0.4 {
        return 0.0;
    }
    let fade = (at / 0.01).min((0.4 - at) / 0.01).min(1.0);
    (0.25 * fade * (std::f64::consts::TAU * 440.0 * t).sin()) as f32
}

/// A raised-cosine crossfade, all `old` at `t` = 0 and all `new` at 1.
#[inline]
pub(crate) fn crossfade(old: [f32; 2], new: [f32; 2], t: f32) -> [f32; 2] {
    let w = 0.5 - 0.5 * (std::f32::consts::PI * t).cos();
    [
        old[0] + (new[0] - old[0]) * w,
        old[1] + (new[1] - old[1]) * w,
    ]
}

/// Reads a stereo frame at a fractional position with 4-point Hermite
/// interpolation. Works the same in either direction; outside the track it
/// reads silence.
#[inline]
pub(crate) fn read<F: Frames + ?Sized>(audio: &F, pos: f64) -> [f32; 2] {
    let i = pos.floor() as isize;
    let t = (pos - i as f64) as f32;
    let at = |k: isize| audio.frame_at(k);
    let (xm1, x0, x1, x2) = (at(i - 1), at(i), at(i + 1), at(i + 2));
    let mut y = [0.0; 2];
    for c in 0..2 {
        let c0 = x0[c];
        let c1 = 0.5 * (x1[c] - xm1[c]);
        let c2 = xm1[c] - 2.5 * x0[c] + 2.0 * x1[c] - 0.5 * x2[c];
        let c3 = 0.5 * (x2[c] - xm1[c]) + 1.5 * (x0[c] - x1[c]);
        y[c] = ((c3 * t + c2) * t + c1) * t + c0;
    }
    y
}

/// The master limiter's ceiling: -0.5 dBFS, a little room for the peaks
/// between samples that a DAC or an MP3 encoder can make.
pub(crate) const LIMIT_CEILING: f32 = 0.944;

/// How fast a playing deck's auto gain moves to a new value, in dB per second.
const AUTO_GAIN_GLIDE_DB: f32 = 6.0;

/// Moves gain `from` towards `to` by at most `max_db`.
fn glide_db(from: f32, to: f32, max_db: f32) -> f32 {
    let (a, b) = (from.max(1e-6).log10() * 20.0, to.max(1e-6).log10() * 20.0);
    10f32.powf((a + (b - a).clamp(-max_db, max_db)) / 20.0)
}

/// Peak level to a meter position: -40 dBFS..0 dBFS maps to 0..1.
/// Seconds a waveform drag takes to catch up with the pointer.
const SCRUB_GLIDE: f64 = 0.06;
/// Fastest a waveform drag plays through a track, in multiples of normal speed.
const SCRUB_MAX_SPEED: f64 = 60.0;

/// Waveform scrubbing plays the track many times faster, which pushes it
/// up into a shrill whine. Fast passes are low-passed and turned down, more
/// the faster they go, so they sound like a muffled rush instead; at normal
/// speed the audio is untouched.
fn soften_scrub(s: [f32; 2], speed: f64, sr: f64, state: &mut [f32; 2]) -> [f32; 2] {
    let speed = speed.max(1.0);
    let cutoff = (SCRUB_TONE_HZ / speed).max(120.0);
    let a = (1.0 - (-std::f64::consts::TAU * cutoff / sr).exp()) as f32;
    let gain = (2.0 / speed).clamp(0.3, 1.0) as f32;
    let mut out = [0.0; 2];
    for c in 0..2 {
        state[c] += a * (s[c] - state[c]);
        out[c] = state[c] * gain;
    }
    out
}

/// The low-pass cutoff of waveform scrubbing at normal speed; it falls in
/// proportion as the speed rises.
const SCRUB_TONE_HZ: f64 = 12_000.0;

pub(crate) fn meter(peak: f32) -> f32 {
    if peak <= 1e-4 {
        0.0
    } else {
        ((20.0 * peak.log10() + 40.0) / 40.0).clamp(0.0, 1.0)
    }
}

/// How fast meters fall, in meter units per second.
const VU_FALL: f32 = 1.5;

/// How fast the master meter falls: 0.5 units, 20 dB, a second, close to a
/// DIN peak programme meter, so its readings can be read.
const MASTER_VU_FALL: f32 = 0.5;

/// The master limiter's ceiling while recording or on air, by default:
/// -1 dBTP, as streaming services and EBU R128 ask.
pub const DEFAULT_AIR_CEILING_DB: f64 = -1.0;

/// Left and right gains for `[Master],balance` (-1 = left only, 1 = right only).
fn balance_gains(b: f32) -> [f32; 2] {
    [(1.0 - b).min(1.0), (1.0 + b).min(1.0)]
}

pub struct Engine {
    sample_rate: f64,
    decks: [Deck; NUM_DECKS],
    master: MasterControls,
    fx: EffectRack,
    /// Per-buffer scratch for the mix's stages, [`BLOCK`] frames each, made
    /// up front: each deck's audio and metronome click, the main mix and the
    /// headphone cue before the main mix's effects.
    deck_buf: Vec<Vec<[f32; 2]>>,
    click_buf: Vec<Vec<f32>>,
    main_buf: Vec<[f32; 2]>,
    cue_buf: Vec<[f32; 2]>,
    samplers: Samplers,
    /// Each deck's Groove Box (playing only while `[GrooveN],enabled`),
    /// its sound for this buffer, and the gain each deck is ducked by.
    grooves: Vec<GrooveBox>,
    groove_buf: Vec<Vec<[f32; 2]>>,
    duck_buf: Vec<Vec<f32>>,
    duck_env: [f32; NUM_DECKS],
    /// Each deck's `live_input` and Groove Box switch as last seen, to tell
    /// which one the DJ picked last.
    live_seen: [usize; NUM_DECKS],
    groove_seen: [bool; NUM_DECKS],
    commands: rtrb::Consumer<Command>,
    returned: rtrb::Producer<Returned>,
    master_vu: [f32; 2],
    master_peak_hold: [f32; 2],
    master_gain: f32,
    balance: f32,
    xfade: [f32; NUM_DECKS],
    /// Seconds played, for blinking indicators.
    clock: f64,
    limiter: Limiter,
    visual: Arc<VisualPositions>,
    link: Link,
    /// The master deck: the one Link and MIDI clock follow.
    link_master: Option<usize>,
    /// The master deck's beat and tempo as heard, for MIDI clock out.
    beat_clock: Arc<crate::midi_clock::BeatClock>,
    /// The deck synced decks follow.
    sync_leader: Option<usize>,
    /// The leader picked by hand (its MASTER badge clicked), kept while it
    /// has a track with a tempo.
    sync_pinned: Option<usize>,
    /// Copies the master out while recording.
    record: RecordTap,
    record_feed: Option<Feed>,
    /// Copies the master out for the loudness meter, all the time.
    meter: RecordTap,
    meter_feed: Option<Feed>,
    /// Copies the master out for broadcasting, while on air.
    broadcast: RecordTap,
    broadcast_feed: Option<Feed>,
    /// Plays a track into the headphones for the info panel.
    preview: Preview,
    preview_input: Option<PreviewInput>,
    sampler_input: Option<SamplerInput>,
    /// The microphone and auxiliary inputs, and the ducking of the music
    /// under the microphone.
    inputs: [Input; NUM_INPUTS],
    ducker: Ducker,
    /// Frames into the test tone's beeps (see [`test_tone`]).
    tone_frame: u64,
}

/// How far (in beats) Link's beat may drift from the master deck's before
/// it is pulled back.
const LINK_PHASE_TOLERANCE: f64 = 0.01;

/// The most a synced deck bends its speed, as a fraction, to catch up with
/// its leader's beat.
const SYNC_MAX_BEND: f64 = 0.03;

/// Half, the same or double the leader's tempo, whichever is closest to
/// `own`, as other DJ software does: 153 BPM syncs to 160 against 80.
/// How a deck got where it is this buffer, for [`Engine::land_in_phase`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum Move {
    None,
    /// Started from where it was paused.
    Start,
    /// Jumped: a hot cue, CUE, beat jump or a click on the overview.
    Jump,
}

fn sync_ratio(leader: f64, own: f64) -> f64 {
    [1.0, 2.0, 0.5]
        .into_iter()
        .min_by(|a, b| {
            (leader * a / own)
                .ln()
                .abs()
                .total_cmp(&(leader * b / own).ln().abs())
        })
        .unwrap_or(1.0)
}

impl Engine {
    /// Each deck's Color FX, picked from the menu beside its filter knob:
    /// `loaded_chain_preset` set, or stepped with next/prev_chain_preset.
    /// Stepping skips 0 (none).
    fn update_color_fx(&mut self) {
        let n = COLOR_FX.len() as f64;
        for d in &mut self.decks {
            let mut chosen = d.c.cfx_preset.get().round();
            if d.c.cfx_edges[0].pressed(&d.c.cfx_next) {
                chosen = if chosen >= n { 1.0 } else { chosen + 1.0 };
            }
            if d.c.cfx_edges[1].pressed(&d.c.cfx_prev) {
                chosen = if chosen <= 1.0 { n } else { chosen - 1.0 };
            }
            let chosen = chosen.clamp(0.0, n);
            if d.c.cfx_preset.get() != chosen {
                d.c.cfx_preset.set(chosen);
            }
        }
    }

    /// Creates all engine controls in `registry`.
    pub fn new(registry: &ControlRegistry, sample_rate: f64) -> (Self, EngineInput) {
        let master = create_global_controls(registry);
        // The rate the engine runs at, as Mixxx's [App],samplerate.
        registry
            .indicator("[App]", "samplerate", 384_000.0)
            .set(sample_rate);
        let decks = std::array::from_fn(|d| Deck {
            c: DeckControls::create(registry, d),
            track: None,
            audio: None,
            pos: 0.0,
            last_written_position: 0.0,
            loop_start: 0.0,
            loop_end: 0.0,
            loop_in_pending: false,
            grid_edit: None,
            clock: 0.0,
            taps: TapTempo::default(),
            bpmlock_seen: 0,
            metronome_beat: f64::NAN,
            click: None,
            velocity: 0.0,
            jog_pending: 0.0,
            search_quiet: 0.0,
            bend_rate: 0.0,
            slip: None,
            slip_moving: false,
            slip_cue: None,
            rolling: false,
            was_playing: false,
            cue_edge: Edge::default(),
            cup_edge: Edge::default(),
            zero_edge: Edge::default(),
            cup_held: false,
            previewing: None,
            eject_edge: Edge::default(),
            sync_edge: Edge::default(),
            was_synced: false,
            following: false,
            sync_speed: None,
            sync_bend: 0.0,
            sync_ratio: 1.0,
            sync_pair: (0, 0),
            phase_offset: 0.0,
            rate_written: 0.0,
            eq: Isolator::new(sample_rate),
            filter: ColorFx::new(sample_rate),
            sculpt: Sculpt::new(sample_rate),
            eq_gains: [1.0; 3],
            pregain: 1.0,
            track_gain: false,
            auto_gain: 1.0,
            volume: 1.0,
            quick: 1.0,
            vu: [0.0; 2],
            peak_hold: 0.0,
            file_key: None,
            shifter: KeyShifter::new(sample_rate),
            shifted: vec![[0.0; 2]; 4096],
            scrub_tone: [0.0; 2],
            expected_pos: None,
            declick: None,
            stem_gains: [1.0; MAX_STEMS],
            stem_upgrade: 0.0,
            stems_shown: 0,
            stem_fx: StemFx::new(sample_rate),
            stem_send: [0.0; MAX_STEMS],
            stem_fed: [0.0; MAX_STEMS],
            stem_muted: [false; MAX_STEMS],
            stem_shifter: KeyShifter::new(sample_rate),
            stem_shifted: vec![[0.0; 2]; 4096],
            hand_over: None,
            join_after: None,
            join_before: None,
        });
        let fx = EffectRack::new(registry, sample_rate);
        let (samplers, sampler_input) = Samplers::new(registry, sample_rate);
        let kits: Arc<[crate::kits::Kit]> = crate::kits::build(sample_rate).into();
        let grooves = (0..NUM_DECKS)
            .map(|d| GrooveBox::new(registry, d, kits.clone(), sample_rate))
            .collect();
        let (preview, preview_input) = Preview::new(registry, sample_rate);
        let (commands_tx, commands) = rtrb::RingBuffer::new(QUEUE);
        let (returned, returned_rx) = rtrb::RingBuffer::new(QUEUE * 2);
        let (record, record_feed) = recorder::tap(sample_rate);
        let (meter, meter_feed) = recorder::tap(sample_rate);
        meter_feed.set_active(true);
        let (broadcast, broadcast_feed) = recorder::tap(sample_rate);
        let inputs = std::array::from_fn(|i| Input::new(registry, i, sample_rate));
        let ducker = Ducker::new(registry, sample_rate);
        crate::midi_clock::create_controls(registry);
        (
            Self {
                inputs,
                ducker,
                visual: Arc::default(),
                link: Link::new(registry),
                link_master: None,
                beat_clock: Arc::default(),
                sync_leader: None,
                sync_pinned: None,
                record,
                record_feed: Some(record_feed),
                meter,
                meter_feed: Some(meter_feed),
                broadcast,
                broadcast_feed: Some(broadcast_feed),
                preview,
                preview_input: Some(preview_input),
                sampler_input: Some(sampler_input),
                sample_rate,
                decks,
                master,
                fx,
                deck_buf: vec![vec![[0.0; 2]; BLOCK]; NUM_DECKS],
                click_buf: vec![vec![0.0; BLOCK]; NUM_DECKS],
                main_buf: vec![[0.0; 2]; BLOCK],
                cue_buf: vec![[0.0; 2]; BLOCK],
                samplers,
                grooves,
                groove_buf: vec![vec![[0.0; 2]; BLOCK]; NUM_DECKS],
                duck_buf: vec![vec![1.0; BLOCK]; NUM_DECKS],
                duck_env: [0.0; NUM_DECKS],
                live_seen: [0; NUM_DECKS],
                groove_seen: [false; NUM_DECKS],
                commands,
                returned,
                master_vu: [0.0; 2],
                master_peak_hold: [0.0; 2],
                master_gain: 1.0,
                balance: 0.0,
                xfade: [1.0; NUM_DECKS],
                clock: 0.0,
                limiter: Limiter::new(sample_rate, LIMIT_CEILING),
                tone_frame: 0,
            },
            EngineInput {
                commands: commands_tx,
                returned: returned_rx,
            },
        )
    }

    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    /// The other end of the recording tap; there is only one.
    pub(crate) fn take_record_feed(&mut self) -> Option<Feed> {
        self.record_feed.take()
    }

    /// The other end of the loudness meter's tap; there is only one.
    pub(crate) fn take_meter_feed(&mut self) -> Option<Feed> {
        self.meter_feed.take()
    }

    /// The other end of the broadcast's tap; there is only one.
    pub(crate) fn take_broadcast_feed(&mut self) -> Option<Feed> {
        self.broadcast_feed.take()
    }

    /// Plays input `input` (see [`crate::inputs`]) from `feed`, or from
    /// nothing. Returns the feed it played before, to be dropped off the
    /// audio thread.
    pub(crate) fn set_input(&mut self, input: usize, feed: Option<InputFeed>) -> Option<InputFeed> {
        self.inputs[input].set_feed(feed)
    }

    /// The other end of the preview player's queues; there is only one.
    pub(crate) fn take_preview_input(&mut self) -> Option<PreviewInput> {
        self.preview_input.take()
    }

    /// The other end of the sampler's queues; there is only one.
    pub(crate) fn take_sampler_input(&mut self) -> Option<SamplerInput> {
        self.sampler_input.take()
    }

    fn apply_commands(&mut self) {
        while let Ok(cmd) = self.commands.pop() {
            match cmd {
                Command::Load { deck, track } => {
                    // The tempo a playing deck follows from this one, kept
                    // below so that deck doesn't jump to the new track's.
                    let followed = self.sync_leader == Some(deck)
                        && (0..NUM_DECKS).any(|k| {
                            k != deck && self.decks[k].following && self.decks[k].c.play.get_bool()
                        });
                    let held_bpm = followed
                        .then(|| self.decks[deck].effective_bpm())
                        .filter(|&b| b > 0.0);
                    // A track loaded on a Groove Box or a live input makes
                    // it a track deck again.
                    self.grooves[deck].c.enabled.set_bool(false);
                    self.decks[deck].c.live_input.set(0.0);
                    self.live_seen[deck] = 0;
                    let d = &mut self.decks[deck];
                    d.c.play.set_bool(false);
                    d.expected_pos = None;
                    d.declick = None;
                    d.c.duration.set(track.duration);
                    d.c.track_loaded.set_bool(true);
                    d.show_grid_lock(track.info.grid_locked);
                    d.c.cue_point.set(track.first_beat);
                    d.c.loop_enabled.set_bool(false);
                    d.loop_in_pending = false;
                    d.c.loop_in_pending.set_bool(false);
                    for h in &d.c.hotcues {
                        h.enabled.set_bool(false);
                        h.kind.set(0.0);
                        h.end_position.set(-1.0);
                    }
                    d.c.hotcue_focus.set(-1.0);
                    let [reset_fx, reset_eq, reset_gain] =
                        self.master.auto_reset.each_ref().map(|c| c.get_bool());
                    // The track's own gain, set in its info; auto gain levels
                    // it on top.
                    if let Some(db) = track.info.gain_db {
                        d.c.pregain
                            .set(10f64.powf(db as f64 / 20.0).clamp(0.0, 4.0));
                        d.track_gain = true;
                    } else if d.track_gain || reset_gain {
                        d.c.pregain.set(1.0);
                        d.track_gain = false;
                    }
                    if reset_eq {
                        for (eq, kill) in d.c.eq.iter().zip(&d.c.eq_kill) {
                            eq.set(1.0);
                            kill.set_bool(false);
                        }
                        d.c.filter.set(0.5);
                        for (c, v) in d.c.sculpt.iter().zip(SCULPT_DEFAULTS) {
                            c.set(v);
                        }
                    }
                    // The stems pads belong to the track they were pressed
                    // on: a muted stem (Acapella, Instrumental or a stem's
                    // own pad) left over from the last track would silence
                    // whatever part of the new one it holds, with nothing
                    // lit to say why until its stems are ready. So every
                    // stem plays again; their levels, on the EQ knobs in
                    // stems mode, reset with the EQ.
                    // Their effects too, and whatever still rings from the
                    // last track.
                    for s in &d.c.stems {
                        s.mute.set_bool(false);
                        s.fx.set_bool(false);
                        if reset_eq {
                            s.volume.set(1.0);
                        }
                    }
                    d.stem_fx.clear();
                    d.stem_send = [0.0; MAX_STEMS];
                    d.stem_fed = [0.0; MAX_STEMS];
                    d.stem_muted = [false; MAX_STEMS];
                    d.stem_shifter.stop();
                    if reset_fx {
                        let playing = std::array::from_fn(|k| self.decks[k].c.play.get_bool());
                        self.fx.reset_deck(deck, playing);
                    }
                    self.fx.release_pad_fx(deck);
                    let d = &mut self.decks[deck];
                    // Hot cues and saved loops from the library.
                    for cue in &track.info.hotcues {
                        if let Some(h) = d.c.hotcues.get(cue.number) {
                            if cue.seconds >= 0.0 && cue.seconds < track.duration {
                                h.position.set(cue.seconds);
                                match cue.end.filter(|&end| end > cue.seconds) {
                                    Some(end) => {
                                        h.end_position.set(end.min(track.duration));
                                        h.kind.set(HOTCUE_LOOP);
                                    }
                                    None => h.kind.set(HOTCUE_CUE),
                                }
                                h.enabled.set_bool(true);
                            }
                        }
                    }
                    d.audio = track.audio.clone();
                    d.velocity = 0.0;
                    d.jog_pending = 0.0;
                    d.slip = None;
                    d.slip_moving = false;
                    d.slip_cue = None;
                    if let Some(old) = d.track.replace(track.clone()) {
                        // If the queue is full the old track is freed here; rare and harmless.
                        let _ = self.returned.push(Returned::Done(old));
                    }
                    d.set_seconds(track.first_beat);
                    if let Some(held) = held_bpm.filter(|_| track.bpm > 0.0) {
                        // At the held tempo, or half or double it, whichever
                        // is nearest the track's own.
                        let own = track.bpm_at(track.first_beat);
                        let speed = held * sync_ratio(held, own) / own;
                        let range = d.c.rate_range.get() * d.rate_dir();
                        d.c.rate.set((speed - 1.0) / range);
                    }
                    self.match_key_on_load(deck, &track);
                    // The headphones follow the deck the DJ just loaded, as
                    // VirtualDJ's pflOnSelect does. The cue never reaches
                    // the master, so this can't touch what the room hears.
                    if self.master.pfl_on_load.get_bool() && !self.master.autodj_enabled.get_bool()
                    {
                        for (k, d) in self.decks.iter().enumerate() {
                            d.c.pfl.set_bool(k == deck);
                        }
                    }
                }
                Command::Analysed { deck, track } => {
                    let d = &mut self.decks[deck];
                    let same = match (&d.audio, &track.audio) {
                        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                        _ => false,
                    };
                    if !same {
                        let _ = self.returned.push(Returned::Done(track));
                        continue;
                    }
                    let old_first_beat = d.track.as_ref().map_or(0.0, |t| t.first_beat);
                    d.c.duration.set(track.duration);
                    // Move the cue point and play position if they are still
                    // where loading put them.
                    let cue_untouched = (d.c.cue_point.get() - old_first_beat).abs() < 1e-9;
                    let parked = cue_untouched
                        && !d.c.play.get_bool()
                        && (d.seconds() - old_first_beat).abs() < 1e-9;
                    if cue_untouched {
                        d.c.cue_point.set(track.first_beat);
                    }
                    let first_beat = track.first_beat;
                    d.show_grid_lock(track.info.grid_locked);
                    if let Some(old) = d.track.replace(track) {
                        let _ = self.returned.push(Returned::Done(old));
                    }
                    if parked {
                        d.set_seconds(first_beat);
                    }
                }
                Command::Regrid { deck, track } => {
                    let d = &mut self.decks[deck];
                    let same = match (&d.audio, &track.audio) {
                        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                        _ => false,
                    };
                    let old = if same {
                        d.show_grid_lock(track.info.grid_locked);
                        d.track.replace(track)
                    } else {
                        Some(track)
                    };
                    if let Some(old) = old {
                        let _ = self.returned.push(Returned::Done(old));
                    }
                }
            }
        }
    }

    /// Ejects a stopped deck's track when its eject button is pressed, or
    /// when the deck is turned into a Groove Box.
    fn eject(&mut self) {
        for (deck, d) in self.decks.iter_mut().enumerate() {
            let groove = self.grooves[deck].enabled() && d.track.is_some();
            if groove {
                d.c.play.set_bool(false);
            }
            let pressed = d.eject_edge.pressed(&d.c.eject) && !d.c.play.get_bool();
            // Ejecting an empty Groove Box or live input deck makes it an
            // empty track deck.
            if pressed && d.track.is_none() {
                self.grooves[deck].c.enabled.set_bool(false);
                d.c.live_input.set(0.0);
            }
            if pressed || groove {
                if let Some(track) = d.unload() {
                    // The loader clears the deck from the loaded tracks the UI shows.
                    let _ = self.returned.push(Returned::Ejected { deck, track });
                }
            }
        }
    }

    /// Plays live inputs through decks (`[ChannelN],live_input`). The input
    /// picked last for a deck wins over its Groove Box and the other way
    /// round; an input plays through one deck at a time, so picking it for
    /// another deck takes it off the first; and a deck's track is ejected
    /// (its deck stopped) when an input takes it over.
    fn live_inputs(&mut self) {
        for deck in 0..NUM_DECKS {
            let live = self.decks[deck].live_input();
            let groove = self.grooves[deck].enabled();
            if live > 0 && live != self.live_seen[deck] {
                self.grooves[deck].c.enabled.set_bool(false);
                for other in (0..NUM_DECKS).filter(|&k| k != deck) {
                    if self.decks[other].live_input() == live {
                        self.decks[other].c.live_input.set(0.0);
                        self.live_seen[other] = 0;
                    }
                }
                let d = &mut self.decks[deck];
                d.c.play.set_bool(false);
                if let Some(track) = d.unload() {
                    let _ = self.returned.push(Returned::Ejected { deck, track });
                }
            } else if live > 0 && groove && !self.groove_seen[deck] {
                self.decks[deck].c.live_input.set(0.0);
            }
            self.live_seen[deck] = self.decks[deck].live_input();
            self.groove_seen[deck] = self.grooves[deck].enabled();
        }
    }

    /// The Link instance, to turn on and off off the audio thread (see
    /// [`crate::link::watch`]).
    pub(crate) fn link_handle(&self) -> Arc<rusty_link::AblLink> {
        self.link.handle()
    }

    /// A synced deck follows its leader's tempo, at half or double when
    /// that is closer to its own, and stays on the leader's beat: drift is
    /// caught up by bending the speed a little, never by jumping, and a
    /// nudge by hand moves where it sits against the beat. Turning sync on
    /// while playing, or beatsync, lines the beats up at once. With Link on,
    /// the master deck also leads the Link session (see [`Engine::lead_link`]).
    fn sync(&mut self, link: Option<Moment>) {
        let changed = self.pick_master_deck();
        if let Some(m) = link {
            self.lead_link(m, changed);
        }
        self.publish_beat_clock();
        let synced: [bool; NUM_DECKS] =
            std::array::from_fn(|i| self.decks[i].c.sync_enabled.get_bool());
        self.pick_sync_leader(synced);
        for i in 0..NUM_DECKS {
            let other = self.sync_target(i);
            // sync_key: shift the key to the other deck's, or its relative.
            let d = &mut self.decks[i];
            if d.c.sync_key_edge.pressed(&d.c.sync_key) {
                let theirs = other.and_then(|o| Key::from_mixxx_number(self.decks[o].c.key.get()));
                let d = &self.decks[i];
                if let (Some(mine), Some(theirs)) = (d.file_key, theirs) {
                    let unshifted = mine.playing(0.0, d.speed(), d.c.keylock.get_bool());
                    d.c.pitch.set(unshifted.shift_to_match(theirs) as f64);
                }
            }
            let just_synced = synced[i] && !self.decks[i].was_synced;
            let following = synced[i] && self.sync_leader != Some(i);
            let d = &mut self.decks[i];
            d.was_synced = synced[i];
            let was_following = std::mem::replace(&mut d.following, following);
            let once = d.sync_edge.pressed(&d.c.beatsync);
            // Set again below while following.
            d.sync_speed = None;
            d.sync_bend = 0.0;
            let Some(other) = other else {
                continue;
            };
            let (Some(mine), Some(theirs)) =
                (self.decks[i].track.clone(), self.decks[other].track.clone())
            else {
                continue;
            };
            if !(following || once) || mine.bpm <= 0.0 || theirs.bpm <= 0.0 {
                continue;
            }
            let pair = (Arc::as_ptr(&mine) as usize, Arc::as_ptr(&theirs) as usize);
            let fresh = just_synced || once || !was_following || self.decks[i].sync_pair != pair;
            // With both decks synced, a move of the follower's fader moves
            // the leader, so either fader sets the tempo.
            if following && synced[other] && !fresh {
                let d = &self.decks[i];
                let rate = d.c.rate.get();
                if (rate - d.rate_written).abs() > 1e-9 {
                    let range = d.c.rate_range.get() * d.rate_dir();
                    let wanted = mine.bpm_at(d.seconds()) * (1.0 + rate * range);
                    let l = &self.decks[other];
                    let at = theirs.bpm_at(l.seconds());
                    let range = l.c.rate_range.get() * l.rate_dir();
                    l.c.rate.set((wanted / d.sync_ratio / at - 1.0) / range);
                }
            }
            // Tempos where each deck is, so a deck whose tempo drifts keeps
            // the other in step all the way through.
            let here = mine.bpm_at(self.decks[i].seconds());
            let leader_bpm = self.decks[other].effective_bpm();
            let (leader_beat, leader_moving, leader_nudged) = {
                let l = &self.decks[other];
                (
                    theirs.beat_at(l.seconds()),
                    l.c.play.get_bool(),
                    l.nudging(),
                )
            };
            let d = &mut self.decks[i];
            if fresh {
                d.sync_pair = pair;
                d.sync_ratio = sync_ratio(leader_bpm, here);
                d.phase_offset = 0.0;
            }
            let speed = leader_bpm * d.sync_ratio / here;
            let range = d.c.rate_range.get() * d.rate_dir();
            d.c.rate.set((speed - 1.0) / range);
            d.rate_written = d.c.rate.get();
            d.sync_speed = following.then_some(speed);

            let beat = mine.beat_at(d.seconds());
            let target = leader_beat * d.sync_ratio;
            // Beats this deck is behind where it belongs (negative: ahead).
            let behind = phase_error(target + d.phase_offset, beat);
            let playing = d.c.play.get_bool();
            if once || (just_synced && playing) {
                // Lands on the nearest beat in phase, half a beat at most
                // away (a beat on, at the very start of the track).
                let to = mine.seconds_at(beat + behind);
                let to = if to < 0.0 {
                    mine.seconds_at(beat + behind + 1.0)
                } else {
                    to
                };
                d.set_seconds(to);
                d.sync_bend = 0.0;
            } else if !following || !playing || !leader_moving {
                d.sync_bend = 0.0;
            } else if d.nudging() || leader_nudged {
                d.phase_offset = phase_error(beat, target);
                d.sync_bend = 0.0;
            } else {
                // Closes the gap over about a beat, never faster than
                // SYNC_MAX_BEND, which is too little to hear.
                d.sync_bend = behind.clamp(-SYNC_MAX_BEND, SYNC_MAX_BEND);
            }
        }
    }

    /// The deck synced decks follow. With one deck synced it follows a
    /// playing deck among the others; with more, the leader is the synced
    /// deck that has been playing longest, handing over when it stops, as
    /// Link's master deck does.
    fn pick_sync_leader(&mut self, synced: [bool; NUM_DECKS]) {
        self.read_pinned_leader();
        let count = synced.iter().filter(|&&s| s).count();
        let has_bpm = |d: &Deck| d.track.as_ref().is_some_and(|t| t.bpm > 0.0);
        if let Some(p) = self.sync_pinned.filter(|&p| has_bpm(&self.decks[p])) {
            self.sync_leader = Some(p);
        } else if count == 0 {
            self.sync_leader = None;
        } else {
            self.pick_soft_leader(synced, count);
        }
        self.show_sync_leader(synced);
    }

    /// Takes a pin or unpin from the decks' `sync_leader` controls: 2 on a
    /// deck not pinned pins it (the newest wins), anything else on the
    /// pinned deck unpins it.
    fn read_pinned_leader(&mut self) {
        let pinned = |d: &Deck| d.c.sync_leader.get().round() as i32 == 2;
        if let Some(p) = self.sync_pinned {
            if !pinned(&self.decks[p]) {
                self.sync_pinned = None;
            }
        }
        if let Some(k) =
            (0..NUM_DECKS).find(|&k| self.sync_pinned != Some(k) && pinned(&self.decks[k]))
        {
            self.sync_pinned = Some(k);
        }
    }

    /// Writes the leader back to the decks' `sync_leader` controls: 2 on the
    /// pinned deck, 1 on the leader while another deck follows it.
    fn show_sync_leader(&self, synced: [bool; NUM_DECKS]) {
        // Lit while any deck syncs: on the deck it follows, or on a synced
        // deck leading on its own.
        let followed = synced.iter().any(|&s| s);
        for (k, d) in self.decks.iter().enumerate() {
            let v = if self.sync_pinned == Some(k) {
                2.0
            } else if followed && self.sync_leader == Some(k) {
                1.0
            } else {
                0.0
            };
            d.c.sync_leader.set(v);
        }
    }

    fn pick_soft_leader(&mut self, synced: [bool; NUM_DECKS], count: usize) {
        let candidate = |k: usize| if count == 1 { !synced[k] } else { synced[k] };
        let playing =
            |d: &Deck| d.c.play.get_bool() && d.track.as_ref().is_some_and(|t| t.bpm > 0.0);
        // A synced deck left playing on its own after a mix leads, so the
        // stopped deck's next track doesn't move it, and keeps leading
        // while it plays.
        let alone = |k: usize| count == 1 && synced[k];
        let current = self.sync_leader.filter(|&k| candidate(k) || alone(k));
        self.sync_leader = current
            .filter(|&k| playing(&self.decks[k]))
            .or_else(|| (0..NUM_DECKS).find(|&k| candidate(k) && playing(&self.decks[k])))
            .or_else(|| (0..NUM_DECKS).find(|&k| alone(k) && playing(&self.decks[k])))
            .or(current.filter(|&k| candidate(k)))
            .or_else(|| (0..NUM_DECKS).find(|&k| candidate(k) && self.decks[k].track.is_some()))
            .or_else(|| (0..NUM_DECKS).find(|&k| candidate(k)));
    }

    /// The deck `deck` syncs to: the leader, or else a playing deck, or
    /// else any deck with a track.
    /// Key matching on load: a track loaded while another deck plays is
    /// shifted by up to `key_match_on_load` semitones so its key mixes with
    /// that deck's, with key lock on so the tempo fader doesn't move it
    /// away again. A track that already mixes, or that no shift in range
    /// brings close enough, plays in its own key.
    fn match_key_on_load(&mut self, deck: usize, track: &Track) {
        let range = self.master.key_match_on_load.get().round() as i32;
        if range <= 0 {
            return;
        }
        let playing = |k: usize| k != deck && self.decks[k].c.play.get_bool();
        let other = self
            .sync_leader
            .filter(|&k| playing(k))
            .or_else(|| (0..NUM_DECKS).find(|&k| playing(k)));
        let theirs = other.and_then(|o| Key::from_mixxx_number(self.decks[o].c.key.get()));
        let (Some(mine), Some(theirs)) = (Key::parse(&track.key), theirs) else {
            return;
        };
        let d = &self.decks[deck];
        // Without key lock the speed counts too, as when it plays.
        let heard = mine.playing(0.0, d.speed(), d.c.keylock.get_bool());
        if heard.compatible(theirs) {
            d.c.pitch.set(0.0);
            return;
        }
        let shift = mine.shift_to_mix(theirs, range, key::mixing());
        d.c.pitch.set(shift.unwrap_or(0) as f64);
        if shift.is_some() {
            d.c.keylock.set_bool(true);
        }
    }

    fn sync_target(&self, deck: usize) -> Option<usize> {
        let has_bpm = |d: &Deck| d.track.as_ref().is_some_and(|t| t.bpm > 0.0);
        self.sync_leader
            .filter(|&k| k != deck)
            .or_else(|| {
                (0..NUM_DECKS).find(|&k| {
                    k != deck && has_bpm(&self.decks[k]) && self.decks[k].c.play.get_bool()
                })
            })
            .or_else(|| (0..NUM_DECKS).find(|&k| k != deck && has_bpm(&self.decks[k])))
    }

    /// With quantize on, or on a synced deck, a deck that jumps (hot cue,
    /// CUE, beat jump, a click on the overview) while the other deck plays
    /// lands on the same part of the beat, at most half a step from where
    /// it was going, as in Mixxx.
    ///
    /// PLAY on a paused deck starts where it was paused. Only a synced deck
    /// is lined up as it starts, and then just within the beat (half a beat
    /// at most), never by the quantize size: a start that moved by up to
    /// half a bar sounded like a jump.
    fn land_in_phase(&mut self, moved: [Move; NUM_DECKS]) {
        for (i, moved) in moved.into_iter().enumerate() {
            let Some(other) = self.sync_target(i) else {
                continue;
            };
            let (d, l) = (&self.decks[i], &self.decks[other]);
            let wanted = match moved {
                Move::None => false,
                Move::Start => d.following,
                Move::Jump => d.c.quantize.get_bool() || d.following,
            };
            if !wanted || !d.c.play.get_bool() || !l.c.play.get_bool() || d.nudging() {
                continue;
            }
            let (Some(mine), Some(theirs)) = (&d.track, &l.track) else {
                continue;
            };
            if mine.bpm <= 0.0 || theirs.bpm <= 0.0 {
                continue;
            }
            let (ratio, offset) = if d.following {
                (d.sync_ratio, d.phase_offset)
            } else {
                (sync_ratio(l.effective_bpm(), d.effective_bpm()), 0.0)
            };
            // Quantize lines the jump up within its step (a bar at 4 beats,
            // a sixteenth at 1/16); a synced deck keeps at least the beat,
            // which its phase lock holds.
            let step = if moved == Move::Start {
                1.0
            } else if d.following {
                d.quantize_beats().max(1.0)
            } else {
                d.quantize_beats()
            };
            let beat = mine.beat_at(d.seconds());
            let theirs_beat = theirs.beat_at(l.seconds()) * ratio + offset;
            let mut to = beat + step * phase_error(theirs_beat / step, beat / step);
            // A start before the track or a jump into a loop stays inside it.
            let looping = d.c.loop_enabled.get_bool() && d.loop_end > d.loop_start;
            let first = if looping { d.loop_start - 1e-6 } else { 0.0 };
            if mine.seconds_at(to) < first {
                to += step;
            }
            let to = mine.seconds_at(to);
            if looping && !(d.loop_start - 1e-6..d.loop_end).contains(&to) {
                continue;
            }
            self.decks[i].set_seconds(to);
        }
    }

    /// Picks the master deck, which Link and MIDI clock follow: the deck
    /// playing; with both playing it stays the one that started first, and
    /// when it stops the other takes over. True when it changed.
    fn pick_master_deck(&mut self) -> bool {
        let playing =
            |d: &Deck| d.c.play.get_bool() && d.track.as_ref().is_some_and(|t| t.bpm > 0.0);
        let master = self
            .link_master
            .filter(|&k| playing(&self.decks[k]))
            .or_else(|| (0..NUM_DECKS).find(|&i| playing(&self.decks[i])));
        let changed = master != self.link_master;
        self.link_master = master;
        self.link
            .c
            .master_deck
            .set(master.map_or(0.0, |k| k as f64 + 1.0));
        changed
    }

    /// Makes Link follow the master deck: the session takes its tempo and
    /// beat, and other apps follow. With no deck playing, the session keeps
    /// its last tempo.
    fn lead_link(&mut self, m: Moment, changed: bool) {
        let Some(k) = self.link_master else { return };
        let d = &self.decks[k];
        let Some(track) = &d.track else { return };
        let tempo = d.effective_bpm();
        let beat = track.beat_at(d.seconds());
        let off_beat = phase_error(beat, m.beat).abs() > LINK_PHASE_TOLERANCE;
        let align = changed || m.just_enabled || off_beat;
        if align || (tempo - m.tempo).abs() > 1e-6 {
            self.link.lead(tempo, align.then_some(beat));
        }
    }

    /// Publishes the master deck's beat and tempo as this buffer is heard,
    /// for MIDI clock out (see [`crate::midi_clock`]).
    fn publish_beat_clock(&mut self) {
        let heard_at = self.link.heard_at;
        let now = self.link_master.and_then(|k| {
            let d = &self.decks[k];
            let track = d.track.as_ref()?;
            Some((track.beat_at(d.seconds()), d.effective_bpm()))
        });
        match now {
            Some((beat, tempo)) if tempo > 0.0 => self.beat_clock.publish(heard_at, beat, tempo),
            _ => self.beat_clock.publish(heard_at, 0.0, 0.0),
        }
    }

    /// The master deck's beat clock, for [`crate::midi_clock::start`].
    pub(crate) fn beat_clock(&self) -> Arc<crate::midi_clock::BeatClock> {
        self.beat_clock.clone()
    }

    /// Where each deck is, for smooth drawing; updated every buffer.
    /// The plugin host's link to the effect slots, for
    /// [`crate::plugins::PluginHost::start`]; there is one, taken once.
    pub fn take_plugin_link(&mut self) -> Option<crate::effects::PluginLink> {
        self.fx.take_plugin_link()
    }

    /// Where notes for the Instrument input come in; there is one, taken once.
    pub fn take_note_input(&mut self) -> Option<crate::effects::NoteInput> {
        self.fx.take_note_input()
    }

    pub fn visual_positions(&self) -> Arc<VisualPositions> {
        self.visual.clone()
    }

    /// Renders one buffer of interleaved audio with `channels` channels. With
    /// four or more channels, channels 3 and 4 carry the headphone mix.
    /// Gapless playback: a deck with a `play_next` deck reads on past its
    /// track's end into that deck's track. In the buffer where its track
    /// ends it plays on into the next track to the end of the buffer, and
    /// the next deck takes over from there (see [`Self::hand_over`]), so
    /// the two join with neither a gap nor an overlap. (A UI timer can only
    /// start the next deck to within a screen frame.)
    fn start_gapless(&mut self, frames: usize) {
        let out_rate = self.sample_rate;
        for d in &mut self.decks {
            d.join_after = None;
            // Reads before the start hear the track before only just after
            // it ended.
            if !(d.c.play.get_bool() && d.pos < JOIN_SECONDS * d.source_rate()) {
                d.join_before = None;
            }
        }
        for i in 0..NUM_DECKS {
            let next = self.decks[i].c.play_next.get().round() as usize;
            let Some(j) = next.checked_sub(1).filter(|&j| j < NUM_DECKS && j != i) else {
                continue;
            };
            if self.decks[j].audio.is_none() || self.decks[j].c.play.get_bool() {
                continue;
            }
            let a = &mut self.decks[i];
            let Some(len) = a.audio.as_ref().map(|x| x.len() as f64) else {
                continue;
            };
            let looping = a.c.loop_enabled.get_bool() && a.loop_end > a.loop_start;
            if !a.c.play.get_bool() || looping || a.velocity <= 0.0 {
                continue;
            }
            a.join_after = Some(j);
            let step = a.velocity * a.source_rate() / out_rate;
            if a.pos + step * frames as f64 >= len {
                a.c.play_next.set(0.0);
                a.hand_over = Some(j);
            }
        }
    }

    /// The other half of [`Self::start_gapless`]: deck `i` has played past
    /// its track's end into deck `j`'s, which starts where it got to, with
    /// the time-stretcher's state if it was in use, so even a key-locked
    /// deck carries on without a seam.
    /// Returns whether deck `i`'s track has ended.
    fn hand_over(&mut self, i: usize, j: usize) -> bool {
        let (a_len, a_rate, at) = {
            let a = &self.decks[i];
            let Some(audio) = a.audio.as_ref() else {
                return true;
            };
            (
                audio.len() as f64,
                a.source_rate(),
                Arc::as_ptr(audio) as usize,
            )
        };
        let [a, b] = self.decks.get_disjoint_mut([i, j]).expect("two decks");
        a.hand_over = None;
        let past = a.pos - a_len;
        if past < 0.0 {
            // It slowed down and didn't get there after all.
            a.c.play_next.set((j + 1) as f64);
            return false;
        }
        a.pos = a_len;
        if b.audio.is_none() || b.c.play.get_bool() {
            return true;
        }
        let scale = b.source_rate() / a_rate;
        b.pos = (past * scale).max(0.0);
        if a.shifter.is_active() {
            std::mem::swap(&mut a.shifter, &mut b.shifter);
            b.shifter.carry_over(a_len, scale);
            a.shifter.stop();
        }
        b.join_before = Some((i, at));
        b.c.play.set_bool(true);
        true
    }

    pub fn process(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        let frames = out.len() / channels;
        if frames == 0 {
            return;
        }
        if frames > BLOCK {
            // Longer buffers than the mix's blocks are rare: play them in parts.
            for part in out.chunks_mut(BLOCK * channels) {
                self.process(part, channels);
            }
            return;
        }
        crate::dsp::flush_denormals();
        let dt = frames as f64 / self.sample_rate;
        // Positions published below are where the decks are at the end of
        // this buffer, which starts playing after the current one.
        let buffer_end = Instant::now() + Duration::from_secs_f64(dt);
        self.apply_commands();
        self.eject();
        self.live_inputs();
        let link = self.link.begin(frames, self.sample_rate);
        self.sync(link);
        let mut moved = [Move::None; NUM_DECKS];
        let clock = self.clock;
        for (deck, d) in self.decks.iter_mut().enumerate() {
            let before = d.pos;
            d.clock = clock;
            d.reset_rate(dt);
            d.handle_controls();
            moved[deck] = if d.pos != before {
                Move::Jump
            } else if !d.was_playing && d.c.play.get_bool() {
                Move::Start
            } else {
                Move::None
            };
            if let (Some((edit, seconds)), Some(track)) = (d.grid_edit.take(), &d.track) {
                let _ = self.returned.push(Returned::GridEdit {
                    deck,
                    track: track.clone(),
                    edit,
                    seconds,
                });
            }
        }
        self.land_in_phase(moved);
        self.start_gapless(frames);
        // The audio each deck's own joins on to, for this buffer.
        let joins: [Joins; NUM_DECKS] = std::array::from_fn(|i| {
            let d = &self.decks[i];
            let before = d.join_before.and_then(|(k, at)| {
                self.decks[k]
                    .audio
                    .clone()
                    .filter(|a| Arc::as_ptr(a) as usize == at)
            });
            (
                before,
                d.join_after.and_then(|k| self.decks[k].audio.clone()),
            )
        });
        self.update_color_fx();
        self.samplers.update();
        self.preview.update();
        // Tempo-synced effects on the main mix follow the first playing deck.
        let main_deck = self
            .decks
            .iter()
            .position(|d| d.c.play.get_bool() && d.track.as_ref().is_some_and(|t| t.bpm > 0.0))
            .unwrap_or(0);
        let main_beat = self.decks[main_deck].beat_seconds();
        // Gates, LFOs and rolls lock to each playing deck's beat grid; on a
        // stopped deck they run free rather than freeze.
        let grid = |d: &Deck| {
            d.track
                .as_ref()
                .filter(|t| t.bpm > 0.0 && d.c.play.get_bool())
                .map(|t| t.beat_at(d.seconds()))
        };
        self.fx.set_beats(std::array::from_fn(|t| {
            if t < MAIN {
                grid(&self.decks[t])
            } else if t == MAIN {
                grid(&self.decks[main_deck])
            } else {
                None
            }
        }));
        let main_transport = self.decks[main_deck].transport();
        self.fx.set_transport(std::array::from_fn(|t| {
            if t >= MAIN {
                main_transport
            } else {
                self.decks[t].transport()
            }
        }));
        self.fx.prepare(
            std::array::from_fn(|t| {
                if t >= MAIN {
                    main_beat
                } else {
                    self.decks[t].beat_seconds()
                }
            }),
            self.clock,
        );
        debug_assert_eq!(TARGETS, NUM_DECKS + 1 + NUM_INPUTS);
        // The input each deck plays instead of a track, if any: that input's
        // own channel is quiet meanwhile.
        let live_on: [usize; NUM_DECKS] = std::array::from_fn(|i| {
            let d = &self.decks[i];
            if d.track.is_none() && !self.grooves[i].enabled() {
                d.live_input()
            } else {
                0
            }
        });
        for (i, input) in self.inputs.iter_mut().enumerate() {
            let on_deck = live_on.contains(&(i + 1));
            input.render(frames, &mut self.fx, input_target(i), dt as f32, on_deck);
        }
        let duck = self.ducker.settings();

        let mut x = self.master.crossfader.get() as f32;
        if self.master.xfader_reverse.get_bool() {
            x = -x;
        }
        let sides = xfader_gains(x, self.master.xfader_shape.get() as u8);
        // Each deck follows the side it is assigned to; thru decks skip the
        // crossfader, and so does every deck while it is switched off.
        let xfader_on =
            self.master.xfader_enabled.get_bool() || self.master.autodj_enabled.get_bool();
        let xfade_target: [f32; NUM_DECKS] =
            std::array::from_fn(|i| match self.decks[i].c.orientation.get().round() as i32 {
                _ if !xfader_on => 1.0,
                0 => sides[0],
                2 => sides[1],
                _ => 1.0,
            });
        let master_target = self.master.gain.get() as f32;
        // Recording or on air (either tap copying), the limiter keeps true
        // peaks under the air ceiling (-1 dBTP), where lossy encoders don't
        // clip; otherwise under -0.5 dB.
        let ceiling = if self.record.is_active() || self.broadcast.is_active() {
            10f32
                .powf(self.master.air_ceiling.get() as f32 / 20.0)
                .min(LIMIT_CEILING)
        } else {
            LIMIT_CEILING
        };
        if ceiling != self.limiter.ceiling() {
            self.limiter.set_ceiling(ceiling);
        }
        let auto_gain_on = self.master.auto_gain.get_bool();
        let auto_gain_target = self.master.auto_gain_target.get() as f32;
        let balance_target = self.master.balance.get() as f32;
        let head_gain = self.master.head_gain.get() as f32;
        // headMix: -1 = cue only, +1 = main only. Constant power, as in
        // Mixxx, so the centre plays both equally loud.
        let mix = (self.master.head_mix.get() as f32 + 1.0) * std::f32::consts::FRAC_PI_4;
        let (cue_mix, main_mix) = (mix.cos(), mix.sin());
        let booth_gain = self.master.booth_gain.get() as f32;
        let head_split = self.master.head_split.get_bool();
        let tone = self.master.test_tone.get().round() as u8;
        if tone == 0 {
            self.tone_frame = 0;
        }

        struct Plan {
            moving: bool,
            /// Speed at the start and end of the buffer.
            velocity: (f64, f64),
            /// Source frames per output frame at normal speed.
            ratio: f64,
            /// How far the slip position moves this buffer, in source frames.
            slip_step: f64,
            loop_frames: Option<(f64, f64)>,
            eq_from: [f32; 3],
            eq_to: [f32; 3],
            pre: (f32, f32),
            quick: (f32, f32),
            vol: (f32, f32),
            xf: (f32, f32),
            pfl: bool,
            /// The metronome is on and the deck has a grid: source frames
            /// per second, to find the beat at the play position.
            metronome: Option<f64>,
            /// This buffer's audio is in the deck's `shifted` buffer.
            shifted: bool,
            /// The deck goes into or out of the time-stretcher this buffer.
            shift_fade: Option<ShiftFade>,
            /// Dragging a scrolling waveform: fast passes are softened.
            scrub: bool,
            /// Stem gains at the start and end of the buffer, when the deck
            /// plays stems.
            stems: Option<([f32; MAX_STEMS], [f32; MAX_STEMS])>,
            /// The crossfade to high-quality stems at the start and end.
            stem_upgrade: (f32, f32),
            /// What each stem sends into the stem effect at the start and
            /// end, while any does.
            stem_send: Option<([f32; MAX_STEMS], [f32; MAX_STEMS])>,
            /// The stem effect plays this buffer (sent to, or ringing on).
            stem_fx: bool,
        }
        let plans: [Plan; NUM_DECKS] = std::array::from_fn(|i| {
            let d = &mut self.decks[i];
            let sr = d.source_rate();
            let eq_to = std::array::from_fn(|k| {
                if d.c.eq_kill[k].get_bool() {
                    0.0
                } else {
                    d.c.eq[k].get() as f32
                }
            });
            let beat = d.beat_seconds();
            d.filter
                .prepare(d.c.cfx_preset.get(), d.c.filter.get(), beat);
            let k = d.c.sculpt.each_ref().map(|c| c.get());
            d.sculpt.set(k[0], k[1], k[2], k[3]);
            let loop_frames = (d.c.loop_enabled.get_bool() && d.loop_end > d.loop_start)
                .then_some((d.loop_start * sr, d.loop_end * sr));
            let target = d.target_velocity(dt);
            let ratio = sr / self.sample_rate;
            let playing = d.c.play.get_bool() && d.audio.is_some();
            let auto_target = match d.audio.as_ref().and_then(|a| a.loudness()) {
                Some(l) if auto_gain_on => l.gain(auto_gain_target, LIMIT_CEILING),
                _ => 1.0,
            };
            // A playing deck glides to a new auto gain (its analysis just
            // finished, or auto gain was switched); a stopped one jumps.
            let auto_gain = if playing {
                glide_db(d.auto_gain, auto_target, AUTO_GAIN_GLIDE_DB * dt as f32)
            } else {
                auto_target
            };
            d.auto_gain = auto_gain;
            d.c.replaygain.set(auto_gain as f64);
            let pregain = d.c.pregain.get() as f32;
            // Mute silences the deck in the main mix; it can still be cued.
            let volume = if d.c.mute.get_bool() {
                0.0
            } else {
                d.c.volume.get() as f32
            };
            // Play and stop act at once, as in Mixxx; scratching, jog and
            // search speeds ramp across the buffer.
            let held = d.c.scratch2_enable.get_bool() || d.c.scrub_enable.get_bool();
            // Holding the overview cuts the sound at once instead of winding
            // down through what the deck was playing.
            let from = if d.c.seek_hold.get_bool() || (playing != d.was_playing && !held) {
                target
            } else {
                d.velocity
            };
            d.was_playing = playing;
            // Key lock and key shifts go through the time-stretcher while
            // playing forwards, nudges and pitch bends included; scratching,
            // searching and reverse play as a record does.
            let keylock = d.c.keylock.get_bool();
            let semitones = d.c.pitch.get();
            let shifting = from > 0.0
                && target > 0.0
                && from.max(target) <= MAX_RATE
                && !held
                && d.c.rate_search.get() == 0.0
                && (keylock || semitones.abs() > 1e-3);
            // The deck was heard moving last buffer (not starting from a stop).
            let was_moving = d.velocity != 0.0 && from != 0.0;
            let was_shifting = d.shifter.is_active();
            let shift_fade_frames = (DECLICK_SECONDS * self.sample_rate).round().max(1.0) as usize;
            let mut shift_fade = None;
            let stems = d.audio.as_ref().and_then(|a| a.stems()).map(|s| &**s);
            let shown = stems.map_or(0, |s| s as *const Stems as usize);
            if shown != d.stems_shown {
                d.stems_shown = shown;
                d.c.stem_count.set(stems.map_or(0, Stems::count) as f64);
                for (k, c) in d.c.stems.iter().enumerate() {
                    let info = stems.and_then(|s| s.info.get(k));
                    c.color.set(info.map_or(-1.0, |i| i.color as f64));
                }
                d.stem_gains = [1.0; MAX_STEMS];
                // Stems that are already high quality when they turn up
                // start there.
                d.stem_upgrade = if stems.and_then(Stems::ready_upgrade).is_some() {
                    1.0
                } else {
                    0.0
                };
            }
            let upgrade = match stems.and_then(Stems::ready_upgrade) {
                Some(_) => (
                    d.stem_upgrade,
                    (d.stem_upgrade
                        + frames as f32 / (STEM_UPGRADE_SECONDS * self.sample_rate) as f32)
                        .min(1.0),
                ),
                None => (0.0, 0.0),
            };
            d.stem_upgrade = upgrade.1;
            d.stem_fx.prepare(d.c.stem_fx.get(), beat);
            let capture = d.stem_fx.capture_seconds(beat);
            let mut send_to = [0.0f32; MAX_STEMS];
            let stem_gains = stems.map(|_| {
                let to = std::array::from_fn(|k| {
                    let c = &d.c.stems[k];
                    let fx = c.fx.get_bool();
                    if !fx {
                        d.stem_fed[k] = 0.0;
                    }
                    // Muted with its effect just switched on (an echo out
                    // in one press): it plays on until the echo holds it.
                    let held =
                        c.mute.get_bool() && fx && !d.stem_muted[k] && d.stem_fed[k] < capture;
                    let muted = c.mute.get_bool() && !held;
                    d.stem_muted[k] = muted;
                    let gain = if muted { 0.0 } else { c.volume.get() as f32 };
                    if fx && gain > 0.0 {
                        d.stem_fed[k] += dt;
                        send_to[k] = gain;
                    }
                    gain
                });
                (d.stem_gains, to)
            });
            // The stems sent into the stem effect, gliding like the gains.
            let send_from = d.stem_send;
            d.stem_send = send_to;
            let sending = stem_gains.is_some()
                && d.audio.is_some()
                && send_from.iter().chain(&send_to).any(|&g| g > 0.0);
            let stem_send = sending.then_some((send_from, send_to));
            match &d.audio {
                Some(audio) if shifting => {
                    // The average of the speed ramp the frame loop below takes.
                    let speed = from + (target - from) * (frames + 1) as f64 / (2 * frames) as f64;
                    if d.shifted.len() < frames {
                        d.shifted.resize(frames, [0.0; 2]);
                    }
                    let transpose = transpose_factor(semitones, speed, keylock);
                    let out = &mut d.shifted[..frames];
                    match (stems, stem_gains) {
                        (Some(s), Some((from, to))) => {
                            let span = speed * ratio * frames as f64;
                            let mix = StemMix::blending(s, audio, from, to, upgrade, span);
                            d.shifter.render(
                                &mix,
                                d.pos,
                                speed,
                                ratio,
                                transpose,
                                loop_frames,
                                out,
                            );
                        }
                        _ => d.shifter.render(
                            &joined(audio, &joins[i]),
                            d.pos,
                            speed,
                            ratio,
                            transpose,
                            loop_frames,
                            out,
                        ),
                    }
                    // The stems sent into the stem effect, through their own
                    // stretcher, while there are any.
                    match (stems, stem_send) {
                        (Some(s), Some((from, to))) => {
                            if d.stem_shifted.len() < frames {
                                d.stem_shifted.resize(frames, [0.0; 2]);
                            }
                            let span = speed * ratio * frames as f64;
                            let mix = StemMix::solo(s, audio, from, to, upgrade, span);
                            d.stem_shifter.render(
                                &mix,
                                d.pos,
                                speed,
                                ratio,
                                transpose,
                                loop_frames,
                                &mut d.stem_shifted[..frames],
                            );
                        }
                        _ => d.stem_shifter.stop(),
                    }
                    // Coming into the stretcher from playing as a record
                    // (after a scratch, say): fade over from the record.
                    if !was_shifting && was_moving {
                        shift_fade = Some(ShiftFade::In(shift_fade_frames.min(frames)));
                    }
                }
                Some(audio) if was_shifting && (from != 0.0 || target != 0.0) => {
                    // Leaving the stretcher while still moving: fade from
                    // what it would have played on.
                    let n = shift_fade_frames.min(frames);
                    if d.shifted.len() < n {
                        d.shifted.resize(n, [0.0; 2]);
                    }
                    let out = &mut d.shifted[..n];
                    match (stems, stem_gains) {
                        (Some(s), Some((from, to))) => {
                            let span = d.shifter.last_step() * n as f64;
                            let mix = StemMix::blending(s, audio, from, to, upgrade, span);
                            d.shifter.render_on(&mix, d.pos, loop_frames, out);
                        }
                        _ => {
                            d.shifter
                                .render_on(&joined(audio, &joins[i]), d.pos, loop_frames, out)
                        }
                    }
                    d.shifter.stop();
                    d.stem_shifter.stop();
                    shift_fade = Some(ShiftFade::Out(n));
                }
                _ => {
                    d.shifter.stop();
                    d.stem_shifter.stop();
                }
            }
            if let Some((_, to)) = stem_gains {
                d.stem_gains = to;
            }
            // A beatjump, hotcue, seek or sync jump moved the play position:
            // fade from where the deck would have been, so it doesn't click.
            // (The time-stretcher fades its own jumps.)
            match d.expected_pos {
                Some(e) if !shifting && (d.pos - e).abs() > 0.5 => {
                    d.declick = Some(Declick::new(e, self.sample_rate));
                }
                _ if shifting => d.declick = None,
                _ => {}
            }
            Plan {
                stems: stem_gains,
                stem_upgrade: upgrade,
                stem_send,
                stem_fx: stem_send.is_some() || d.stem_fx.ringing(),
                shifted: shifting && d.audio.is_some(),
                shift_fade,
                scrub: d.c.scrub_enable.get_bool(),
                moving: d.audio.is_some() && (from != 0.0 || target != 0.0),
                velocity: (from, target),
                ratio,
                slip_step: if playing || d.slip_moving {
                    d.forward_speed() * ratio * frames as f64
                } else {
                    0.0
                },
                loop_frames,
                eq_from: d.eq_gains,
                eq_to,
                // A fast jog search plays silently; the ramp avoids clicks.
                pre: (
                    d.pregain,
                    if d.search_quiet > 0.0 {
                        0.0
                    } else {
                        pregain * auto_gain
                    },
                ),
                quick: (
                    d.quick,
                    if d.c.quick_enabled.get_bool() {
                        1.0
                    } else {
                        0.0
                    },
                ),
                vol: (d.volume, volume),
                xf: (self.xfade[i], xfade_target[i]),
                pfl: d.c.pfl.get_bool(),
                metronome: (d.c.metronome.get_bool()
                    && d.track.as_ref().is_some_and(|t| !t.grid.is_empty()))
                .then_some(sr),
            }
        });
        let master = (self.master_gain, master_target);
        let balance = (self.balance, balance_target);

        let mut deck_peak = [[0.0f32; 2]; NUM_DECKS];
        let mut master_peak = [0.0f32; 2];
        let mut master_clip = [false; 2];
        let mut ended = [false; NUM_DECKS];
        let out_rate = self.sample_rate;
        // The mix runs in stages over the whole buffer, so effects can work
        // a block at a time (plugin slots need that): each deck's audio up
        // to its effects, then the effects, then the mix up to the main
        // mix's effects, then those, then the rest.
        let mut deck_buf = std::mem::take(&mut self.deck_buf);
        let mut click_buf = std::mem::take(&mut self.click_buf);
        let mut main_buf = std::mem::take(&mut self.main_buf);
        let mut cue_buf = std::mem::take(&mut self.cue_buf);
        // Groove Boxes render their buffer first, on the beat of the
        // leading deck (the one SYNC follows, else the first playing), else
        // Link's, else their own tempo.
        let mut groove_buf = std::mem::take(&mut self.groove_buf);
        let mut duck_buf = std::mem::take(&mut self.duck_buf);
        let groove_on: [bool; NUM_DECKS] =
            std::array::from_fn(|i| self.grooves[i].enabled() && self.decks[i].track.is_none());
        for g in duck_buf.iter_mut() {
            g[..frames].fill(1.0);
        }
        if groove_on.iter().any(|&on| on) {
            let playing =
                |d: &Deck| d.c.play.get_bool() && d.track.as_ref().is_some_and(|t| t.bpm > 0.0);
            let leader = self
                .sync_leader
                .filter(|&k| playing(&self.decks[k]))
                .or_else(|| (0..NUM_DECKS).find(|&k| playing(&self.decks[k])))
                .and_then(|k| {
                    let d = &self.decks[k];
                    Some((d.effective_bpm(), d.track.as_ref()?.beat_at(d.seconds())))
                });
            let link_beat = link.map(|m| (m.tempo, m.beat));
            for i in (0..NUM_DECKS).filter(|&i| groove_on[i]) {
                let g = &mut self.grooves[i];
                let source = pick_source(leader, link_beat, g.c.bpm.get());
                if source.beat.is_some() {
                    // Shown, and kept for when the leader stops.
                    g.c.bpm.set(source.bpm);
                }
                g.c.following.set_bool(source.beat.is_some());
                let mut kicks = [0usize; 32];
                let mut n = 0;
                let play = self.decks[i].c.play.get_bool();
                g.render(play, source, &mut groove_buf[i][..frames], |f| {
                    if n < kicks.len() {
                        kicks[n] = f;
                        n += 1;
                    }
                });
                let target = (g.c.duck_deck.get().round() as usize).wrapping_sub(1);
                if target < NUM_DECKS && target != i {
                    let depth = 1.0 - 10f32.powf(-(g.c.duck_amount.get() as f32) / 20.0);
                    // Back up over about half a 16th.
                    let recover = 15.0 / source.bpm.max(1.0) * 0.5 * self.sample_rate;
                    let fall = (-1.0 / recover).exp() as f32;
                    let mut env = self.duck_env[target];
                    let mut k = 0;
                    for (f, gain) in duck_buf[target][..frames].iter_mut().enumerate() {
                        while k < n && kicks[k] == f {
                            env = 1.0;
                            k += 1;
                        }
                        *gain *= 1.0 - depth * env;
                        env *= fall;
                    }
                    self.duck_env[target] = env;
                }
            }
        }
        for f in 0..frames {
            let r = (f + 1) as f32 / frames as f32;
            let lerp = |(a, b): (f32, f32)| a + (b - a) * r;
            for (i, d) in self.decks.iter_mut().enumerate() {
                let p = &plans[i];
                let mut s = [0.0f32; 2];
                let mut send = [0.0f32; 2];
                if p.moving && !ended[i] {
                    let audio = d.audio.as_ref().unwrap();
                    let (v0, v1) = p.velocity;
                    // Ramp the speed across the buffer so scratching does not zip.
                    let v = v0 + (v1 - v0) * r as f64;
                    let source = |pos: f64| match (p.stems, audio.stems()) {
                        (Some((from, to)), Some(stems)) => {
                            let gains = std::array::from_fn(|k| from[k] + (to[k] - from[k]) * r);
                            let (u0, u1) = p.stem_upgrade;
                            let w = u0 + (u1 - u0) * r;
                            read(
                                &StemMix::blending(stems, audio, gains, gains, (w, w), 1.0),
                                pos,
                            )
                        }
                        _ => read(&joined(audio, &joins[i]), pos),
                    };
                    s = if p.shifted {
                        d.shifted[f]
                    } else {
                        source(d.pos)
                    };
                    if let (Some((from, to)), Some(stems)) = (p.stem_send, audio.stems()) {
                        send = if p.shifted {
                            d.stem_shifted[f]
                        } else {
                            let w = std::array::from_fn(|k| from[k] + (to[k] - from[k]) * r);
                            let (u0, u1) = p.stem_upgrade;
                            let u = u0 + (u1 - u0) * r;
                            read(&StemMix::solo(stems, audio, w, w, (u, u), 1.0), d.pos)
                        };
                    }
                    match p.shift_fade {
                        Some(ShiftFade::In(n)) if f < n => {
                            s = crossfade(source(d.pos), s, (f + 1) as f32 / n as f32);
                        }
                        // `shifted` holds what the stretcher would have played on.
                        Some(ShiftFade::Out(n)) if f < n => {
                            s = crossfade(d.shifted[f], s, (f + 1) as f32 / n as f32);
                        }
                        _ => {}
                    }
                    let step = v * p.ratio;
                    if let Some(dc) = &mut d.declick {
                        s = dc.mix(s, source(dc.pos));
                        dc.pos += step;
                        if dc.left == 0 {
                            d.declick = None;
                        }
                    }
                    if p.scrub {
                        s = soften_scrub(s, v.abs(), out_rate, &mut d.scrub_tone);
                    }
                    let before = d.pos;
                    d.pos += step;
                    if let Some((a, b)) = p.loop_frames {
                        let unwrapped = d.pos;
                        if v > 0.0 && d.pos >= b {
                            d.pos -= b - a;
                        } else if v < 0.0 && d.pos < a && before >= a {
                            d.pos += b - a;
                        }
                        // The loop wrapped: fade from the audio running on
                        // past its end (or before its start, in reverse).
                        if d.pos != unwrapped && !p.shifted {
                            d.declick = Some(Declick::new(unwrapped, out_rate));
                        }
                    }
                    let len = audio.len() as f64;
                    // Handing over, it plays on into the next track.
                    if d.pos >= len && d.hand_over.is_none() {
                        d.pos = len;
                        ended[i] = true;
                    } else if d.pos < 0.0 {
                        d.pos = 0.0;
                    }
                }
                if p.stem_fx {
                    let wet = d.stem_fx.process(send, p.stem_send.is_some() && p.moving);
                    s[0] += wet[0];
                    s[1] += wet[1];
                }
                if groove_on[i] {
                    s = groove_buf[i][f];
                }
                if live_on[i] > 0 {
                    s = self.inputs[live_on[i] - 1].raw[f];
                }
                let gains = std::array::from_fn(|k| p.eq_from[k] + (p.eq_to[k] - p.eq_from[k]) * r);
                let pre = lerp(p.pre);
                s = d.eq.process([s[0] * pre, s[1] * pre], gains);
                let filtered = d.filter.process(s);
                let q = lerp(p.quick);
                for c in 0..2 {
                    s[c] += (filtered[c] - s[c]) * q;
                }
                s = d.sculpt.process(s);
                deck_buf[i][f] = s;
                click_buf[i][f] = 0.0;
                if let Some(sr) = p.metronome {
                    let c = metronome(d, sr, p.moving, out_rate);
                    click_buf[i][f] = c;
                }
            }
        }
        for (i, buf) in deck_buf.iter_mut().enumerate() {
            self.fx.process_block(i, &mut buf[..frames]);
        }
        for f in 0..frames {
            let r = (f + 1) as f32 / frames as f32;
            let lerp = |(a, b): (f32, f32)| a + (b - a) * r;
            let mut main = [0.0f32; 2];
            let mut cue = [0.0f32; 2];
            for i in 0..NUM_DECKS {
                let p = &plans[i];
                let c = click_buf[i][f];
                let s = [deck_buf[i][f][0] + c, deck_buf[i][f][1] + c];
                for c in 0..2 {
                    deck_peak[i][c] = deck_peak[i][c].max(s[c].abs());
                }
                if p.pfl {
                    cue[0] += s[0];
                    cue[1] += s[1];
                }
                let g = lerp(p.vol) * lerp(p.xf) * duck_buf[i][f];
                main[0] += s[0] * g;
                main[1] += s[1] * g;
            }
            // The preview player is for the headphones only.
            let preview = self.preview.next();
            cue[0] += preview[0];
            cue[1] += preview[1];
            let samples = self.samplers.next();
            main[0] += samples[0];
            main[1] += samples[1];
            // The auxiliary input joins the music; the microphone is added
            // after the main mix's effects, which leave the voice alone, and
            // the music ducks under it.
            let mic = &self.inputs[MICROPHONE];
            let m = mic.buf[f];
            let gm = lerp(mic.level_ramp);
            let ducked = self.ducker.next(duck, mic.live && gm > 0.0, m);
            // The auxiliary inputs and the instrument play with the music.
            for input in [AUXILIARY, INSTRUMENT, AUXILIARY_2, AUXILIARY_3] {
                let inp = &self.inputs[input];
                let a = inp.buf[f];
                let ga = lerp(inp.level_ramp);
                for c in 0..2 {
                    main[c] += a[c] * ga;
                }
                if inp.pfl {
                    cue[0] += a[0];
                    cue[1] += a[1];
                }
            }
            for v in &mut main {
                *v *= ducked;
            }
            main_buf[f] = main;
            cue_buf[f] = cue;
        }
        self.fx.process_block(MAIN, &mut main_buf[..frames]);
        for f in 0..frames {
            let r = (f + 1) as f32 / frames as f32;
            let lerp = |(a, b): (f32, f32)| a + (b - a) * r;
            let mut main = main_buf[f];
            let mut cue = cue_buf[f];
            let mic = &self.inputs[MICROPHONE];
            let m = mic.buf[f];
            let gm = lerp(mic.level_ramp);
            for c in 0..2 {
                main[c] += m[c] * gm;
            }
            if mic.pfl {
                cue[0] += m[0];
                cue[1] += m[1];
            }
            if tone != 0 {
                let t = test_tone(self.tone_frame, out_rate);
                self.tone_frame += 1;
                let to = if tone == 1 { &mut main } else { &mut cue };
                to[0] += t;
                to[1] += t;
            }
            // The booth hears the main mix before the main level, at its own.
            let booth = main.map(|v| (v * booth_gain).clamp(-1.0, 1.0));
            let mg = lerp(master);
            let bal = balance_gains(lerp(balance));
            let mut hot = [0.0; 2];
            for c in 0..2 {
                hot[c] = main[c] * mg;
                master_clip[c] |= hot[c].abs() > 1.0;
            }
            // Limited rather than clipped when the mix runs hot.
            let limited = self.limiter.process(hot);
            for c in 0..2 {
                main[c] = limited[c] * bal[c];
                master_peak[c] = master_peak[c].max(main[c].abs());
            }
            let o = &mut out[f * channels..(f + 1) * channels];
            o[0] = main[0];
            if channels > 1 {
                o[1] = main[1];
            }
            if channels >= 4 {
                if head_split {
                    let mono = |v: [f32; 2]| (v[0] + v[1]) * 0.5;
                    o[2] = (mono(cue) * head_gain).clamp(-1.0, 1.0);
                    o[3] = (mono(main) * head_gain).clamp(-1.0, 1.0);
                } else {
                    for c in 0..2 {
                        o[2 + c] =
                            ((cue[c] * cue_mix + main[c] * main_mix) * head_gain).clamp(-1.0, 1.0);
                    }
                }
            }
            if channels >= 6 {
                o[4] = booth[0];
                o[5] = booth[1];
            }
            let used = match channels {
                0..=3 => 2,
                4 | 5 => 4,
                _ => 6,
            };
            for v in o.iter_mut().skip(used) {
                *v = 0.0;
            }
        }
        self.deck_buf = deck_buf;
        self.click_buf = click_buf;
        self.groove_buf = groove_buf;
        self.duck_buf = duck_buf;
        self.main_buf = main_buf;
        self.cue_buf = cue_buf;
        self.fx.finish();

        // Remember where the ramps ended, and publish positions and meters.
        self.clock += dt;
        // On for half a second, off for half a second, as Mixxx blinks
        // play and cue on controllers.
        for (i, ended) in ended.iter_mut().enumerate() {
            if let Some(j) = self.decks[i].hand_over {
                *ended |= self.hand_over(i, j);
            }
        }
        let blink = self.clock.rem_euclid(1.0) < 0.5;
        for (i, d) in self.decks.iter_mut().enumerate() {
            let p = &plans[i];
            d.eq_gains = p.eq_to;
            d.pregain = p.pre.1;
            d.quick = p.quick.1;
            d.volume = p.vol.1;
            self.xfade[i] = p.xf.1;
            d.velocity = if ended[i] { 0.0 } else { p.velocity.1 };
            d.expected_pos = (p.moving && !ended[i]).then_some(d.pos);
            if d.expected_pos.is_none() {
                d.declick = None;
            }
            if ended[i] {
                d.c.play.set_bool(false);
            }
            // Paused with a track: play blinks, as on Pioneer players.
            let paused = !d.c.play.get_bool() && d.track.is_some();
            d.c.play_indicator
                .set_bool(d.c.play.get_bool() || (paused && blink));
            d.peak_hold = if deck_peak[i].iter().any(|&p| p > 1.0) {
                PEAK_HOLD
            } else {
                (d.peak_hold - dt as f32).max(0.0)
            };
            d.c.peak.set_bool(d.peak_hold > 0.0);
            let Some(track) = d.track.clone() else {
                d.c.vu.set(0.0);
                d.c.vu_lr.iter().for_each(|v| v.set(0.0));
                d.c.cue_indicator.set(0.0);
                continue;
            };
            if let Some(slip) = d.slip.as_mut() {
                *slip = (*slip + p.slip_step).min(d.audio.as_ref().map_or(0.0, |a| a.len() as f64));
            }
            let ghost = d.slip.map_or(-1.0, |at| at / d.source_rate());
            if d.c.slip_position.get() != ghost {
                d.c.slip_position.set(ghost);
            }
            if p.moving {
                let s = d.seconds();
                d.c.playposition.set(s / track.duration);
                d.last_written_position = d.c.playposition.get();
            }
            self.visual.publish(i, d.seconds(), d.velocity, buffer_end);
            let at_cue = (d.seconds() - d.c.cue_point.get()).abs() < 0.01;
            // Lit while held or paused at the cue point; paused anywhere
            // else it blinks, since pressing it there sets a new cue point.
            let playing = d.c.play.get_bool();
            d.c.cue_indicator
                .set_bool((!playing && (at_cue || blink)) || d.c.cue_default.get_bool());
            let phase = track.beat_at(d.seconds()).rem_euclid(1.0);
            d.c.beat_active.set_bool(p.moving && phase < 0.15);
            for ((vu, peak), control) in d.vu.iter_mut().zip(deck_peak[i]).zip(&d.c.vu_lr) {
                *vu = meter(peak).max(*vu - VU_FALL * dt as f32);
                control.set(*vu as f64);
            }
            d.c.vu.set(d.vu[0].max(d.vu[1]) as f64);
        }
        self.master_gain = master_target;
        self.balance = balance_target;
        for c in 0..2 {
            self.master_vu[c] =
                meter(master_peak[c]).max(self.master_vu[c] - MASTER_VU_FALL * dt as f32);
            self.master_peak_hold[c] = if master_clip[c] {
                PEAK_HOLD
            } else {
                (self.master_peak_hold[c] - dt as f32).max(0.0)
            };
            self.master.peak[c + 1].set_bool(self.master_peak_hold[c] > 0.0);
        }
        self.samplers.publish();
        self.preview.publish();
        self.master.peak[0].set_bool(self.master_peak_hold.iter().any(|&h| h > 0.0));
        self.master.vu_l.set(self.master_vu[0] as f64);
        self.master.vu_r.set(self.master_vu[1] as f64);
        self.master
            .vu
            .set(self.master_vu[0].max(self.master_vu[1]) as f64);
        self.record.push(out, channels);
        self.meter.push(out, channels);
        self.broadcast.push(out, channels);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::beats::BeatGrid;
    use crate::loader::prepare;
    use crate::synth;
    use crate::track::Hotcue;
    use crate::AVAILABLE_EFFECTS;
    use crate::KEYBOARD_SEMITONES;

    const SR: f64 = 48_000.0;

    fn short_track(bpm: f64, seconds: f64) -> Arc<Track> {
        let mut t = (*synth::sample_library()[0]).clone();
        t.bpm = bpm;
        t.first_beat = 0.1;
        t.duration = seconds;
        Arc::new(prepare(&t).unwrap())
    }

    /// Runs the engine for `seconds`, returning the peak output level.
    fn run(engine: &mut Engine, seconds: f64) -> f32 {
        let mut buf = vec![0.0f32; 512 * 2];
        let mut peak = 0.0f32;
        for _ in 0..(seconds * SR / 512.0).ceil() as usize {
            engine.process(&mut buf, 2);
            peak = buf.iter().fold(peak, |p, v| p.max(v.abs()));
        }
        peak
    }

    /// Runs the engine for `seconds`, returning the output.
    fn record(engine: &mut Engine, seconds: f64) -> Vec<f32> {
        let mut out = Vec::new();
        let mut buf = vec![0.0f32; 512 * 2];
        for _ in 0..(seconds * SR / 512.0).ceil() as usize {
            engine.process(&mut buf, 2);
            out.extend_from_slice(&buf);
        }
        out
    }

    fn setup() -> (Arc<ControlRegistry>, Engine, EngineInput) {
        let reg = ControlRegistry::new();
        let (engine, input) = Engine::new(&reg, SR);
        // The tests measure levels at unity, not at the DJ's starting levels.
        for (key, value) in [("gain", 1.0), ("headGain", 1.0), ("headMix", 0.0)] {
            reg.get("[Master]", key).unwrap().set(value);
        }
        (reg, engine, input)
    }

    fn load(input: &mut EngineInput, deck: usize, track: Arc<Track>) {
        input.send(Command::Load { deck, track }).ok().unwrap();
    }

    /// A track of a steady sine tone in `key`.
    fn tone_track(hz: f64, key: &str) -> Arc<Track> {
        long_tone_track(hz, key, 4.0)
    }

    fn long_tone_track(hz: f64, key: &str, seconds: f64) -> Arc<Track> {
        let mut t = (*short_track(120.0, seconds)).clone();
        let frames = (0..(SR * seconds) as usize)
            .map(|i| {
                let v = (2.0 * std::f64::consts::PI * hz * i as f64 / SR).sin() as f32 * 0.5;
                [v, v]
            })
            .collect();
        t.audio = Some(Arc::new(Audio::from_frames(SR as u32, frames)));
        t.key = key.into();
        Arc::new(t)
    }

    /// Runs the engine in real time, as Link's clock does.
    fn run_live(engine: &mut Engine, seconds: f64) {
        let mut buf = vec![0.0f32; 512 * 2];
        let start = Instant::now();
        for n in 0..(seconds * SR / 512.0).ceil() as usize {
            let due = start + Duration::from_secs_f64(n as f64 * 512.0 / SR);
            std::thread::sleep(due.saturating_duration_since(Instant::now()));
            engine.process(&mut buf, 2);
        }
    }

    #[test]
    fn a_color_fx_for_each_deck() {
        let (reg, mut engine, _input) = setup();
        let c = |d: usize, k: &str| {
            reg.get(&format!("[QuickEffectRack1_[Channel{}]]", d + 1), k)
                .unwrap()
        };
        let presets = || {
            (0..NUM_DECKS)
                .map(|d| c(d, "loaded_chain_preset").get())
                .collect::<Vec<_>>()
        };
        run(&mut engine, 0.01);
        assert_eq!(presets(), vec![1.0; NUM_DECKS], "Filter to start with");
        // Picking one on a deck leaves the others alone.
        c(2, "loaded_chain_preset").set(3.0);
        run(&mut engine, 0.01);
        assert_eq!(presets(), vec![1.0, 1.0, 3.0, 1.0]);
        // Stepping on deck 3 wraps round past the last.
        for want in [4.0, 5.0, 1.0] {
            c(2, "next_chain_preset").set(1.0);
            c(2, "next_chain_preset").set(0.0);
            run(&mut engine, 0.01);
            assert_eq!(presets()[2], want);
        }
        c(1, "prev_chain_preset").set(1.0);
        c(1, "prev_chain_preset").set(0.0);
        run(&mut engine, 0.01);
        assert_eq!(presets(), vec![1.0, 5.0, 1.0, 1.0]);
    }

    #[test]
    fn ableton_link_follows_the_master_deck() {
        let (reg, mut engine, mut input) = setup();
        let link = engine.link_handle();
        link.enable(true);
        // Another app in the session is at 128 BPM.
        let mut state = rusty_link::SessionState::new();
        link.capture_app_session_state(&mut state);
        state.set_tempo(128.0, link.clock_micros());
        link.commit_app_session_state(&state);
        load(&mut input, 0, short_track(120.0, 8.0));
        load(&mut input, 1, short_track(124.0, 8.0));
        run(&mut engine, 0.05);
        let c = |g: &str, k: &str| reg.get(g, k).unwrap();
        let master = || c("[Link]", "master_deck").get();
        let session_tempo = |state: &mut rusty_link::SessionState| {
            link.capture_app_session_state(state);
            state.tempo()
        };
        assert_eq!(master(), 0.0, "no deck playing");
        c("[Channel1]", "play").set_bool(true);
        run_live(&mut engine, 0.5);
        assert_eq!(master(), 1.0);
        // The deck keeps its tempo; the session takes it.
        assert!((c("[Channel1]", "bpm").get() - 120.0).abs() < 0.01);
        let t = session_tempo(&mut state);
        assert!((t - 120.0).abs() < 0.01, "session tempo {t}");
        // On the deck's beat.
        let check_phase = |engine: &mut Engine, deck: usize| {
            let mut s = rusty_link::SessionState::new();
            link.capture_app_session_state(&mut s);
            let d = &engine.decks[deck];
            let track = d.track.clone().unwrap();
            // The next buffer starts where the deck is now.
            let heard = engine.link.heard_at + (512.0 / SR * 1e6) as i64;
            let e = phase_error(track.beat_at(d.seconds()), s.beat_at_time(heard, 4.0));
            assert!(e.abs() < 0.05, "phase error {e}");
        };
        check_phase(&mut engine, 0);
        // Moving the master's tempo moves the session's.
        let rate = c("[Channel1]", "rate");
        rate.set((126.0 / 120.0 - 1.0) / c("[Channel1]", "rateRange").get());
        run_live(&mut engine, 0.3);
        let t = session_tempo(&mut state);
        assert!((t - 126.0).abs() < 0.01, "session tempo {t}");
        check_phase(&mut engine, 0);
        // The second deck starting doesn't take over; the first stopping
        // hands the session to it.
        c("[Channel2]", "play").set_bool(true);
        run_live(&mut engine, 0.2);
        assert_eq!(master(), 1.0);
        c("[Channel1]", "play").set_bool(false);
        run_live(&mut engine, 0.3);
        assert_eq!(master(), 2.0);
        let t = session_tempo(&mut state);
        assert!((t - 124.0).abs() < 0.01, "session tempo {t}");
        check_phase(&mut engine, 1);
        link.enable(false);
    }

    #[test]
    fn plays_stems_with_their_own_volume_and_mute() {
        // The full mix is silent, so all sound comes from the stems: a tone
        // in the first and silence in the others.
        let mut t = (*short_track(120.0, 4.0)).clone();
        let n = (SR * 4.0) as usize;
        let audio = Audio::from_frames(SR as u32, vec![[0.0; 2]; n]);
        let frames: Vec<_> = (0..n)
            .map(|i| {
                let v = (std::f64::consts::TAU * 220.0 * i as f64 / SR).sin() as f32 * 0.5;
                [[v, v], [0.0; 2], [0.0; 2], [0.0; 2], [0.0; 2]]
            })
            .collect();
        let stems =
            crate::stems::Stems::from_frames(crate::stems::default_info(), SR as u32, &frames);
        assert!(audio.set_stems(stems));
        t.audio = Some(Arc::new(audio));
        let track = Arc::new(t);
        let level = |configure: &dyn Fn(&ControlRegistry)| {
            let out = render(&track, configure);
            let tail = &out[out.len() / 2..];
            (tail.iter().map(|v| v * v).sum::<f32>() / tail.len() as f32).sqrt()
        };
        let full = level(&|_| {});
        assert!(full > 0.2, "the stems play: {full}");
        let reg_stem = |reg: &ControlRegistry, key: &str| reg.get("[Channel1_Stem1]", key).unwrap();
        let muted = level(&|reg| reg_stem(reg, "mute").set_bool(true));
        assert!(muted < 1e-4, "a muted stem is silent: {muted}");
        let half = level(&|reg| reg_stem(reg, "volume").set(0.5));
        assert!(
            (half / full - 0.5).abs() < 0.02,
            "half volume: {}",
            half / full
        );
        // Through the time-stretcher too.
        let locked = level(&|reg| {
            reg.get("[Channel1]", "keylock").unwrap().set_bool(true);
            reg.get("[Channel1]", "rate").unwrap().set(0.5);
            reg_stem(reg, "volume").set(0.5);
        });
        assert!(
            (locked / full - 0.5).abs() < 0.1,
            "key lock: {}",
            locked / full
        );
        // The engine reports the stems.
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, track.clone());
        run(&mut engine, 0.01);
        assert_eq!(reg.get("[Channel1]", "stem_count").unwrap().get(), 4.0);
        assert_eq!(reg_stem(&reg, "color").get(), 0x2E4E75 as f64);
        assert_eq!(reg.get("[Channel2]", "stem_count").unwrap().get(), 0.0);
    }

    #[test]
    fn separated_stems_play_the_original_less_what_is_turned_down() {
        // The track is two tones; stem 1 holds the first, stem 2 the second.
        let mut t = (*short_track(120.0, 4.0)).clone();
        let n = (SR * 4.0) as usize;
        let tone =
            |hz: f64, i: usize| (std::f64::consts::TAU * hz * i as f64 / SR).sin() as f32 * 0.3;
        let audio = Audio::from_frames(
            SR as u32,
            (0..n)
                .map(|i| [tone(220.0, i) + tone(330.0, i); 2])
                .collect(),
        );
        let mut info = crate::stems::default_info();
        crate::stems::split_drums(&mut info);
        let stems = Arc::new(crate::stems::Stems::separated(
            info,
            SR as u32,
            n,
            crate::stems::Quality::Fast,
        ));
        // Only the first 0.9 s is separated (of the 1.5 s rendered).
        let frames = (0..(SR * 0.9) as usize)
            .map(|i| {
                let mut f = [[0i16; 2]; MAX_STEMS];
                f[0] = [crate::stems::to_i16(tone(220.0, i)); 2];
                f[1] = [crate::stems::to_i16(tone(330.0, i)); 2];
                f
            })
            .collect();
        stems.fill(0, frames);
        assert!(audio.set_stems(stems));
        t.audio = Some(Arc::new(audio));
        let track = Arc::new(t);
        let out = render(&track, |reg| {
            reg.get("[Channel1_Stem1]", "mute").unwrap().set_bool(true)
        });
        let rms = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
        let full = render(&track, |_| {});
        let quarter = out.len() / 4;
        let (first, second) = (&out[quarter..2 * quarter], &out[3 * quarter..]);
        let (full_first, full_second) = (&full[quarter..2 * quarter], &full[3 * quarter..]);
        // Muted where separated: one tone of two left.
        let ratio = rms(first) / rms(full_first);
        assert!((ratio - 0.71).abs() < 0.08, "one tone left: {ratio}");
        // Not separated yet: the original plays.
        let ratio = rms(second) / rms(full_second);
        assert!((ratio - 1.0).abs() < 0.02, "the original: {ratio}");
    }

    /// A track whose only sound is a 440 Hz tone in the vocal stem (stem
    /// 4), as an NI stem file (the stems play as their sum).
    fn vocal_tone_track(seconds: f64) -> Arc<Track> {
        let mut t = (*short_track(120.0, seconds)).clone();
        let n = (SR * seconds) as usize;
        let audio = Audio::from_frames(SR as u32, vec![[0.0; 2]; n]);
        let frames: Vec<_> = (0..n)
            .map(|i| {
                let v = (std::f64::consts::TAU * 440.0 * i as f64 / SR).sin() as f32 * 0.3;
                [[0.0; 2], [0.0; 2], [0.0; 2], [v, v], [0.0; 2]]
            })
            .collect();
        let stems =
            crate::stems::Stems::from_frames(crate::stems::default_info(), SR as u32, &frames);
        assert!(audio.set_stems(stems));
        t.audio = Some(Arc::new(audio));
        Arc::new(t)
    }

    /// Echoes the vocals out in one press (effect on and mute together)
    /// after half a second of play; returns the left channel from the press.
    fn echo_out_vocals(configure: impl Fn(&ControlRegistry), fx: bool) -> Vec<f32> {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, vocal_tone_track(12.0));
        run(&mut engine, 0.01);
        configure(&reg);
        run(&mut engine, 0.01);
        reg.get("[Channel1]", "play").unwrap().set_bool(true);
        record(&mut engine, 0.5);
        if fx {
            reg.get("[QuickEffectRack1_[Channel1_Stem4]]", "enabled")
                .unwrap()
                .set_bool(true);
        }
        reg.get("[Channel1_Stem4]", "mute").unwrap().set_bool(true);
        record(&mut engine, 3.0).into_iter().step_by(2).collect()
    }

    #[test]
    fn a_stem_echoes_out_when_muted_with_its_effect_on() {
        let rms = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
        let at = |s: f64| (s * SR) as usize;
        // Without the effect a mute is a mute.
        let dry = echo_out_vocals(|_| {}, false);
        assert!(rms(&dry[at(0.1)..at(0.5)]) < 1e-4, "muted");
        let wet = echo_out_vocals(|_| {}, true);
        // The vocal plays on for one echo (half a beat at 120 BPM) so the
        // echo has something to hold, then rings on and fades.
        assert!(rms(&wet[..at(0.2)]) > 0.15, "held until the echo has it");
        let tail = rms(&wet[at(0.4)..at(0.9)]);
        assert!(tail > 0.08, "the echo rings on: {tail}");
        let later = rms(&wet[at(2.5)..at(3.0)]);
        assert!(later < tail / 4.0, "and fades: {later} after {tail}");
        // Sample to sample: no step bigger than the tone's own.
        let step = wet.windows(2).fold(0f32, |m, w| m.max((w[1] - w[0]).abs()));
        assert!(step < 0.05, "a click: {step}");
        // Through the time-stretcher too.
        let locked = echo_out_vocals(
            |reg| {
                reg.get("[Channel1]", "keylock").unwrap().set_bool(true);
                reg.get("[Channel1]", "rate").unwrap().set(0.3);
            },
            true,
        );
        let tail = rms(&locked[at(0.4)..at(0.9)]);
        assert!(tail > 0.03, "key-locked echo: {tail}");
    }

    #[test]
    fn a_stem_effect_adds_to_the_stem_and_stops_when_rung_out() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, vocal_tone_track(14.0));
        run(&mut engine, 0.01);
        reg.get("[Channel1]", "play").unwrap().set_bool(true);
        let before = record(&mut engine, 0.5);
        let fx = reg
            .get("[QuickEffectRack1_[Channel1_Stem4]]", "enabled")
            .unwrap();
        fx.set_bool(true);
        // The first echo comes half a beat later: until then nothing changes.
        let on = record(&mut engine, 0.2);
        let reference = {
            let (reg, mut engine, mut input) = setup();
            load(&mut input, 0, vocal_tone_track(14.0));
            run(&mut engine, 0.01);
            reg.get("[Channel1]", "play").unwrap().set_bool(true);
            record(&mut engine, 0.5);
            record(&mut engine, 0.2)
        };
        assert_eq!(before.len() + on.len(), before.len() + reference.len());
        let diff = on
            .iter()
            .zip(&reference)
            .fold(0f32, |m, (a, b)| m.max((a - b).abs()));
        assert!(diff < 1e-4, "the dry stem is untouched: {diff}");
        record(&mut engine, 1.0);
        assert!(engine.decks[0].stem_fx.ringing());
        // Switched off, it rings out and the deck stops running it.
        fx.set_bool(false);
        record(&mut engine, 9.0);
        assert!(!engine.decks[0].stem_fx.ringing());
        assert!(!engine.decks[0].stem_shifter.is_active());
        // A load switches every stem's effect off.
        fx.set_bool(true);
        load(&mut input, 0, vocal_tone_track(2.0));
        run(&mut engine, 0.05);
        assert!(!fx.get_bool());
    }

    #[test]
    fn high_quality_stems_take_over_without_a_click() {
        // The stems made on load hold the whole tone in stem 1; the better
        // ones hold none of it. With stem 1 muted, the deck goes from
        // silence to the tone when they take over: it must fade, not jump.
        let mut t = (*short_track(120.0, 4.0)).clone();
        let n = (SR * 4.0) as usize;
        let tone = |i: usize| (std::f64::consts::TAU * 220.0 * i as f64 / SR).sin() as f32 * 0.3;
        let audio = Audio::from_frames(SR as u32, (0..n).map(|i| [tone(i); 2]).collect());
        let info = crate::stems::default_info();
        let layer = |stem: bool, quality| {
            let s = Arc::new(crate::stems::Stems::separated(
                info.clone(),
                SR as u32,
                n,
                quality,
            ));
            for (k, start) in (0..n).step_by(crate::stems::CHUNK).enumerate() {
                let frames = (start..(start + crate::stems::CHUNK).min(n))
                    .map(|i| {
                        let mut f = [[0i16; 2]; MAX_STEMS];
                        if stem {
                            f[0] = [crate::stems::to_i16(tone(i)); 2];
                        }
                        f
                    })
                    .collect();
                s.fill(k, frames);
            }
            s.finish();
            s
        };
        let fast = layer(true, crate::stems::Quality::Fast);
        assert!(audio.set_stems(fast.clone()));
        t.audio = Some(Arc::new(audio));
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, Arc::new(t));
        run(&mut engine, 0.01);
        reg.get("[Channel1_Stem1]", "mute").unwrap().set_bool(true);
        run(&mut engine, 0.01);
        reg.get("[Channel1]", "play").unwrap().set_bool(true);
        let before = record(&mut engine, 0.3);
        let quiet = before[before.len() / 2..]
            .iter()
            .fold(0f32, |m, v| m.max(v.abs()));
        assert!(quiet < 1e-3, "the muted stem is the whole tone: {quiet}");
        assert!(fast.set_upgrade(layer(false, crate::stems::Quality::High)));
        let after = record(&mut engine, 0.3);
        let loud = after[after.len() / 2..]
            .iter()
            .fold(0f32, |m, v| m.max(v.abs()));
        assert!(loud > 0.2, "the better stems play the tone: {loud}");
        // Left channel, sample to sample: no step bigger than the tone's own.
        let left: Vec<f32> = before.iter().chain(&after).step_by(2).copied().collect();
        let step = left
            .windows(2)
            .fold(0f32, |m, w| m.max((w[1] - w[0]).abs()));
        assert!(step < 0.02, "a click at the switch: {step}");
    }

    #[test]
    fn booth_follows_the_main_mix_at_its_own_level() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, tone_track(440.0, "A"));
        run(&mut engine, 0.05);
        reg.get("[Channel1]", "play").unwrap().set(1.0);
        reg.get("[Master]", "booth_gain").unwrap().set(0.5);
        let mut buf = vec![0.0f32; 512 * 6];
        let mut peak = [0.0f32; 6];
        for _ in 0..40 {
            engine.process(&mut buf, 6);
            for f in buf.as_chunks::<6>().0 {
                for (p, v) in peak.iter_mut().zip(f) {
                    *p = p.max(v.abs());
                }
            }
        }
        assert!(peak[0] > 0.1, "main {peak:?}");
        assert!(peak[4] > 0.05 && peak[4] < peak[0], "booth {peak:?}");
        reg.get("[Master]", "booth_gain").unwrap().set(0.0);
        engine.process(&mut buf, 6);
        engine.process(&mut buf, 6);
        assert!(buf
            .as_chunks::<6>()
            .0
            .iter()
            .all(|f| f[4] == 0.0 && f[5] == 0.0));
    }

    #[test]
    fn auto_gain_levels_tracks_and_glides_while_playing() {
        use crate::loudness::Loudness;
        let (reg, mut engine, mut input) = setup();
        let track = tone_track(440.0, "A");
        // As loud as a club master: auto gain should take 6 dB off.
        let audio = track.audio.as_ref().unwrap();
        audio.set_loudness(Loudness {
            lufs: -8.0,
            peak: 0.5,
        });
        load(&mut input, 0, track);
        run(&mut engine, 0.05);
        let gain = reg.get("[Channel1]", "replaygain").unwrap();
        assert!((gain.get() - 0.501).abs() < 0.01, "{}", gain.get());
        reg.get("[Channel1]", "play").unwrap().set(1.0);
        let peak = |out: &[f32]| {
            out[out.len() / 2..]
                .iter()
                .fold(0.0f32, |p, v| p.max(v.abs()))
        };
        let levelled = peak(&record(&mut engine, 0.5));

        // Switched off while playing, the gain glides back up at 6 dB/s
        // rather than jumping.
        reg.get("[ReplayGain]", "ReplayGainEnabled")
            .unwrap()
            .set(0.0);
        run(&mut engine, 0.5);
        let halfway = gain.get();
        assert!(halfway > 0.6 && halfway < 0.8, "{halfway}");
        let full = peak(&record(&mut engine, 1.0));
        assert_eq!(gain.get(), 1.0);
        assert!(
            (20.0 * (full / levelled).log10() - 6.0).abs() < 0.2,
            "{levelled} {full}"
        );
    }

    #[test]
    fn loading_a_track_resets_fx_eq_and_gain() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(120.0, 4.0));
        load(&mut input, 1, short_track(120.0, 4.0));
        run(&mut engine, 0.02);
        let c = |g: &str, k: &str| reg.get(g, k).unwrap();
        let deck1 = |k: &str| c("[Channel1]", k);
        let eq = |k: &str| c("[EqualizerRack1_[Channel1]_Effect1]", k);
        let slot = |u: usize, k: &str| c(&format!("[EffectRack1_EffectUnit{u}_Effect1]"), k);
        deck1("pregain").set(2.0);
        eq("parameter1").set(0.2);
        eq("button_parameter3").set_bool(true);
        c("[QuickEffectRack1_[Channel1]]", "super1").set(0.8);
        c("[Sculpt_[Channel1]]", "lpf").set(0.3);
        // Deck 1's unit (with Echo, which has a length in beats), and the
        // Beat FX on decks 1 and 2 with deck 2 playing.
        slot(1, "loaded_effect").set(1.0);
        run(&mut engine, 0.01);
        slot(1, "enabled").set_bool(true);
        slot(1, "beats").set(2.0);
        c("[EffectRack1_EffectUnit1]", "mix").set(0.4);
        slot(5, "enabled").set_bool(true);
        c("[Channel2]", "play").set_bool(true);
        run(&mut engine, 0.02);

        load(&mut input, 0, short_track(124.0, 4.0));
        run(&mut engine, 0.02);
        assert_eq!(deck1("pregain").get(), 1.0);
        assert_eq!(eq("parameter1").get(), 1.0);
        assert!(!eq("button_parameter3").get_bool());
        assert_eq!(c("[QuickEffectRack1_[Channel1]]", "super1").get(), 0.5);
        assert_eq!(c("[Sculpt_[Channel1]]", "lpf").get(), 1.0);
        // Off, with its beats and dry/wet kept.
        assert!(!slot(1, "enabled").get_bool());
        assert_eq!(slot(1, "beats").get(), 2.0);
        assert_eq!(c("[EffectRack1_EffectUnit1]", "mix").get(), 0.4);
        // The Beat FX still plays on deck 2.
        assert!(slot(5, "enabled").get_bool());

        // Switched off, loading leaves them as they are.
        for k in ["fx", "eq", "gain"] {
            c("[Controls]", &format!("auto_reset_{k}")).set_bool(false);
        }
        deck1("pregain").set(2.0);
        eq("parameter1").set(0.2);
        slot(1, "enabled").set_bool(true);
        load(&mut input, 0, short_track(126.0, 4.0));
        run(&mut engine, 0.02);
        assert_eq!(deck1("pregain").get(), 2.0);
        assert_eq!(eq("parameter1").get(), 0.2);
        assert!(slot(1, "enabled").get_bool());
    }

    #[test]
    fn loading_a_track_brings_back_its_stems_and_lets_go_of_pad_fx() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(120.0, 4.0));
        run(&mut engine, 0.02);
        let c = |g: &str, k: &str| reg.get(g, k).unwrap();
        let stem = |s: usize, k: &str| c(&format!("[Channel1_Stem{s}]"), k);
        let pad_slot = || c("[EffectRack1_EffectUnit6_Effect1]", "enabled");
        // Acapella (only the vocals, stem 4), a level turned down and a
        // Pad FX left on.
        for s in [1, 2, 3, 5] {
            stem(s, "mute").set_bool(true);
        }
        stem(4, "volume").set(0.3);
        pad_slot().set_bool(true);
        run(&mut engine, 0.02);

        load(&mut input, 0, short_track(124.0, 4.0));
        run(&mut engine, 0.02);
        for s in 1..=5 {
            assert!(!stem(s, "mute").get_bool(), "stem {s} plays");
        }
        assert_eq!(stem(4, "volume").get(), 1.0);
        assert!(!pad_slot().get_bool());

        // With auto reset off, the stems still play again and the Pad FX
        // still let go; only the levels stay.
        for k in ["fx", "eq", "gain"] {
            c("[Controls]", &format!("auto_reset_{k}")).set_bool(false);
        }
        stem(4, "mute").set_bool(true);
        stem(4, "volume").set(0.3);
        pad_slot().set_bool(true);
        load(&mut input, 0, short_track(126.0, 4.0));
        run(&mut engine, 0.02);
        assert!(!stem(4, "mute").get_bool());
        assert_eq!(stem(4, "volume").get(), 0.3);
        assert!(!pad_slot().get_bool());
    }

    #[test]
    fn loading_a_track_cues_its_deck_in_the_headphones() {
        let (reg, mut engine, mut input) = setup();
        let pfl = |d: usize| reg.get(&crate::deck_group(d), "pfl").unwrap();
        let cued = || {
            (0..NUM_DECKS)
                .map(|d| pfl(d).get_bool())
                .collect::<Vec<_>>()
        };
        pfl(0).set_bool(true);
        pfl(2).set_bool(true);
        load(&mut input, 1, short_track(124.0, 4.0));
        run(&mut engine, 0.02);
        assert_eq!(cued(), [false, true, false, false]);

        // Automix picks the decks itself, so the cue stays where it is.
        reg.get("[AutoDJ]", "enabled").unwrap().set_bool(true);
        load(&mut input, 0, short_track(124.0, 4.0));
        run(&mut engine, 0.02);
        assert_eq!(cued(), [false, true, false, false]);
        reg.get("[AutoDJ]", "enabled").unwrap().set_bool(false);

        // Switched off, loading leaves the cue alone.
        reg.get("[Controls]", "pfl_on_load")
            .unwrap()
            .set_bool(false);
        load(&mut input, 3, short_track(124.0, 4.0));
        run(&mut engine, 0.02);
        assert_eq!(cued(), [false, true, false, false]);
    }

    #[test]
    fn a_hot_mix_is_limited_to_the_ceiling() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(120.0, 4.0));
        run(&mut engine, 0.02);
        let c = |k: &str| reg.get("[Channel1]", k).unwrap();
        c("pregain").set(4.0);
        c("play").set_bool(true);
        let peak = run(&mut engine, 1.0);
        assert!(
            peak <= LIMIT_CEILING && peak > LIMIT_CEILING * 0.9,
            "{peak}"
        );
    }

    #[test]
    fn recording_or_on_air_limits_true_peaks_to_the_air_ceiling() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(120.0, 4.0));
        run(&mut engine, 0.02);
        let c = |k: &str| reg.get("[Channel1]", k).unwrap();
        c("pregain").set(4.0);
        c("play").set_bool(true);
        // Recording: -1 dBTP, measured between the samples too.
        engine.record_feed.as_ref().unwrap().set_active(true);
        let out = record(&mut engine, 1.0);
        let air = 10f32.powf(-1.0 / 20.0);
        assert!((engine.limiter.ceiling() - air).abs() < 1e-6);
        let mut tp = crate::loudness::TruePeak::new();
        let peak = out
            .as_chunks::<2>()
            .0
            .iter()
            .skip(4800)
            .map(|f| tp.push(*f))
            .fold(0.0f32, f32::max);
        assert!(peak <= air * 1.002 && peak > air * 0.9, "{peak}");
        // -2 dBTP when chosen.
        reg.get("[Master]", "air_ceiling").unwrap().set(-2.0);
        run(&mut engine, 0.05);
        assert!((engine.limiter.ceiling() - 10f32.powf(-2.0 / 20.0)).abs() < 1e-6);
        // Not recording: back to -0.5 dB.
        engine.record_feed.as_ref().unwrap().set_active(false);
        run(&mut engine, 0.05);
        assert_eq!(engine.limiter.ceiling(), LIMIT_CEILING);
        // On air counts the same as recording.
        let mut feed = engine.take_broadcast_feed().unwrap();
        feed.set_active(true);
        run(&mut engine, 0.05);
        assert!((engine.limiter.ceiling() - 10f32.powf(-2.0 / 20.0)).abs() < 1e-6);
        let mut got = Vec::new();
        feed.drain_into(&mut got);
        assert!(got.len() >= (0.04 * SR) as usize * 2, "{}", got.len());
        feed.set_active(false);
        run(&mut engine, 0.05);
        assert_eq!(engine.limiter.ceiling(), LIMIT_CEILING);
    }

    /// The strongest frequency between 300 and 700 Hz in the left channel.
    fn main_pitch(interleaved: &[f32]) -> f64 {
        let x: Vec<f32> = interleaved.iter().step_by(2).copied().collect();
        let x = &x[x.len() - 12_000..];
        (300..700)
            .map(|hz| {
                let w = 2.0 * std::f64::consts::PI * hz as f64 / SR;
                let (re, im) = x.iter().enumerate().fold((0.0, 0.0), |(re, im), (i, &v)| {
                    (
                        re + v as f64 * (w * i as f64).cos(),
                        im + v as f64 * (w * i as f64).sin(),
                    )
                });
                (hz as f64, re * re + im * im)
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap()
            .0
    }

    #[test]
    fn rate_dir_sets_which_way_the_tempo_fader_goes() {
        let (reg, mut engine, mut input) = setup();
        let c = |k: &str| reg.get("[Channel1]", k).unwrap();
        load(&mut input, 0, tone_track(440.0, "Am"));
        run(&mut engine, 0.05);
        // Pioneer's way: the fader at the top (+1) slows the deck by the range.
        c("rate_dir").set(-1.0);
        c("rate").set(1.0);
        c("play").set_bool(true);
        let from = c("playposition").get() * c("duration").get();
        run(&mut engine, 1.0);
        let moved = c("playposition").get() * c("duration").get() - from;
        assert!((moved - 0.92).abs() < 0.02, "{moved}");
        // "+" still speeds up: the fader moves down.
        c("rate").set(0.0);
        c("rate_perm_up").set_bool(true);
        run(&mut engine, 0.02);
        c("rate_perm_up").set_bool(false);
        assert!(c("rate").get() < 0.0);
    }

    #[test]
    fn pitch_reset_glides_pauses_and_jumps() {
        let (reg, mut engine, _input) = setup();
        let c = |k: &str| reg.get("[Channel1]", k).unwrap();
        // +4% at the default ±8%.
        c("rate").set(0.5);
        c("rate_reset").set_bool(true);
        run(&mut engine, 1.0);
        // Half a percent a second: about +3.5% now.
        assert!(
            (c("rate").get() * 0.08 - 0.035).abs() < 0.001,
            "{}",
            c("rate").get()
        );
        // Pressed again it stops where it is.
        c("rate_reset").set_bool(false);
        let held = c("rate").get();
        run(&mut engine, 0.5);
        assert_eq!(c("rate").get(), held);
        // Again: it carries on to 0% and turns itself off.
        c("rate_reset").set_bool(true);
        run(&mut engine, 8.0);
        assert_eq!(c("rate").get(), 0.0);
        assert!(!c("rate_reset").get_bool());
        // Slower than 0% glides up the same way.
        c("rate").set(-0.1);
        c("rate_reset").set_bool(true);
        run(&mut engine, 0.5);
        assert!(c("rate").get() > -0.1 && c("rate").get() < 0.0);
        // rate_set_zero is at once, and ends the glide.
        c("rate_set_zero").set_bool(true);
        c("rate_set_zero").set_bool(false);
        run(&mut engine, 0.02);
        assert_eq!(c("rate").get(), 0.0);
        assert!(!c("rate_reset").get_bool());
    }

    #[test]
    fn key_lock_and_key_shift() {
        let (reg, mut engine, mut input) = setup();
        let c = |k: &str| reg.get("[Channel1]", k).unwrap();
        load(&mut input, 0, tone_track(440.0, "Am"));
        load(&mut input, 1, tone_track(440.0, "Em"));
        run(&mut engine, 0.05);
        assert_eq!(
            Key::from_mixxx_number(c("file_key").get()),
            Key::parse("Am")
        );
        // Faster without key lock: pitch and key rise with the speed.
        c("rate").set(0.75); // +6% at the default ±8%
        c("play").set_bool(true);
        assert!((main_pitch(&record(&mut engine, 0.6)) - 466.0).abs() < 4.0);
        // Key lock brings the original pitch back at the same speed.
        c("keylock").set_bool(true);
        assert!((main_pitch(&record(&mut engine, 0.6)) - 440.0).abs() < 4.0);
        assert_eq!(Key::from_mixxx_number(c("key").get()), Key::parse("Am"));
        // pitch_up: one semitone higher (466 Hz) and the key follows.
        c("pitch_up").set_bool(true);
        run(&mut engine, 0.02);
        c("pitch_up").set_bool(false);
        assert_eq!(c("pitch").get(), 1.0);
        assert!((main_pitch(&record(&mut engine, 0.6)) - 466.0).abs() < 4.0);
        assert_eq!(Key::from_mixxx_number(c("key").get()), Key::parse("A#m"));
        // sync_key: Am to play with Em, by the smallest shift (Am is already
        // compatible as a neighbour, but matching wants the same number: Em
        // is 9A, Am 8A, so up 7 or down 5; ±6 limit gives -5).
        c("sync_key").set_bool(true);
        run(&mut engine, 0.02);
        c("sync_key").set_bool(false);
        assert_eq!(c("pitch").get(), -5.0);
        assert_eq!(Key::from_mixxx_number(c("key").get()), Key::parse("Em"));
        c("reset_key").set_bool(true);
        run(&mut engine, 0.02);
        assert_eq!(c("pitch").get(), 0.0);
        // pitch_adjust is the same control.
        reg.get("[Channel1]", "pitch_adjust").unwrap().set(2.0);
        assert_eq!(c("pitch").get(), 2.0);
    }

    #[test]
    fn matches_keys_on_load() {
        let (reg, mut engine, mut input) = setup();
        let c = |k: &str| reg.get("[Channel2]", k).unwrap();
        let range = reg.get("[Controls]", "key_match_on_load").unwrap();
        load(&mut input, 0, tone_track(440.0, "Am"));
        run(&mut engine, 0.05);
        reg.get("[Channel1]", "play").unwrap().set_bool(true);
        run(&mut engine, 0.05);
        // Off: loaded as it is.
        load(&mut input, 1, tone_track(440.0, "Bbm"));
        run(&mut engine, 0.02);
        assert_eq!((c("pitch").get(), c("keylock").get_bool()), (0.0, false));
        // Bbm (3A) one down is Am (8A), with key lock on.
        range.set(1.0);
        load(&mut input, 1, tone_track(440.0, "Bbm"));
        run(&mut engine, 0.02);
        assert_eq!((c("pitch").get(), c("keylock").get_bool()), (-1.0, true));
        assert_eq!(Key::from_mixxx_number(c("key").get()), Key::parse("Am"));
        // Already mixing: no shift.
        load(&mut input, 1, tone_track(440.0, "Em"));
        run(&mut engine, 0.02);
        assert_eq!(c("pitch").get(), 0.0);
        // F#m (11A) needs two down (Em, 9A): out of reach at ±1, not at ±2.
        c("keylock").set_bool(false);
        load(&mut input, 1, tone_track(440.0, "F#m"));
        run(&mut engine, 0.02);
        assert_eq!((c("pitch").get(), c("keylock").get_bool()), (0.0, false));
        range.set(2.0);
        load(&mut input, 1, tone_track(440.0, "F#m"));
        run(&mut engine, 0.02);
        assert_eq!((c("pitch").get(), c("keylock").get_bool()), (-2.0, true));
    }

    #[test]
    fn plays_audio_and_loops() {
        let (reg, mut engine, mut input) = setup();
        let track = short_track(124.0, 20.0);
        load(&mut input, 0, track.clone());
        assert_eq!(run(&mut engine, 0.1), 0.0, "silent while stopped");
        let ctl = |k: &str| reg.get("[Channel1]", k).unwrap();
        let start = ctl("playposition").get();
        ctl("play").set_bool(true);
        assert!(run(&mut engine, 1.0) > 0.1, "audible while playing");
        let now = ctl("playposition").get() * track.duration;
        let expected = start * track.duration + 1.0;
        assert!((now - expected).abs() < 0.02, "{now} vs {expected}");
        assert!(ctl("VuMeter").get() > 0.3);

        // A 1-beat pad loop keeps the position inside one beat.
        ctl("beatloop_1_activate").set_bool(true);
        run(&mut engine, 0.02);
        assert!(ctl("beatloop_1_enabled").get_bool());
        run(&mut engine, 1.5);
        let (a, b) = (
            ctl("loop_start_position").get(),
            ctl("loop_end_position").get(),
        );
        let now = ctl("playposition").get() * track.duration;
        assert!(now >= a - 0.01 && now <= b + 0.01, "{a} <= {now} <= {b}");
        assert!((b - a - track.beat_length()).abs() < 1e-6);

        // Fader down: silence.
        ctl("volume").set(0.0);
        run(&mut engine, 0.05);
        assert!(run(&mut engine, 0.2) < 1e-3);
    }

    #[test]
    fn loop_in_marks_a_loop_until_loop_out_closes_it() {
        let (reg, mut engine, mut input) = setup();
        let track = short_track(120.0, 20.0);
        load(&mut input, 0, track.clone());
        run(&mut engine, 0.02);
        let ctl = |k: &str| reg.get("[Channel1]", k).unwrap();
        ctl("play").set_bool(true);
        run(&mut engine, 0.6);
        let press = |k: &str, engine: &mut Engine| {
            ctl(k).set_bool(true);
            run(engine, 0.01);
            ctl(k).set_bool(false);
        };
        press("loop_in", &mut engine);
        assert!(ctl("loop_in_pending").get_bool());
        assert!(!ctl("loop_enabled").get_bool());
        // In on the nearest beat.
        let start = ctl("loop_start_position").get();
        let beats = (start - track.first_beat) / track.beat_length();
        assert!((beats - beats.round()).abs() < 1e-6, "{beats}");
        run(&mut engine, 1.0);
        press("loop_out", &mut engine);
        run(&mut engine, 0.01);
        assert!(!ctl("loop_in_pending").get_bool());
        assert!(ctl("loop_enabled").get_bool());
        let end = ctl("loop_end_position").get();
        assert!(
            (end - start - 2.0 * track.beat_length()).abs() < 1e-6,
            "{start}..{end}"
        );
        // The beat loop size is left alone.
        assert_eq!(ctl("beatloop_size").get(), 4.0);
    }

    #[test]
    fn sync_matches_tempo_and_hotcues_catch_fast_presses() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(124.0, 10.0));
        load(&mut input, 1, short_track(128.0, 10.0));
        run(&mut engine, 0.02);
        let hc = reg.get("[Channel1]", "hotcue_1_activate").unwrap();
        hc.set_bool(true);
        hc.set_bool(false);
        run(&mut engine, 0.02);
        assert!(reg
            .get("[Channel1]", "hotcue_1_enabled")
            .unwrap()
            .get_bool());
        assert_eq!(reg.get("[Channel1]", "hotcue_1_status").unwrap().get(), 1.0);
        assert_eq!(reg.get("[Channel1]", "hotcue_focus").unwrap().get(), 1.0);
        reg.get("[Channel2]", "sync_enabled")
            .unwrap()
            .set_bool(true);
        run(&mut engine, 0.02);
        let bpm2 = reg.get("[Channel2]", "bpm").unwrap().get();
        assert!((bpm2 - 124.0).abs() < 0.01, "{bpm2}");

        // Playing together, the beats line up.
        for d in ["[Channel1]", "[Channel2]"] {
            reg.get(d, "play").unwrap().set_bool(true);
        }
        run(&mut engine, 1.3);
        let phase = |deck: usize| {
            let d = &engine.decks[deck];
            let t = d.track.as_ref().unwrap();
            ((d.seconds() - t.first_beat) / t.beat_length()).rem_euclid(1.0)
        };
        let diff = (phase(0) - phase(1)).rem_euclid(1.0);
        assert!(diff.min(1.0 - diff) < 0.01, "{} vs {}", phase(0), phase(1));
    }

    #[test]
    fn sync_matches_double_tempo_when_closer() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(80.0, 10.0));
        load(&mut input, 1, short_track(153.0, 10.0));
        run(&mut engine, 0.02);
        reg.get("[Channel2]", "sync_enabled")
            .unwrap()
            .set_bool(true);
        run(&mut engine, 0.02);
        let bpm2 = reg.get("[Channel2]", "bpm").unwrap().get();
        assert!((bpm2 - 160.0).abs() < 0.01, "{bpm2}");
    }

    /// How far apart the two decks' beats are, in beats (0 to 0.5).
    fn phase_gap(engine: &Engine) -> f64 {
        let beat = |i: usize| {
            let d = &engine.decks[i];
            d.track.as_ref().unwrap().beat_at(d.seconds())
        };
        phase_error(beat(0), beat(1)).abs()
    }

    fn press(reg: &ControlRegistry, group: &str, key: &str) {
        let c = reg.get(group, key).unwrap();
        c.set_bool(true);
        c.set_bool(false);
    }

    #[test]
    fn a_deck_synced_while_stopped_starts_in_phase() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(124.0, 30.0));
        load(&mut input, 1, short_track(128.0, 30.0));
        run(&mut engine, 0.02);
        let c = |d: &str, k: &str| reg.get(d, k).unwrap();
        c("[Channel1]", "play").set_bool(true);
        run(&mut engine, 1.0);
        c("[Channel2]", "sync_enabled").set_bool(true);
        run(&mut engine, 0.79);
        c("[Channel2]", "play").set_bool(true);
        run(&mut engine, 0.05);
        assert!(phase_gap(&engine) < 0.01, "{}", phase_gap(&engine));
        assert!((c("[Channel2]", "bpm").get() - 124.0).abs() < 0.01);
    }

    #[test]
    fn play_starts_an_unsynced_deck_where_it_was_paused() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(124.0, 30.0));
        load(&mut input, 1, short_track(124.0, 30.0));
        run(&mut engine, 0.02);
        let c = |d: &str, k: &str| reg.get(d, k).unwrap();
        c("[Channel1]", "play").set_bool(true);
        run(&mut engine, 1.13);
        // Paused off the beat, with quantize on (as by default) at any size.
        for size in [1.0, 4.0] {
            c("[Channel2]", "quantize_beats").set(size);
            c("[Channel2]", "playposition").set(0.4);
            run(&mut engine, 0.02);
            let paused = engine.decks[1].seconds();
            c("[Channel2]", "play").set_bool(true);
            run(&mut engine, 0.01);
            let moved = engine.decks[1].seconds() - paused;
            // Only as far as it played in that buffer.
            assert!((0.0..0.03).contains(&moved), "size {size}: moved {moved}");
            c("[Channel2]", "play").set_bool(false);
            run(&mut engine, 0.02);
        }
    }

    #[test]
    fn a_freshly_loaded_deck_starts_at_the_start() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(124.0, 30.0));
        run(&mut engine, 0.02);
        reg.get("[Channel1]", "play").unwrap().set_bool(true);
        run(&mut engine, 2.37);
        load(&mut input, 1, short_track(124.0, 30.0));
        run(&mut engine, 0.02);
        let parked = engine.decks[1].seconds();
        reg.get("[Channel2]", "quantize_beats").unwrap().set(4.0);
        reg.get("[Channel2]", "play").unwrap().set_bool(true);
        run(&mut engine, 0.01);
        let moved = engine.decks[1].seconds() - parked;
        assert!((0.0..0.03).contains(&moved), "moved {moved}");
    }

    #[test]
    fn a_synced_deck_starts_in_phase_within_the_beat() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(124.0, 30.0));
        load(&mut input, 1, short_track(124.0, 30.0));
        run(&mut engine, 0.02);
        let c = |d: &str, k: &str| reg.get(d, k).unwrap();
        c("[Channel1]", "play").set_bool(true);
        c("[Channel2]", "sync_enabled").set_bool(true);
        // A bar, which used to move a start by up to two beats.
        c("[Channel2]", "quantize_beats").set(4.0);
        run(&mut engine, 2.6);
        let track = engine.decks[1].track.clone().unwrap();
        // Two and a third beats off the other deck within the bar.
        let theirs = track.beat_at(engine.decks[0].seconds() + 0.05);
        let paused = track.seconds_at(theirs + 2.3 + 8.0);
        c("[Channel2]", "playposition").set(paused / track.duration);
        run(&mut engine, 0.02);
        let paused = engine.decks[1].seconds();
        c("[Channel2]", "play").set_bool(true);
        run(&mut engine, 0.01);
        assert!(phase_gap(&engine) < 0.01, "{}", phase_gap(&engine));
        let beats = (track.beat_at(engine.decks[1].seconds()) - track.beat_at(paused)).abs();
        assert!(beats <= 0.55, "moved {beats} beats");
    }

    /// How far apart the two decks are within `step` beats, 0 when lined up.
    fn step_gap(engine: &Engine, step: f64) -> f64 {
        let beat = |i: usize| {
            let d = &engine.decks[i];
            d.track.as_ref().unwrap().beat_at(d.seconds())
        };
        step * phase_error(beat(0) / step, beat(1) / step).abs()
    }

    #[test]
    fn quantize_size_sets_how_a_jump_lines_up() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(124.0, 30.0));
        load(&mut input, 1, short_track(124.0, 30.0));
        run(&mut engine, 0.02);
        let c = |d: &str, k: &str| reg.get(d, k).unwrap();
        c("[Channel1]", "play").set_bool(true);
        c("[Channel2]", "play").set_bool(true);
        // A bar: a click on the overview lands on the same beat of the bar.
        c("[Channel2]", "quantize_beats").set(4.0);
        run(&mut engine, 2.6);
        c("[Channel2]", "playposition").set(0.37);
        run(&mut engine, 0.02);
        assert!(step_gap(&engine, 4.0) < 0.01, "{}", step_gap(&engine, 4.0));

        // Half a beat: in step on the half beat, not on the beat.
        c("[Channel2]", "quantize_beats").set(0.5);
        c("[Channel2]", "playposition").set(0.3);
        run(&mut engine, 0.02);
        assert!(step_gap(&engine, 0.5) < 0.01, "{}", step_gap(&engine, 0.5));
    }

    #[test]
    fn a_hot_cue_is_set_on_the_quantize_step() {
        let (reg, mut engine, mut input) = setup();
        let track = short_track(120.0, 30.0);
        load(&mut input, 0, track.clone());
        run(&mut engine, 0.02);
        let c = |k: &str| reg.get("[Channel1]", k).unwrap();
        let beat_at = |k: &str| track.beat_at(c(k).get());
        // A quarter beat in from beat 2 snaps to the quarter.
        c("quantize_beats").set(0.25);
        c("playposition").set(track.seconds_at(2.27) / track.duration);
        run(&mut engine, 0.02);
        press(&reg, "[Channel1]", "hotcue_1_activate");
        run(&mut engine, 0.02);
        assert!((beat_at("hotcue_1_position") - 2.25).abs() < 1e-3);
        // Quantize off: where it was pressed.
        c("quantize").set_bool(false);
        press(&reg, "[Channel1]", "hotcue_2_activate");
        run(&mut engine, 0.02);
        assert!((beat_at("hotcue_2_position") - 2.27).abs() < 0.02);
    }

    /// A controller's delete is pressed and let go between two blocks; it
    /// still deletes.
    #[test]
    fn a_quick_hot_cue_delete_counts() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(120.0, 30.0));
        run(&mut engine, 0.02);
        let c = |k: &str| reg.get("[Channel1]", k).unwrap();
        press(&reg, "[Channel1]", "hotcue_1_activate");
        run(&mut engine, 0.02);
        assert!(c("hotcue_1_enabled").get_bool());
        press(&reg, "[Channel1]", "hotcue_1_clear");
        run(&mut engine, 0.02);
        assert!(!c("hotcue_1_enabled").get_bool());
    }

    #[test]
    fn a_pinned_leader_stays_leader_until_unpinned() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(124.0, 30.0));
        load(&mut input, 1, short_track(128.0, 30.0));
        run(&mut engine, 0.02);
        let c = |d: &str, k: &str| reg.get(d, k).unwrap();
        c("[Channel1]", "play").set_bool(true);
        run(&mut engine, 0.2);
        for d in ["[Channel1]", "[Channel2]"] {
            c(d, "play").set_bool(true);
            c(d, "sync_enabled").set_bool(true);
        }
        run(&mut engine, 0.2);
        // Deck 1 started first, so it leads.
        assert_eq!(c("[Channel1]", "sync_leader").get(), 1.0);
        assert_eq!(c("[Channel2]", "sync_leader").get(), 0.0);

        c("[Channel2]", "sync_leader").set(2.0);
        run(&mut engine, 0.2);
        assert_eq!(engine.sync_leader, Some(1));
        assert_eq!(c("[Channel1]", "sync_leader").get(), 0.0);
        // Deck 1 now follows, at the tempo they were both playing.
        assert!((c("[Channel1]", "bpm").get() - c("[Channel2]", "bpm").get()).abs() < 0.01);
        assert!(engine.decks[0].following && !engine.decks[1].following);
        // Still leading when it stops.
        c("[Channel2]", "play").set_bool(false);
        run(&mut engine, 0.2);
        assert_eq!(engine.sync_leader, Some(1));
        assert_eq!(c("[Channel2]", "sync_leader").get(), 2.0);

        c("[Channel2]", "sync_leader").set(0.0);
        run(&mut engine, 0.2);
        assert_eq!(engine.sync_leader, Some(0));
        assert_eq!(c("[Channel1]", "sync_leader").get(), 1.0);
    }

    #[test]
    fn a_hot_cue_on_a_synced_deck_lands_in_phase() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(124.0, 30.0));
        load(&mut input, 1, short_track(128.0, 30.0));
        run(&mut engine, 0.02);
        for d in ["[Channel1]", "[Channel2]"] {
            reg.get(d, "play").unwrap().set_bool(true);
        }
        reg.get("[Channel2]", "sync_enabled")
            .unwrap()
            .set_bool(true);
        run(&mut engine, 0.5);
        reg.get("[Channel2]", "quantize").unwrap().set_bool(false);
        press(&reg, "[Channel2]", "hotcue_3_activate");
        run(&mut engine, 1.33);
        press(&reg, "[Channel2]", "hotcue_3_activate");
        run(&mut engine, 0.05);
        assert!(phase_gap(&engine) < 0.01, "{}", phase_gap(&engine));
    }

    #[test]
    fn either_fader_moves_both_synced_decks() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(124.0, 30.0));
        load(&mut input, 1, short_track(128.0, 30.0));
        run(&mut engine, 0.02);
        let c = |d: &str, k: &str| reg.get(d, k).unwrap();
        c("[Channel1]", "play").set_bool(true);
        run(&mut engine, 0.1);
        c("[Channel2]", "play").set_bool(true);
        for d in ["[Channel1]", "[Channel2]"] {
            c(d, "sync_enabled").set_bool(true);
        }
        run(&mut engine, 0.1);
        let bpm = |d: &str| c(d, "bpm").get();
        assert!(
            (bpm("[Channel2]") - 124.0).abs() < 0.01,
            "{}",
            bpm("[Channel2]")
        );
        // Deck 1 leads; its fader moves both.
        c("[Channel1]", "rate").set(0.5);
        run(&mut engine, 0.1);
        assert!(
            (bpm("[Channel1]") - 128.96).abs() < 0.01,
            "{}",
            bpm("[Channel1]")
        );
        assert!(
            (bpm("[Channel2]") - 128.96).abs() < 0.01,
            "{}",
            bpm("[Channel2]")
        );
        // So does the follower's.
        c("[Channel2]", "rate").set(0.0);
        run(&mut engine, 0.1);
        assert!(
            (bpm("[Channel1]") - 128.0).abs() < 0.01,
            "{}",
            bpm("[Channel1]")
        );
        assert!(
            (bpm("[Channel2]") - 128.0).abs() < 0.01,
            "{}",
            bpm("[Channel2]")
        );
    }

    #[test]
    fn sync_goes_past_the_fader_range_and_halves() {
        for (leader, own, want) in [
            (120.0, 132.0, 120.0),
            (140.0, 70.0, 70.0),
            (80.0, 113.0, 80.0),
        ] {
            let (reg, mut engine, mut input) = setup();
            load(&mut input, 0, short_track(leader, 10.0));
            load(&mut input, 1, short_track(own, 10.0));
            run(&mut engine, 0.02);
            reg.get("[Channel2]", "sync_enabled")
                .unwrap()
                .set_bool(true);
            run(&mut engine, 0.05);
            let bpm = reg.get("[Channel2]", "bpm").unwrap().get();
            assert!((bpm - want).abs() < 0.01, "{leader} vs {own}: {bpm}");
        }
    }

    #[test]
    fn sync_catches_up_drift_without_jumping() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(124.0, 30.0));
        load(&mut input, 1, short_track(128.0, 30.0));
        run(&mut engine, 0.02);
        for d in ["[Channel1]", "[Channel2]"] {
            reg.get(d, "play").unwrap().set_bool(true);
        }
        reg.get("[Channel2]", "sync_enabled")
            .unwrap()
            .set_bool(true);
        run(&mut engine, 0.5);
        assert!(phase_gap(&engine) < 0.01);
        // Something knocks deck 2 a fifth of a beat late.
        let beat = 60.0 / 128.0;
        let d = &mut engine.decks[1];
        d.pos -= 0.2 * beat * d.source_rate();
        assert!(phase_gap(&engine) > 0.19);
        let mut last = engine.decks[1].seconds();
        for _ in 0..(10.0 * SR / 512.0) as usize {
            run(&mut engine, 512.0 / SR);
            let now = engine.decks[1].seconds();
            let step = (now - last) / (512.0 / SR);
            assert!(step > 0.9 && step < 1.0 + SYNC_MAX_BEND, "{step}");
            last = now;
        }
        assert!(phase_gap(&engine) < 0.01, "{}", phase_gap(&engine));
    }

    #[test]
    fn a_nudge_on_a_synced_deck_is_kept() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(124.0, 30.0));
        load(&mut input, 1, short_track(128.0, 30.0));
        run(&mut engine, 0.02);
        for d in ["[Channel1]", "[Channel2]"] {
            reg.get(d, "play").unwrap().set_bool(true);
        }
        reg.get("[Channel2]", "sync_enabled")
            .unwrap()
            .set_bool(true);
        run(&mut engine, 0.5);
        let bend = reg.get("[Channel2]", "rate_temp_up").unwrap();
        bend.set_bool(true);
        run(&mut engine, 1.0);
        bend.set_bool(false);
        run(&mut engine, 0.05);
        let nudged = phase_gap(&engine);
        assert!(nudged > 0.05, "{nudged}");
        run(&mut engine, 5.0);
        assert!(
            (phase_gap(&engine) - nudged).abs() < 0.01,
            "{} vs {nudged}",
            phase_gap(&engine)
        );
    }

    #[test]
    fn an_empty_hot_cue_saves_the_playing_loop() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(120.0, 10.0));
        run(&mut engine, 0.02);
        let c = |k: &str| reg.get("[Channel1]", k).unwrap();
        let press = |k: &str| {
            c(k).set_bool(true);
            c(k).set_bool(false);
        };
        c("play").set_bool(true);
        run(&mut engine, 0.5);
        press("beatloop_4_activate");
        run(&mut engine, 0.02);
        let (start, end) = (engine.decks[0].loop_start, engine.decks[0].loop_end);
        assert!(c("loop_enabled").get_bool() && end > start);

        press("hotcue_2_activate");
        run(&mut engine, 0.02);
        assert_eq!(c("hotcue_2_type").get(), HOTCUE_LOOP);
        assert_eq!(c("hotcue_2_position").get(), start);
        assert_eq!(c("hotcue_2_endposition").get(), end);
        // The loop goes on playing.
        assert!(c("loop_enabled").get_bool());

        // Later, the saved loop brings the loop back from elsewhere.
        press("reloop_toggle");
        run(&mut engine, 0.02);
        assert!(!c("loop_enabled").get_bool());
        press("hotcue_2_activate");
        run(&mut engine, 0.02);
        let d = &engine.decks[0];
        assert!(c("loop_enabled").get_bool());
        assert_eq!((d.loop_start, d.loop_end), (start, end));
        assert!(d.seconds() >= start && d.seconds() < end);
        // Pressing it again while it plays turns it off.
        press("hotcue_2_activate");
        run(&mut engine, 0.02);
        assert!(!c("loop_enabled").get_bool());

        // A plain cue set with no loop playing stays a cue.
        press("hotcue_3_activate");
        run(&mut engine, 0.02);
        assert_eq!(c("hotcue_3_type").get(), HOTCUE_CUE);
        assert_eq!(c("hotcue_3_endposition").get(), -1.0);

        // Saved loops come back with the track.
        let mut t = (*short_track(120.0, 10.0)).clone();
        t.info.hotcues = vec![Hotcue {
            number: 4,
            seconds: 2.0,
            name: String::new(),
            end: Some(4.0),
            color: None,
        }];
        load(&mut input, 0, Arc::new(t));
        run(&mut engine, 0.02);
        assert_eq!(c("hotcue_2_type").get(), 0.0);
        assert_eq!(c("hotcue_5_type").get(), HOTCUE_LOOP);
        assert_eq!(c("hotcue_5_endposition").get(), 4.0);
        press("hotcue_5_activate");
        run(&mut engine, 0.02);
        assert!(c("loop_enabled").get_bool());
    }

    #[test]
    fn loops_jumps_and_sync_follow_a_dynamic_grid() {
        use crate::beats::{BeatGrid, GridMarker};
        let (reg, mut engine, mut input) = setup();
        // 120 BPM for 8 beats, then 140 BPM.
        let mut t = (*short_track(120.0, 12.0)).clone();
        t.set_grid(BeatGrid::new(vec![
            GridMarker {
                seconds: 0.1,
                bpm: 120.0,
            },
            GridMarker {
                seconds: 4.1,
                bpm: 140.0,
            },
        ]));
        let dynamic = Arc::new(t);
        load(&mut input, 0, dynamic.clone());
        load(&mut input, 1, short_track(130.0, 12.0));
        run(&mut engine, 0.02);
        let ctl = |d: &str, k: &str| reg.get(d, k).unwrap();

        // A 4-beat jump from beat 6 lands two beats into the 140 section.
        ctl("[Channel1]", "playposition").set(dynamic.seconds_at(6.0) / dynamic.duration);
        run(&mut engine, 0.02);
        ctl("[Channel1]", "beatjump_4_forward").set_bool(true);
        run(&mut engine, 0.02);
        let now = ctl("[Channel1]", "playposition").get() * dynamic.duration;
        assert!((now - (4.1 + 2.0 * 60.0 / 140.0)).abs() < 1e-3, "{now}");
        assert!((ctl("[Channel1]", "bpm").get() - 140.0).abs() < 1e-6);

        // A 2-beat loop there is two beats at 140.
        ctl("[Channel1]", "beatloop_2_activate").set_bool(true);
        run(&mut engine, 0.02);
        let (a, b) = (
            ctl("[Channel1]", "loop_start_position").get(),
            ctl("[Channel1]", "loop_end_position").get(),
        );
        assert!((b - a - 2.0 * 60.0 / 140.0).abs() < 1e-6, "{a}..{b}");
        ctl("[Channel1]", "beatloop_2_activate").set_bool(false);
        ctl("[Channel1]", "reloop_toggle").set_bool(true);
        run(&mut engine, 0.02);

        // Deck 2 synced to it follows the tempo where deck 1 is.
        ctl("[Channel2]", "sync_enabled").set_bool(true);
        run(&mut engine, 0.02);
        assert!((ctl("[Channel2]", "bpm").get() - 140.0).abs() < 0.01);
        ctl("[Channel1]", "playposition").set(1.0 / dynamic.duration);
        run(&mut engine, 0.04);
        assert!((ctl("[Channel2]", "bpm").get() - 120.0).abs() < 0.01);

        // A grid edit goes back to the loader with where it was pressed.
        ctl("[Channel1]", "beats_section_add").set_bool(true);
        run(&mut engine, 0.02);
        let edits: Vec<_> = input
            .collect_garbage()
            .into_iter()
            .filter_map(|r| match r {
                Returned::GridEdit {
                    deck,
                    edit,
                    seconds,
                    ..
                } => Some((deck, edit, seconds)),
                _ => None,
            })
            .collect();
        assert_eq!(edits.len(), 1);
        assert_eq!((edits[0].0, edits[0].1), (0, GridEdit::AddSection));
        assert!((edits[0].2 - 1.0).abs() < 0.05);
    }

    #[test]
    fn deck_bpm_tap_sets_the_tracks_tempo_and_reloop_exit_reloops() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(120.0, 20.0));
        run(&mut engine, 0.02);
        let ctl = |k: &str| reg.get("[Channel1]", k).unwrap();
        // Playing 25% fast: taps half a second apart are 120 BPM as heard,
        // 96 in the track.
        ctl("rateRange").set(0.5);
        ctl("rate").set(0.5);
        for _ in 0..4 {
            ctl("bpm_tap").set_bool(true);
            run(&mut engine, 0.02);
            ctl("bpm_tap").set_bool(false);
            run(&mut engine, 0.48);
        }
        let taps: Vec<_> = input
            .collect_garbage()
            .into_iter()
            .filter_map(|r| match r {
                Returned::GridEdit {
                    edit: GridEdit::SetTempo(bpm),
                    ..
                } => Some(bpm),
                _ => None,
            })
            .collect();
        // The first tap only starts the count.
        assert_eq!(taps.len(), 3, "{taps:?}");
        for bpm in taps {
            assert!((bpm - 96.0).abs() < 1.0, "{bpm}");
        }

        // reloop_exit is Mixxx's old name for reloop_toggle.
        ctl("beatloop_2_activate").set_bool(true);
        run(&mut engine, 0.02);
        ctl("beatloop_2_activate").set_bool(false);
        assert!(ctl("loop_enabled").get_bool());
        reg.get("[Channel1]", "reloop_exit").unwrap().set_bool(true);
        run(&mut engine, 0.02);
        assert!(!ctl("loop_enabled").get_bool());
    }

    #[test]
    fn stops_at_track_end() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(120.0, 2.0));
        run(&mut engine, 0.01);
        reg.get("[Channel1]", "play").unwrap().set_bool(true);
        run(&mut engine, 2.5);
        assert!(!reg.get("[Channel1]", "play").unwrap().get_bool());
        assert!((reg.get("[Channel1]", "playposition").unwrap().get() - 1.0).abs() < 1e-6);
        input.collect_garbage();
    }

    /// Plays deck 1 for a while with `configure` applied, returning the output.
    fn render(track: &Arc<Track>, configure: impl Fn(&ControlRegistry)) -> Vec<f32> {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, track.clone());
        // Loading resets the deck's effects and EQ, so configure after it.
        run(&mut engine, 0.01);
        configure(&reg);
        // Let gain ramps settle before the music starts, so a bypassed
        // effect leaves the output bit-identical.
        run(&mut engine, 0.01);
        reg.get("[Channel1]", "play").unwrap().set_bool(true);
        record(&mut engine, 1.5)
    }

    #[test]
    fn the_metronome_clicks_on_each_beat_of_the_grid() {
        let mut t = (*short_track(120.0, 4.0)).clone();
        t.set_grid(BeatGrid::constant(120.0, 0.1));
        let track = Arc::new(t);
        let dry = render(&track, |_| {});
        let wet = render(&track, |reg| {
            reg.get("[Channel1]", "metronome").unwrap().set_bool(true);
        });
        // Where the click starts after a quiet spell, in seconds.
        let mut clicks = Vec::new();
        let mut quiet = usize::MAX;
        for (f, (w, d)) in wet.chunks(2).zip(dry.chunks(2)).enumerate() {
            if (w[0] - d[0]).abs() > 0.05 {
                if quiet > (0.1 * SR) as usize {
                    clicks.push(f as f64 / SR);
                }
                quiet = 0;
            } else {
                quiet = quiet.saturating_add(1);
            }
        }
        // The deck starts at the first beat, 0.1 s into the track.
        assert_eq!(clicks.len(), 4, "{clicks:?}");
        for (k, at) in clicks.iter().enumerate() {
            let beat = 0.5 * k as f64;
            assert!(
                (at - beat).abs() < 0.015,
                "click {k} at {at}, beat at {beat}"
            );
        }
    }

    fn rms_difference(a: &[f32], b: &[f32]) -> f32 {
        let e: f32 = a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum();
        (e / a.len() as f32).sqrt()
    }

    #[test]
    fn every_effect_changes_the_deck_and_bypasses_when_off() {
        let track = short_track(124.0, 4.0);
        let dry = render(&track, |_| {});
        for (k, name) in AVAILABLE_EFFECTS.iter().enumerate() {
            let fx = |enabled: bool, unit_on: bool| {
                move |reg: &ControlRegistry| {
                    let slot =
                        |key: &str| reg.get("[EffectRack1_EffectUnit1_Effect1]", key).unwrap();
                    slot("loaded_effect").set((k + 1) as f64);
                    slot("meta").set(0.8);
                    slot("enabled").set_bool(enabled);
                    reg.get("[EffectRack1_EffectUnit1]", "enabled")
                        .unwrap()
                        .set_bool(unit_on);
                }
            };
            let wet = render(&track, fx(true, true));
            let d = rms_difference(&dry, &wet);
            println!("{name}: rms difference {d:.3}");
            assert!(d > 0.01, "{name} changes the sound: {d}");
            assert!(
                wet.iter().all(|v| v.is_finite() && v.abs() <= 1.0),
                "{name}"
            );
            assert_eq!(rms_difference(&dry, &render(&track, fx(false, true))), 0.0);
            assert_eq!(rms_difference(&dry, &render(&track, fx(true, false))), 0.0);
        }
    }

    #[test]
    fn effects_follow_routing_to_main_and_mix() {
        let track = short_track(124.0, 4.0);
        let dry = render(&track, |_| {});
        let unit = "[EffectRack1_EffectUnit3]";
        let on_main = |mix: f64| {
            move |reg: &ControlRegistry| {
                reg.get(unit, "group_[Master]_enable")
                    .unwrap()
                    .set_bool(true);
                reg.get(unit, "mix").unwrap().set(mix);
                // Unit 3 slot 3 holds the Bitcrusher; meta starts at 0, which
                // leaves it clean, so turn it up.
                let slot = "[EffectRack1_EffectUnit3_Effect3]";
                reg.get(slot, "meta").unwrap().set(0.5);
                reg.get(slot, "enabled").unwrap().set_bool(true);
            }
        };
        assert!(rms_difference(&dry, &render(&track, on_main(1.0))) > 0.01);
        assert_eq!(rms_difference(&dry, &render(&track, on_main(0.0))), 0.0);
    }

    #[test]
    fn super1_moves_metas_and_selector_cycles_effects() {
        let (reg, mut engine, _input) = setup();
        let unit = "[EffectRack1_EffectUnit1]";
        let slot = |n: usize, key: &str| {
            reg.get(&format!("[EffectRack1_EffectUnit1_Effect{n}]"), key)
                .unwrap()
        };
        // Echo in slot 1 and the Filter in slot 3 (the decks start with
        // Reverb, Echo and Gate).
        slot(1, "loaded_effect").set(1.0);
        slot(3, "loaded_effect").set(3.0);
        run(&mut engine, 0.01);
        reg.get(unit, "super1").unwrap().set(0.9);
        run(&mut engine, 0.01);
        for n in 1..=3 {
            assert_eq!(slot(n, "meta").get(), 0.9);
        }
        // Slot 3 holds the Filter: turning meta up sweeps its high-pass.
        assert_eq!(slot(3, "parameter1").get(), 1.0);
        assert!((slot(3, "parameter3").get() - 0.9).abs() < 1e-9);
        assert_eq!(slot(3, "num_parameters").get(), 3.0);

        assert_eq!(slot(1, "loaded_effect").get(), 1.0);
        slot(1, "effect_selector").set(1.0);
        run(&mut engine, 0.01);
        assert_eq!(slot(1, "loaded_effect").get(), 2.0);
        assert_eq!(slot(1, "effect_selector").get(), 0.0);
        slot(1, "effect_selector").set(-1.0);
        run(&mut engine, 0.01);
        slot(1, "effect_selector").set(-1.0);
        run(&mut engine, 0.01);
        assert_eq!(
            slot(1, "loaded_effect").get(),
            AVAILABLE_EFFECTS.len() as f64
        );
        slot(1, "next_effect").set_bool(true);
        run(&mut engine, 0.01);
        assert_eq!(crate::effect_name(slot(1, "loaded_effect").get()), "Echo");
        slot(1, "clear").set_bool(true);
        run(&mut engine, 0.01);
        assert_eq!(slot(1, "loaded").get(), 0.0);
        assert_eq!(crate::effect_name(slot(1, "loaded_effect").get()), "");
    }

    #[test]
    fn fast_scrubbing_is_softened() {
        let sr = 48_000.0;
        let rms = |hz: f64, speed: f64| {
            let mut state = [0.0; 2];
            let mut sum = 0.0;
            for i in 0..48_000 {
                let v = (std::f64::consts::TAU * hz * i as f64 / sr).sin() as f32;
                let o = soften_scrub([v, v], speed, sr, &mut state);
                if i >= 4_800 {
                    sum += (o[0] * o[0]) as f64;
                }
            }
            (sum / 43_200.0).sqrt()
        };
        // Normal speed: a mid tone passes nearly untouched.
        assert!(rms(1_000.0, 1.0) > 0.65, "{}", rms(1_000.0, 1.0));
        // A fast pass's whine is mostly gone.
        assert!(rms(6_000.0, 30.0) < 0.05, "{}", rms(6_000.0, 30.0));
    }

    #[test]
    fn waveform_drag_glides_to_the_pointer() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(120.0, 60.0));
        run(&mut engine, 0.01);
        let ctl = |k: &str| reg.get("[Channel1]", k).unwrap();
        let seconds = || ctl("playposition").get() * 60.0;
        ctl("scrub_position").set(0.1);
        ctl("scrub_enable").set_bool(true);
        // On its way after a moment, playing what it passes, not there yet.
        assert!(run(&mut engine, 0.02) > 0.01, "audible while dragging");
        let early = seconds();
        assert!(early > 0.5 && early < 5.9, "{early}");
        run(&mut engine, 0.5);
        assert!((seconds() - 6.0).abs() < 0.05, "{}", seconds());
        // Far moves jump most of the way, then glide.
        ctl("scrub_position").set(0.9);
        run(&mut engine, 0.02);
        assert!(seconds() > 45.0, "{}", seconds());
        run(&mut engine, 0.5);
        assert!((seconds() - 54.0).abs() < 0.05, "{}", seconds());
        // Letting go leaves a stopped deck where it is.
        ctl("scrub_enable").set_bool(false);
        run(&mut engine, 0.2);
        assert!((seconds() - 54.0).abs() < 0.05, "{}", seconds());
    }

    #[test]
    fn overview_drag_is_silent_and_resumes_where_let_go() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(120.0, 60.0));
        run(&mut engine, 0.01);
        let ctl = |k: &str| reg.get("[Channel1]", k).unwrap();
        let seconds = || ctl("playposition").get() * 60.0;
        ctl("play").set_bool(true);
        assert!(run(&mut engine, 0.1) > 0.01, "audible while playing");
        // Holding the overview: dragging moves the deck without a sound.
        ctl("seek_hold").set_bool(true);
        ctl("playposition").set(0.3);
        // Past the mixer's own short tail, as when stopping.
        run(&mut engine, 0.02);
        assert!(run(&mut engine, 0.1) < 1e-4, "silent while held");
        ctl("playposition").set(0.5);
        assert!(run(&mut engine, 0.1) < 1e-4, "silent while dragging");
        assert!((seconds() - 30.0).abs() < 0.01, "{}", seconds());
        // Letting go plays on from there.
        ctl("seek_hold").set_bool(false);
        assert!(run(&mut engine, 0.1) > 0.01, "audible after letting go");
        assert!(seconds() > 30.05 && seconds() < 30.2, "{}", seconds());
    }

    #[test]
    fn scratching_backwards_and_jog() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(120.0, 10.0));
        run(&mut engine, 0.01);
        let ctl = |k: &str| reg.get("[Channel1]", k).unwrap();
        let seconds = || ctl("playposition").get() * 10.0;
        ctl("playposition").set(0.5);
        run(&mut engine, 0.01);
        ctl("scratch2_enable").set_bool(true);
        ctl("scratch2").set(-1.0);
        assert!(run(&mut engine, 1.0) > 0.01, "audible while scratching");
        assert!((seconds() - 4.0).abs() < 0.02, "{}", seconds());
        // Play does not matter while scratching; zero speed holds still.
        ctl("play").set_bool(true);
        ctl("scratch2").set(0.0);
        run(&mut engine, 0.05);
        let held = seconds();
        run(&mut engine, 0.5);
        assert!((seconds() - held).abs() < 1e-6);
        // Scratching back past the start stops there.
        ctl("scratch2").set(-20.0);
        run(&mut engine, 0.5);
        assert_eq!(seconds(), 0.0);

        // Jog while paused scrubs forward: 10 ms of track per unit.
        ctl("scratch2_enable").set_bool(false);
        ctl("play").set_bool(false);
        run(&mut engine, 0.02);
        ctl("playposition").set(0.2);
        run(&mut engine, 0.02);
        ctl("jog").set(20.0);
        assert!(run(&mut engine, 0.5) > 0.01, "a nudge is heard");
        assert_eq!(ctl("jog").get(), 0.0);
        assert!((seconds() - 2.2).abs() < 0.01, "{}", seconds());

        // A fast search (SHIFT + jog) moves on silently, then sound returns.
        ctl("play").set_bool(true);
        run(&mut engine, 0.2);
        let before = seconds();
        let mut quiet = 0.0f32;
        for i in 0..4 {
            ctl("jog").set(75.0);
            let peak = run(&mut engine, 0.02);
            // The first buffer fades out.
            if i > 0 {
                quiet = quiet.max(peak);
            }
        }
        assert!(seconds() > before + 1.5, "{} -> {}", before, seconds());
        assert!(quiet < 1e-3, "silent while searching: {quiet}");
        run(&mut engine, 0.3);
        assert!(run(&mut engine, 0.2) > 0.01, "heard again after searching");
    }

    #[test]
    fn turning_the_jog_rim_bends_the_speed() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(120.0, 60.0));
        run(&mut engine, 0.01);
        let ctl = |k: &str| reg.get("[Channel1]", k).unwrap();
        let seconds = || ctl("playposition").get() * 60.0;
        ctl("play").set_bool(true);
        run(&mut engine, 0.2);
        // Turn the rim at 180 ticks a second, a quarter turn, for about a
        // second; returns the track seconds played per second.
        let turn = |engine: &mut Engine, ticks: f64| {
            let dt = 512.0 / SR;
            let n = 90;
            let start = seconds();
            for _ in 0..n {
                ctl("jog_bend").set(ticks * dt);
                run(engine, dt);
            }
            (seconds() - start) / (n as f64 * dt)
        };
        // About +14%, a little less while the bend builds up.
        let faster = turn(&mut engine, 180.0);
        assert!(faster > 1.1 && faster < 1.15, "clockwise: {faster}");
        assert_eq!(ctl("jog_bend").get(), 0.0);
        // It springs back once the rim stops.
        run(&mut engine, 1.0);
        let start = seconds();
        run(&mut engine, 1.0);
        assert!(
            (seconds() - start - 1.0).abs() < 0.01,
            "{}",
            seconds() - start
        );
        let slower = turn(&mut engine, -180.0);
        assert!(slower < 0.9 && slower > 0.85, "counterclockwise: {slower}");
        // Paused, the rim moves the track 8 ms a tick.
        ctl("play").set_bool(false);
        run(&mut engine, 0.5);
        let start = seconds();
        ctl("jog_bend").set(25.0);
        run(&mut engine, 0.5);
        assert!(
            (seconds() - start - 0.2).abs() < 0.01,
            "{}",
            seconds() - start
        );
    }

    #[test]
    fn reverseroll_and_loop_rolls_return_to_the_slip_position() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(120.0, 20.0));
        run(&mut engine, 0.01);
        let ctl = |k: &str| reg.get("[Channel1]", k).unwrap();
        let seconds = || ctl("playposition").get() * 20.0;
        ctl("play").set_bool(true);
        run(&mut engine, 1.0);
        let start = seconds();
        ctl("reverseroll").set_bool(true);
        run(&mut engine, 0.5);
        assert!(seconds() < start - 0.4, "plays backwards: {}", seconds());
        ctl("reverseroll").set_bool(false);
        run(&mut engine, 0.01);
        let expected = start + 0.51;
        assert!(
            (seconds() - expected).abs() < 0.03,
            "{} vs {expected}",
            seconds()
        );

        let start = seconds();
        ctl("beatlooproll_0.5_activate").set_bool(true);
        run(&mut engine, 1.0);
        assert!(ctl("loop_enabled").get_bool());
        let (a, b) = (
            ctl("loop_start_position").get(),
            ctl("loop_end_position").get(),
        );
        assert!((b - a - 0.25).abs() < 1e-6);
        ctl("beatlooproll_0.5_activate").set_bool(false);
        run(&mut engine, 0.01);
        assert!(!ctl("loop_enabled").get_bool());
        assert!((seconds() - (start + 1.01)).abs() < 0.03, "{}", seconds());
    }

    #[test]
    fn slip_mode_returns_when_each_trick_ends() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(120.0, 30.0));
        run(&mut engine, 0.01);
        let ctl = |k: &str| reg.get("[Channel1]", k).unwrap();
        let seconds = || ctl("playposition").get() * 30.0;
        // Within a buffer or two of where the track would have been.
        let near = |want: f64| {
            let got = seconds();
            assert!((got - want).abs() < 0.03, "{got} vs {want}");
        };
        ctl("slip_enabled").set_bool(true);
        ctl("play").set_bool(true);
        run(&mut engine, 1.0);
        assert_eq!(ctl("slip_position").get(), -1.0, "no ghost without a trick");

        // Scratching: held still for half a second, then let go.
        let start = seconds();
        ctl("scratch2_enable").set_bool(true);
        ctl("scratch2").set(0.0);
        run(&mut engine, 0.5);
        assert!((seconds() - start).abs() < 0.03, "held: {}", seconds());
        let ghost = ctl("slip_position").get();
        assert!((ghost - (start + 0.5)).abs() < 0.03, "ghost {ghost}");
        ctl("scratch2_enable").set_bool(false);
        run(&mut engine, 0.01);
        near(start + 0.51);
        assert!(ctl("slip_enabled").get_bool(), "slip stays on");
        assert_eq!(ctl("slip_position").get(), -1.0);

        // A loop: exiting it carries on from the silent playhead.
        let start = seconds();
        ctl("beatloop_1_toggle").set_bool(true);
        run(&mut engine, 0.01);
        ctl("beatloop_1_toggle").set_bool(false);
        run(&mut engine, 2.0);
        assert!(seconds() < start + 0.6, "looping: {}", seconds());
        ctl("reloop_toggle").set_bool(true);
        run(&mut engine, 0.01);
        ctl("reloop_toggle").set_bool(false);
        assert!(!ctl("loop_enabled").get_bool());
        near(start + 2.01);

        // A hot cue held: it plays from its point, then back on time.
        ctl("hotcue_1_position").set(2.0);
        ctl("hotcue_1_enabled").set_bool(true);
        ctl("hotcue_1_type").set(1.0);
        let start = seconds();
        ctl("hotcue_1_activate").set_bool(true);
        run(&mut engine, 0.5);
        assert!(
            (seconds() - 2.5).abs() < 0.03,
            "plays the cue: {}",
            seconds()
        );
        ctl("hotcue_1_activate").set_bool(false);
        run(&mut engine, 0.01);
        near(start + 0.51);

        // Reverse: back to where it would have been when it's turned off.
        let start = seconds();
        ctl("reverse").set_bool(true);
        run(&mut engine, 0.5);
        assert!(seconds() < start - 0.4, "backwards: {}", seconds());
        ctl("reverse").set_bool(false);
        run(&mut engine, 0.01);
        near(start + 0.51);

        // A pause: the silent playhead runs on, play picks up from it.
        let start = seconds();
        ctl("play").set_bool(false);
        run(&mut engine, 1.0);
        assert!((seconds() - start).abs() < 0.03, "paused: {}", seconds());
        ctl("play").set_bool(true);
        run(&mut engine, 0.01);
        near(start + 1.01);
    }

    #[test]
    fn slip_mode_leaves_a_paused_deck_and_plain_jumps_alone() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(120.0, 30.0));
        run(&mut engine, 0.01);
        let ctl = |k: &str| reg.get("[Channel1]", k).unwrap();
        let seconds = || ctl("playposition").get() * 30.0;
        ctl("slip_enabled").set_bool(true);
        ctl("playposition").set(0.1);
        run(&mut engine, 0.01);
        // Paused: scratching to find a spot stays there.
        ctl("scratch2_enable").set_bool(true);
        ctl("scratch2").set(1.0);
        run(&mut engine, 0.5);
        ctl("scratch2_enable").set_bool(false);
        run(&mut engine, 0.1);
        assert!((seconds() - 3.5).abs() < 0.03, "{}", seconds());

        // Playing: a beat jump is no trick, and CUE stops at the cue point.
        ctl("play").set_bool(true);
        run(&mut engine, 0.5);
        let start = seconds();
        ctl("beatjump_4_forward").set_bool(true);
        run(&mut engine, 0.01);
        ctl("beatjump_4_forward").set_bool(false);
        run(&mut engine, 0.5);
        assert!((seconds() - (start + 2.51)).abs() < 0.03, "{}", seconds());
        assert_eq!(ctl("slip_position").get(), -1.0);
        ctl("cue_point").set(1.0);
        ctl("cue_default").set_bool(true);
        run(&mut engine, 0.01);
        ctl("cue_default").set_bool(false);
        run(&mut engine, 0.5);
        assert!((seconds() - 1.0).abs() < 1e-6, "{}", seconds());
        assert_eq!(ctl("slip_position").get(), -1.0);
    }

    #[test]
    fn slip_switched_off_mid_loop_returns_and_ends_the_loop() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(120.0, 30.0));
        run(&mut engine, 0.01);
        let ctl = |k: &str| reg.get("[Channel1]", k).unwrap();
        let seconds = || ctl("playposition").get() * 30.0;
        ctl("slip_enabled").set_bool(true);
        ctl("play").set_bool(true);
        run(&mut engine, 1.0);
        let start = seconds();
        ctl("beatloop_1_toggle").set_bool(true);
        run(&mut engine, 0.01);
        ctl("beatloop_1_toggle").set_bool(false);
        run(&mut engine, 2.0);
        ctl("slip_enabled").set_bool(false);
        run(&mut engine, 0.5);
        assert!(!ctl("loop_enabled").get_bool());
        assert!((seconds() - (start + 2.51)).abs() < 0.03, "{}", seconds());
    }

    #[test]
    fn keyboard_pads_play_the_last_hot_cue_shifted() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(120.0, 20.0));
        run(&mut engine, 0.01);
        let ctl = |k: &str| reg.get("[Channel1]", k).unwrap();
        let seconds = || ctl("playposition").get() * 20.0;
        let press = |k: &str, engine: &mut Engine| {
            ctl(k).set_bool(true);
            ctl(k).set_bool(false);
            run(engine, 0.01);
        };
        // With no hot cue set, the pads play from the cue point.
        ctl("cue_point").set(2.0);
        press("keyboard_2_activate", &mut engine);
        assert!(ctl("play").get_bool());
        assert_eq!(ctl("pitch_adjust").get(), 1.0);
        assert!((seconds() - 2.0).abs() < 0.05, "{}", seconds());
        // Two hot cues: the one pressed last is the one played.
        for (n, at) in [(1, 5.0), (3, 9.0)] {
            ctl(&format!("hotcue_{n}_position")).set(at);
            ctl(&format!("hotcue_{n}_enabled")).set_bool(true);
            ctl(&format!("hotcue_{n}_type")).set(1.0);
        }
        press("hotcue_3_activate", &mut engine);
        run(&mut engine, 0.5);
        press("keyboard_6_activate", &mut engine);
        assert_eq!(ctl("pitch_adjust").get(), KEYBOARD_SEMITONES[5]);
        assert!((seconds() - 9.0).abs() < 0.05, "{}", seconds());
        // Its focus gone, the first hot cue set takes over.
        ctl("hotcue_focus").set(-1.0);
        press("keyboard_1_activate", &mut engine);
        assert_eq!(ctl("pitch_adjust").get(), 0.0);
        assert!((seconds() - 5.0).abs() < 0.05, "{}", seconds());
        // While they play the instrument, the deck leaves them alone.
        reg.get("[Instrument1]", "keyboard_pads")
            .unwrap()
            .set_bool(true);
        press("keyboard_2_activate", &mut engine);
        assert_eq!(ctl("pitch_adjust").get(), 0.0);
    }

    #[test]
    fn short_beat_loops_and_rolls_exist() {
        let (reg, _engine, _input) = setup();
        for key in [
            "beatloop_0.25_toggle",
            "beatlooproll_0.0625_activate",
            "beatlooproll_0.125_activate",
        ] {
            assert!(reg.get("[Channel1]", key).is_some(), "{key}");
        }
    }

    #[test]
    fn transport_buttons_and_indicators() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(120.0, 10.0));
        run(&mut engine, 0.01);
        let ctl = |k: &str| reg.get("[Channel1]", k).unwrap();
        let seconds = || ctl("playposition").get() * 10.0;
        assert!(ctl("cue_indicator").get_bool(), "stopped at the cue point");
        ctl("cue_gotoandplay").set_bool(true);
        run(&mut engine, 0.5);
        assert!(ctl("play_indicator").get_bool());
        assert!(!ctl("cue_indicator").get_bool());
        // beatjump_size 4 at 120 BPM is 2 s.
        let before = seconds();
        ctl("beatjump_forward").set_bool(true);
        run(&mut engine, 0.01);
        assert!((seconds() - before - 2.01).abs() < 0.02, "{}", seconds());
        ctl("beatjump").set(-2.0);
        run(&mut engine, 0.01);
        assert_eq!(ctl("beatjump").get(), 0.0);
        assert!((seconds() - before - 1.02).abs() < 0.03, "{}", seconds());
        ctl("start_stop").set_bool(true);
        run(&mut engine, 0.01);
        assert!(!ctl("play").get_bool());
        assert_eq!(seconds(), 0.0);
        ctl("reverse").set_bool(true);
        ctl("playposition").set(0.5);
        ctl("play").set_bool(true);
        run(&mut engine, 1.0);
        assert!((seconds() - 4.0).abs() < 0.02, "{}", seconds());

        // loop_scale halves the loop by moving its end; beatloop_size stays.
        ctl("reverse").set_bool(false);
        ctl("beatloop_4_activate").set_bool(true);
        run(&mut engine, 0.01);
        ctl("loop_scale").set(0.5);
        run(&mut engine, 0.01);
        let (a, b) = (
            ctl("loop_start_position").get(),
            ctl("loop_end_position").get(),
        );
        assert!((b - a - 1.0).abs() < 1e-9, "{a}..{b}");
        assert_eq!(ctl("beatloop_size").get(), 4.0);
        assert_eq!(ctl("loop_scale").get(), 0.0);

        // loop_move_loops moves the 1 s loop by its length, loop_move by
        // beats; the playing position goes along.
        let at = seconds();
        ctl("loop_move_loops").set(1.0);
        run(&mut engine, 0.01);
        assert!((ctl("loop_start_position").get() - a - 1.0).abs() < 1e-6);
        assert!((seconds() - at - 1.01).abs() < 0.03, "{}", seconds());
        ctl("loop_move").set(-2.0);
        run(&mut engine, 0.01);
        assert!((ctl("loop_start_position").get() - a).abs() < 1e-6);
        assert_eq!(ctl("loop_move").get(), 0.0);

        // CUP: back to the cue point while held, playing from it on release.
        ctl("loop_enabled").set_bool(false);
        ctl("cue_play").set_bool(true);
        run(&mut engine, 0.1);
        assert!(!ctl("play").get_bool());
        assert!((seconds() - ctl("cue_point").get()).abs() < 1e-9);
        ctl("cue_play").set_bool(false);
        run(&mut engine, 0.1);
        assert!(ctl("play").get_bool());

        // Mixxx 2.4's meter names reach the same controls.
        assert!(reg.get("[Channel1]", "vu_meter").unwrap().get() > 0.0);
        assert!(reg.get("[Main]", "vu_meter_left").unwrap().get() > 0.0);
    }

    #[test]
    fn cue_previews_while_held_unless_set_and_return() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(120.0, 10.0));
        run(&mut engine, 0.01);
        let ctl = |k: &str| reg.get("[Channel1]", k).unwrap();
        let seconds = || ctl("playposition").get() * 10.0;
        ctl("playposition").set(0.2);
        run(&mut engine, 0.01);
        // Held on a stopped deck: sets the cue point there and plays.
        ctl("cue_default").set_bool(true);
        run(&mut engine, 0.5);
        assert!(ctl("play").get_bool());
        assert!((ctl("cue_point").get() - 2.0).abs() < 0.02);
        assert!(seconds() > 2.3, "{}", seconds());
        // Released: stopped, back at the cue point.
        ctl("cue_default").set_bool(false);
        run(&mut engine, 0.05);
        assert!(!ctl("play").get_bool());
        assert!((seconds() - ctl("cue_point").get()).abs() < 1e-9);
        // Play pressed during the preview keeps it playing.
        ctl("cue_default").set_bool(true);
        run(&mut engine, 0.1);
        ctl("play").set_bool(false);
        ctl("play").set_bool(true);
        run(&mut engine, 0.05);
        ctl("cue_default").set_bool(false);
        run(&mut engine, 0.1);
        assert!(ctl("play").get_bool());
        // Numark (set and return): no preview.
        ctl("cue_default").set_bool(true);
        run(&mut engine, 0.05);
        ctl("cue_default").set_bool(false);
        run(&mut engine, 0.05);
        assert!(!ctl("play").get_bool());
        ctl("cue_mode").set(3.0);
        ctl("cue_default").set_bool(true);
        run(&mut engine, 0.3);
        assert!(!ctl("play").get_bool());
        ctl("cue_default").set_bool(false);
    }

    #[test]
    fn crossfader_curves() {
        let close =
            |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3;
        for shape in 0..3 {
            assert!(close(xfader_gains(-1.0, shape), [1.0, 0.0]), "{shape}");
            assert!(close(xfader_gains(1.0, shape), [0.0, 1.0]), "{shape}");
        }
        assert!(close(xfader_gains(0.0, 0), [1.0, 1.0]));
        assert!(close(xfader_gains(0.0, 1), [0.5, 0.5]));
        // Scratch: both full until the last few percent of travel.
        assert!(close(xfader_gains(0.85, 2), [1.0, 1.0]));
        assert!(xfader_gains(0.97, 2)[0] < 0.7);
    }

    #[test]
    fn decks_follow_their_crossfader_side() {
        // Deck 3 starts on the left, deck 4 on the right.
        let loud = |orientation: Option<f64>, crossfader: f64| {
            let (reg, mut engine, mut input) = setup();
            load(&mut input, 2, short_track(124.0, 4.0));
            if let Some(o) = orientation {
                reg.get("[Channel3]", "orientation").unwrap().set(o);
            }
            reg.get("[Master]", "crossfader").unwrap().set(crossfader);
            run(&mut engine, 0.01);
            reg.get("[Channel3]", "play").unwrap().set_bool(true);
            let out = record(&mut engine, 0.5);
            out.iter().map(|x| x.abs()).fold(0.0, f32::max) > 0.01
        };
        assert_eq!(reg_default_orientation(), [0.0, 2.0, 0.0, 2.0]);
        assert!(loud(None, -1.0));
        assert!(!loud(None, 1.0), "deck 3 is on the left");
        assert!(loud(Some(2.0), 1.0));
        assert!(!loud(Some(2.0), -1.0));
        // Thru: the crossfader leaves it alone.
        assert!(loud(Some(1.0), -1.0) && loud(Some(1.0), 1.0));
    }

    #[test]
    fn disabled_crossfader_plays_every_deck() {
        let loud = |enabled: bool, autodj: bool| {
            let (reg, mut engine, mut input) = setup();
            load(&mut input, 2, short_track(124.0, 4.0));
            reg.get("[Mixer]", "crossfader_enabled")
                .unwrap()
                .set_bool(enabled);
            reg.get("[AutoDJ]", "enabled").unwrap().set_bool(autodj);
            // Deck 3 is on the left; the crossfader is all the way right.
            reg.get("[Master]", "crossfader").unwrap().set(1.0);
            run(&mut engine, 0.01);
            reg.get("[Channel3]", "play").unwrap().set_bool(true);
            let out = record(&mut engine, 0.5);
            out.iter().map(|x| x.abs()).fold(0.0, f32::max) > 0.01
        };
        assert!(!loud(true, false));
        assert!(loud(false, false));
        // Automix needs the crossfader to mix, so it keeps working.
        assert!(!loud(false, true));
    }

    fn reg_default_orientation() -> [f64; 4] {
        let (reg, _engine, _input) = setup();
        std::array::from_fn(|d| reg.get(&crate::deck_group(d), "orientation").unwrap().get())
    }

    #[test]
    fn deck_3_syncs_to_the_playing_deck_4() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 2, short_track(124.0, 10.0));
        load(&mut input, 3, short_track(128.0, 10.0));
        run(&mut engine, 0.02);
        reg.get("[Channel4]", "play").unwrap().set_bool(true);
        reg.get("[Channel3]", "sync_enabled")
            .unwrap()
            .set_bool(true);
        run(&mut engine, 0.05);
        let bpm3 = reg.get("[Channel3]", "bpm").unwrap().get();
        assert!((bpm3 - 128.0).abs() < 0.01, "{bpm3}");
        assert_eq!(engine.sync_leader, Some(3));
    }

    #[test]
    fn split_cue_puts_the_cue_left_and_the_main_mix_right() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, tone_track(440.0, "A"));
        run(&mut engine, 0.05);
        let ch = |k: &str| reg.get("[Channel1]", k).unwrap();
        ch("play").set(1.0);
        ch("pfl").set(1.0);
        // The deck is only in the cue: the main mix is silent.
        ch("volume").set(0.0);
        reg.get("[Master]", "headSplit").unwrap().set(1.0);
        let mut buf = vec![0.0f32; 512 * 4];
        let mut peak = [0.0f32; 4];
        // Past the volume's ramp down.
        for _ in 0..5 {
            engine.process(&mut buf, 4);
        }
        for _ in 0..40 {
            engine.process(&mut buf, 4);
            for f in buf.as_chunks::<4>().0 {
                for (p, v) in peak.iter_mut().zip(f) {
                    *p = p.max(v.abs());
                }
            }
        }
        assert!(peak[2] > 0.05, "cue left {peak:?}");
        assert!(peak[3] < 1e-4, "main right {peak:?}");
    }

    #[test]
    fn a_groove_box_ejects_its_decks_track() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(120.0, 4.0));
        run(&mut engine, 0.01);
        let ctl = |k: &str| reg.get("[Channel1]", k).unwrap();
        assert!(ctl("track_loaded").get_bool());
        reg.get("[Groove1]", "enabled").unwrap().set_bool(true);
        run(&mut engine, 0.01);
        assert!(!ctl("track_loaded").get_bool());
        assert!(reg.get("[Groove1]", "enabled").unwrap().get_bool());
    }

    #[test]
    fn eject_clears_the_deck() {
        let (reg, mut engine, mut input) = setup();
        let track = short_track(120.0, 4.0);
        load(&mut input, 0, track.clone());
        run(&mut engine, 0.01);
        let ctl = |k: &str| reg.get("[Channel1]", k).unwrap();
        ctl("play").set_bool(true);
        ctl("eject").set_bool(true);
        ctl("eject").set_bool(false);
        run(&mut engine, 0.01);
        assert!(
            ctl("track_loaded").get_bool(),
            "a playing deck keeps its track"
        );
        ctl("play").set_bool(false);
        ctl("eject").set_bool(true);
        run(&mut engine, 0.01);
        assert!(!ctl("track_loaded").get_bool());
        assert_eq!(ctl("duration").get(), 0.0);
        let ejected: Vec<(usize, bool)> = input
            .collect_garbage()
            .into_iter()
            .filter_map(|r| match r {
                Returned::Ejected { deck, track: t } => Some((deck, Arc::ptr_eq(&t, &track))),
                _ => None,
            })
            .collect();
        assert_eq!(ejected, [(0, true)]);
        ctl("play").set_bool(true);
        assert!(run(&mut engine, 0.1) < 1e-4, "only EQ ringing is left");
    }

    #[test]
    fn mute_balance_and_quick_effect_bypass() {
        let track = short_track(124.0, 4.0);
        let dry = render(&track, |_| {});
        let muted = render(&track, |reg| {
            reg.get("[Channel1]", "mute").unwrap().set_bool(true)
        });
        assert!(muted.iter().all(|&v| v == 0.0));
        let left = render(&track, |reg| {
            reg.get("[Master]", "balance").unwrap().set(-1.0)
        });
        assert!(left.iter().skip(1).step_by(2).all(|&v| v == 0.0));
        let bypassed = |reg: &ControlRegistry| {
            let q = "[QuickEffectRack1_[Channel1]]";
            reg.get(q, "super1").unwrap().set(0.1);
            reg.get(q, "enabled").unwrap().set_bool(false);
        };
        assert_eq!(rms_difference(&dry, &render(&track, bypassed)), 0.0);
    }

    /// Times the engine with every effect slot on, routed to both decks and
    /// the main mix. Run with `--nocapture` to see the realtime factor.
    #[test]
    fn stays_cheap_with_every_effect_on() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(124.0, 12.0));
        load(&mut input, 1, short_track(128.0, 12.0));
        run(&mut engine, 0.01);
        for u in 1..=4 {
            let unit = format!("[EffectRack1_EffectUnit{u}]");
            for target in ["[Channel1]", "[Channel2]", "[Master]"] {
                reg.get(&unit, &format!("group_{target}_enable"))
                    .unwrap()
                    .set_bool(true);
            }
            for s in 1..=3 {
                let slot = format!("[EffectRack1_EffectUnit{u}_Effect{s}]");
                reg.get(&slot, "enabled").unwrap().set_bool(true);
            }
        }
        for d in ["[Channel1]", "[Channel2]"] {
            reg.get(d, "play").unwrap().set_bool(true);
        }
        let seconds = 10.0;
        let t = std::time::Instant::now();
        let peak = run(&mut engine, seconds);
        let elapsed = t.elapsed().as_secs_f64();
        let factor = seconds / elapsed;
        println!("10 s of audio, 36 effect instances on: {elapsed:.3} s, {factor:.0}x realtime");
        assert!(peak.is_finite() && peak > 0.01);
        assert!(factor > 2.0, "{factor}x realtime");
    }

    /// The worst case a set can throw at the audio thread: four decks with
    /// key lock and key shifts, every effect on, and each deck beatjumping
    /// (which re-seeds its time-stretcher) every few buffers. Times every
    /// 256-frame buffer, as a sound card at 5.8 ms would ask for them, and
    /// fails if the engine needs more than the buffer's time on average or
    /// the slowest buffer runs far past it. Run with `--nocapture` to see
    /// the numbers; `OPENDJ_STRESS_SECONDS` runs it longer, as a soak.
    #[test]
    fn keeps_up_under_stress() {
        const FRAMES: usize = 256;
        let (reg, mut engine, mut input) = setup();
        let seconds: f64 = std::env::var("OPENDJ_STRESS_SECONDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(6.0);
        // `OPENDJ_STRESS_NO_FX` and `OPENDJ_STRESS_NO_JUMPS` leave out the
        // effects or the beatjumps, to time key lock on its own.
        let fx = std::env::var_os("OPENDJ_STRESS_NO_FX").is_none();
        let jumps = std::env::var_os("OPENDJ_STRESS_NO_JUMPS").is_none();
        let track = short_track(124.0, 30.0);
        for deck in 0..NUM_DECKS {
            load(&mut input, deck, track.clone());
        }
        run(&mut engine, 0.01);
        for u in (1..=4).filter(|_| fx) {
            let unit = format!("[EffectRack1_EffectUnit{u}]");
            for target in [
                "[Channel1]",
                "[Channel2]",
                "[Channel3]",
                "[Channel4]",
                "[Master]",
            ] {
                reg.get(&unit, &format!("group_{target}_enable"))
                    .unwrap()
                    .set_bool(true);
            }
            for s in 1..=3 {
                let slot = format!("[EffectRack1_EffectUnit{u}_Effect{s}]");
                reg.get(&slot, "enabled").unwrap().set_bool(true);
            }
        }
        for (d, group) in (1..=NUM_DECKS).map(|d| (d, format!("[Channel{d}]"))) {
            let c = |k: &str| reg.get(&group, k).unwrap();
            c("keylock").set_bool(true);
            c("rate").set(0.02 * d as f64);
            c("pitch").set(d as f64 - 2.0);
            c("play").set_bool(true);
        }
        let buffers = (seconds * SR / FRAMES as f64).ceil() as usize;
        let budget = FRAMES as f64 / SR;
        let mut buf = vec![0.0f32; FRAMES * 6];
        let mut times = Vec::with_capacity(buffers);
        let mut peak = 0.0f32;
        for b in 0..buffers {
            // Each deck jumps on its own beat of a 32-buffer cycle, and every
            // so often all four jump in the same buffer.
            for d in (0..NUM_DECKS).filter(|_| jumps) {
                if b % 32 == d * 8 || b % 512 == 100 {
                    let jump = reg.get(&format!("[Channel{}]", d + 1), "beatjump");
                    jump.unwrap().set(if b % 64 < 32 { -4.0 } else { 4.0 });
                }
            }
            let t = Instant::now();
            engine.process(&mut buf, 6);
            times.push(t.elapsed().as_secs_f64() / budget);
            peak = buf.iter().fold(peak, |p, v| p.max(v.abs()));
        }
        let mean = times.iter().sum::<f64>() / times.len() as f64;
        times.sort_by(f64::total_cmp);
        let p99 = times[times.len() * 99 / 100];
        let worst = times[times.len() - 1];
        let late = times.iter().filter(|&&t| t > 1.0).count();
        println!(
            "{FRAMES}-frame buffers ({:.1} ms), 4 decks key-locked, 60 effect \
             instances, beatjumps: mean {:.0}%, p99 {:.0}%, worst {:.0}% of the \
             buffer; {late} of {} over",
            budget * 1000.0,
            mean * 100.0,
            p99 * 100.0,
            worst * 100.0,
            times.len()
        );
        assert!(peak.is_finite() && peak > 0.01 && peak <= 1.0);
        // Loose, for shared CI machines running other tests alongside: it
        // catches the engine getting much slower, not a single slow buffer.
        // A 4-core cloud machine measured a mean of 40% (2026-11-01).
        assert!(mean < 0.75, "mean {:.0}% of the buffer", mean * 100.0);
    }

    /// An input device for tests: the engine's feed, and the end to fill.
    fn mic_feed() -> (InputFeed, rtrb::Producer<f32>) {
        let (tx, rx) = rtrb::RingBuffer::new(48_000);
        let period = Arc::new(std::sync::atomic::AtomicUsize::new(512));
        (InputFeed { rx, period }, tx)
    }

    /// Runs the engine for `seconds` with `voice(frame)` coming into the
    /// microphone a buffer at a time, as a device does; returns the output.
    fn talk(
        engine: &mut Engine,
        tx: &mut rtrb::Producer<f32>,
        seconds: f64,
        voice: impl Fn(usize) -> f32,
    ) -> Vec<f32> {
        let mut out = Vec::new();
        let mut buf = vec![0.0f32; 512 * 2];
        let mut n = 0;
        for _ in 0..(seconds * SR / 512.0).ceil() as usize {
            for _ in 0..512 {
                let v = voice(n);
                n += 1;
                let _ = tx.push(v);
                let _ = tx.push(v);
            }
            engine.process(&mut buf, 2);
            out.extend_from_slice(&buf);
        }
        out
    }

    fn sine(hz: f64, level: f32) -> impl Fn(usize) -> f32 {
        move |n| (2.0 * std::f64::consts::PI * hz * n as f64 / SR).sin() as f32 * level
    }

    fn peak(x: &[f32]) -> f32 {
        x.iter().fold(0.0, |p, v| p.max(v.abs()))
    }

    /// The level of `hz` in the left channel of `interleaved`.
    fn level_at(interleaved: &[f32], hz: f64) -> f64 {
        let x: Vec<f32> = interleaved.iter().step_by(2).copied().collect();
        let w = 2.0 * std::f64::consts::PI * hz / SR;
        let (re, im) = x.iter().enumerate().fold((0.0, 0.0), |(re, im), (i, &v)| {
            (
                re + v as f64 * (w * i as f64).cos(),
                im + v as f64 * (w * i as f64).sin(),
            )
        });
        2.0 * (re * re + im * im).sqrt() / x.len() as f64
    }

    #[test]
    fn the_test_tone_beeps_in_the_master_or_the_headphones() {
        let (reg, mut engine, _input) = setup();
        let tone = reg.get("[Master]", "test_tone").unwrap();
        let mut run = |channels: usize| {
            let mut buf = vec![0.0f32; 4800 * channels];
            engine.process(&mut buf, channels);
            buf
        };
        assert!(peak(&run(2)) < 1e-6);
        tone.set(1.0);
        let master = run(2);
        assert!(peak(&master) > 0.05, "{}", peak(&master));
        // Headphones only: the master goes quiet (once the limiter's look
        // ahead has played out), channels 3 and 4 beep.
        tone.set(2.0);
        run(4);
        let four = run(4);
        let main: Vec<f32> = four.chunks(4).flat_map(|f| [f[0], f[1]]).collect();
        let phones: Vec<f32> = four.chunks(4).flat_map(|f| [f[2], f[3]]).collect();
        assert!(peak(&main) < 1e-6, "{}", peak(&main));
        assert!(peak(&phones) > 0.01, "{}", peak(&phones));
        tone.set(0.0);
        run(2);
        assert!(peak(&run(2)) < 1e-6);
    }

    #[test]
    fn the_microphone_is_heard_while_talkover_is_on() {
        let (reg, mut engine, _input) = setup();
        let mic = |k: &str| reg.get("[Microphone]", k).unwrap();
        assert!(!mic("input_configured").get_bool());
        let (feed, mut tx) = mic_feed();
        assert!(engine.set_input(MICROPHONE, Some(feed)).is_none());
        assert!(mic("input_configured").get_bool());
        // Off: nothing in the main mix, though the meter shows the voice.
        let off = talk(&mut engine, &mut tx, 0.3, sine(1000.0, 0.5));
        assert!(peak(&off) < 1e-4, "{}", peak(&off));
        assert!(mic("VuMeter").get() > 0.8);
        // On: the voice plays at its level.
        mic("talkover").set_bool(true);
        let on = talk(&mut engine, &mut tx, 0.3, sine(1000.0, 0.5));
        let level = level_at(&on[on.len() / 2..], 1000.0);
        assert!((level - 0.5).abs() < 0.03, "{level}");
        // Its fader, gain and kills apply.
        mic("volume").set(0.5);
        mic("pregain").set(0.5);
        let quieter = talk(&mut engine, &mut tx, 0.3, sine(1000.0, 0.5));
        let level = level_at(&quieter[quieter.len() / 2..], 1000.0);
        assert!((level - 0.125).abs() < 0.01, "{level}");
        reg.get("[EqualizerRack1_[Microphone]_Effect1]", "button_parameter2")
            .unwrap()
            .set_bool(true);
        let killed = talk(&mut engine, &mut tx, 0.3, sine(1000.0, 0.5));
        assert!(level_at(&killed[killed.len() / 2..], 1000.0) < 0.01);
        // Without a device it goes quiet and says so.
        assert!(engine.set_input(MICROPHONE, None).is_some());
        assert!(!mic("input_configured").get_bool());
    }

    #[test]
    fn the_music_ducks_under_the_microphone() {
        let (reg, mut engine, mut input) = setup();
        let deck = |k: &str| reg.get("[Channel1]", k).unwrap();
        load(&mut input, 0, long_tone_track(440.0, "Am", 10.0));
        run(&mut engine, 0.05);
        deck("play").set_bool(true);
        let (feed, mut tx) = mic_feed();
        engine.set_input(MICROPHONE, Some(feed));
        reg.get("[Microphone]", "talkover").unwrap().set_bool(true);
        let quiet = talk(&mut engine, &mut tx, 0.5, |_| 0.0);
        let music = level_at(&quiet[quiet.len() / 2..], 440.0);
        assert!(music > 0.3, "{music}");
        // Auto (the default): talking takes the music 12 dB down.
        let talking = talk(&mut engine, &mut tx, 0.5, sine(1000.0, 0.3));
        let ducked = level_at(&talking[talking.len() / 2..], 440.0);
        assert!((ducked / music - 0.25).abs() < 0.03, "{}", ducked / music);
        // It comes back up once the DJ stops talking.
        let after = talk(&mut engine, &mut tx, 3.0, |_| 0.0);
        let back = level_at(&after[after.len() - 24_000 * 2..], 440.0);
        assert!((back / music - 1.0).abs() < 0.03, "{}", back / music);
        // Off: talking leaves the music alone.
        reg.get("[Master]", "talkover_ducking").unwrap().set(0.0);
        let talking = talk(&mut engine, &mut tx, 0.5, sine(1000.0, 0.3));
        let level = level_at(&talking[talking.len() / 2..], 440.0);
        assert!((level / music - 1.0).abs() < 0.03, "{}", level / music);
    }

    #[test]
    fn the_microphone_has_its_own_effects() {
        let (reg, mut engine, mut input) = setup();
        let mic_unit = crate::effect_unit_group(crate::MIC_FX_UNIT);
        let unit = |k: &str| reg.get(&mic_unit, k).unwrap();
        // The second slot starts with Echo; it is routed to the mic.
        let echo_slot = crate::effect_group(crate::MIC_FX_UNIT, 1);
        let echo = |k: &str| reg.get(&echo_slot, k).unwrap();
        assert_eq!(echo("loaded_effect").get(), 1.0);
        assert!(unit("group_[Microphone]_enable").get_bool());
        let (feed, mut tx) = mic_feed();
        engine.set_input(MICROPHONE, Some(feed));
        reg.get("[Microphone]", "talkover").unwrap().set_bool(true);
        reg.get("[Master]", "talkover_ducking").unwrap().set(0.0);
        // A word, then silence: the dry mic stops with it.
        let word = |n: usize| if n < 4800 { sine(1000.0, 0.5)(n) } else { 0.0 };
        let dry = talk(&mut engine, &mut tx, 1.2, word);
        assert!(peak(&dry[14_400..]) < 1e-3);
        // With the echo on, it repeats.
        echo("enabled").set_bool(true);
        let wet = talk(&mut engine, &mut tx, 1.2, word);
        assert!(peak(&wet[14_400..]) > 0.1, "{}", peak(&wet[14_400..]));

        // The decks' sound doesn't change, even routed to this unit: it only
        // takes the inputs.
        echo("enabled").set_bool(false);
        load(&mut input, 0, tone_track(440.0, "Am"));
        run(&mut engine, 0.05);
        reg.get("[Channel1]", "play").unwrap().set_bool(true);
        talk(&mut engine, &mut tx, 1.0, |_| 0.0);
        let before = talk(&mut engine, &mut tx, 0.3, |_| 0.0);
        unit("group_[Channel1]_enable").set_bool(true);
        let after = talk(&mut engine, &mut tx, 0.3, |_| 0.0);
        let (a, b) = (level_at(&before, 440.0), level_at(&after, 440.0));
        assert!((a - b).abs() < 0.01, "{a} {b}");
        // Nor do the decks' units reach the mic.
        assert!(reg
            .get("[EffectRack1_EffectUnit1]", "group_[Microphone]_enable")
            .is_some());
    }

    #[test]
    fn the_auxiliary_input_joins_the_music() {
        let (reg, mut engine, _input) = setup();
        let aux = |k: &str| reg.get("[Auxiliary1]", k).unwrap();
        // Off from the start, and under Mixxx 2.3's name too.
        assert!(!aux("main_mix").get_bool());
        assert!(!aux("master").get_bool());
        let (feed, mut tx) = mic_feed();
        engine.set_input(AUXILIARY, Some(feed));
        let out = talk(&mut engine, &mut tx, 0.3, sine(500.0, 0.4));
        assert!(peak(&out[out.len() / 2..]) < 1e-4);
        aux("master").set_bool(true);
        assert!(aux("main_mix").get_bool());
        let out = talk(&mut engine, &mut tx, 0.3, sine(500.0, 0.4));
        let level = level_at(&out[out.len() / 2..], 500.0);
        assert!((level - 0.4).abs() < 0.03, "{level}");
        aux("main_mix").set_bool(false);
        let out = talk(&mut engine, &mut tx, 0.3, sine(500.0, 0.4));
        assert!(peak(&out[out.len() / 2..]) < 1e-4);
    }

    #[test]
    fn a_deck_plays_a_live_input_through_its_channel() {
        let (reg, mut engine, mut input) = setup();
        let aux = |k: &str| reg.get("[Auxiliary1]", k).unwrap();
        let ch = |d: usize, k: &str| reg.get(&format!("[Channel{d}]"), k).unwrap();
        let (feed, mut tx) = mic_feed();
        engine.set_input(AUXILIARY, Some(feed));
        // The aux's own channel is off: nothing is heard.
        let out = talk(&mut engine, &mut tx, 0.3, sine(500.0, 0.4));
        assert!(peak(&out[out.len() / 2..]) < 1e-4);
        // On deck 2 it plays through the deck's channel, play or not, and
        // the deck's track is ejected.
        load(&mut input, 1, short_track(120.0, 4.0));
        run(&mut engine, 0.05);
        ch(2, "live_input").set(2.0);
        let out = talk(&mut engine, &mut tx, 0.4, sine(500.0, 0.4));
        assert!(!ch(2, "track_loaded").get_bool());
        let level = level_at(&out[out.len() / 2..], 500.0);
        assert!(level > 0.1, "{level}");
        // The deck's fader turns it down.
        ch(2, "volume").set(0.0);
        talk(&mut engine, &mut tx, 0.1, sine(500.0, 0.4));
        let out = talk(&mut engine, &mut tx, 0.3, sine(500.0, 0.4));
        assert!(peak(&out[out.len() / 2..]) < 1e-3);
        // With the deck holding it, the aux's own switch adds nothing.
        ch(2, "volume").set(1.0);
        aux("main_mix").set_bool(true);
        let out = talk(&mut engine, &mut tx, 0.4, sine(500.0, 0.4));
        let doubled = level_at(&out[out.len() / 2..], 500.0);
        assert!((doubled - level).abs() < 0.05, "{level} {doubled}");
        aux("main_mix").set_bool(false);
        // Picked for deck 1, it leaves deck 2.
        ch(1, "live_input").set(2.0);
        talk(&mut engine, &mut tx, 0.05, sine(500.0, 0.4));
        assert_eq!(ch(2, "live_input").get(), 0.0);
        // A Groove Box picked on deck 1 takes over; eject empties the deck.
        reg.get("[Groove1]", "enabled").unwrap().set_bool(true);
        talk(&mut engine, &mut tx, 0.05, sine(500.0, 0.4));
        assert_eq!(ch(1, "live_input").get(), 0.0);
        ch(1, "live_input").set(2.0);
        talk(&mut engine, &mut tx, 0.05, sine(500.0, 0.4));
        assert!(!reg.get("[Groove1]", "enabled").unwrap().get_bool());
        ch(1, "eject").set_bool(true);
        talk(&mut engine, &mut tx, 0.05, sine(500.0, 0.4));
        ch(1, "eject").set_bool(false);
        assert_eq!(ch(1, "live_input").get(), 0.0);
        // A track loaded makes it a track deck again.
        ch(2, "live_input").set(2.0);
        talk(&mut engine, &mut tx, 0.05, sine(500.0, 0.4));
        load(&mut input, 1, short_track(120.0, 4.0));
        talk(&mut engine, &mut tx, 0.05, sine(500.0, 0.4));
        assert_eq!(ch(2, "live_input").get(), 0.0);
    }

    /// The largest jump in slope between neighbouring samples of the left
    /// channel: a smooth tone keeps it small, a click makes it spike.
    fn roughness(out: &[f32]) -> f32 {
        let l: Vec<f32> = out.iter().step_by(2).copied().collect();
        l.windows(3)
            .map(|w| (w[2] - 2.0 * w[1] + w[0]).abs())
            .fold(0.0, f32::max)
    }

    /// Plays a 150 Hz tone on deck 1 and calls `each` before every buffer.
    fn play_tone(
        keylock: bool,
        buffers: usize,
        mut each: impl FnMut(&ControlRegistry, usize),
    ) -> Vec<f32> {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, long_tone_track(150.0, "", 30.0));
        run(&mut engine, 0.01);
        let ctl = |k: &str| reg.get("[Channel1]", k).unwrap();
        ctl("keylock").set_bool(keylock);
        ctl("playposition").set(0.3);
        ctl("play").set_bool(true);
        run(&mut engine, 0.3);
        let mut out = Vec::new();
        let mut buf = vec![0.0f32; 512 * 2];
        for n in 0..buffers {
            each(&reg, n);
            engine.process(&mut buf, 2);
            out.extend_from_slice(&buf);
        }
        out
    }

    #[test]
    fn loading_the_master_deck_keeps_the_synced_deck_steady() {
        // Deck 1 syncs to deck 2, which is stopped; deck 2 then loads a
        // faster track, with the master pinned there or picked by openDJ.
        for (pinned, keylock) in [(true, false), (true, true), (false, true)] {
            let (reg, mut engine, mut input) = setup();
            load(&mut input, 0, long_tone_track(150.0, "", 30.0));
            load(&mut input, 1, long_tone_track(150.0, "", 30.0));
            run(&mut engine, 0.01);
            let c1 = |k: &str| reg.get("[Channel1]", k).unwrap();
            let c2 = |k: &str| reg.get("[Channel2]", k).unwrap();
            c2("volume").set(0.0);
            if pinned {
                c2("sync_leader").set(2.0);
            }
            c1("keylock").set_bool(keylock);
            c1("sync_enabled").set_bool(true);
            c1("playposition").set(0.3);
            c1("play").set_bool(true);
            run(&mut engine, 0.3);
            let leader = if pinned { 1 } else { 0 };
            assert_eq!(engine.sync_leader, Some(leader), "pinned {pinned}");
            let rate = c1("rate").get();
            let mut faster = (*long_tone_track(440.0, "", 30.0)).clone();
            faster.bpm = 128.0;
            load(&mut input, 1, Arc::new(faster));
            let out = record(&mut engine, 0.3);
            assert!(
                roughness(&out) < 0.0003,
                "pinned {pinned}: {}",
                roughness(&out)
            );
            assert_eq!(c1("rate").get(), rate, "pinned {pinned}");
            if pinned {
                // Deck 2 waits at deck 1's tempo, 120 BPM.
                let speed = 1.0 + c2("rate").get() * c2("rateRange").get();
                assert!((128.0 * speed - 120.0).abs() < 1e-6, "{speed}");
            }
            // The MASTER badge is lit on the deck leading.
            assert!(engine.decks[leader].c.sync_leader.get() >= 1.0);
        }
    }

    #[test]
    fn jumps_and_loops_do_not_click() {
        let steady = roughness(&play_tone(false, 100, |_, _| {}));
        assert!(steady < 0.0003, "{steady}");
        let ctl = |reg: &ControlRegistry, k: &str| reg.get("[Channel1]", k).unwrap();
        for keylock in [false, true] {
            // Beatjumps that land anywhere in the tone's cycle.
            let jumps = roughness(&play_tone(keylock, 100, |reg, n| {
                if n % 10 == 5 {
                    ctl(reg, "beatjump").set(if n % 20 == 5 { 1.37 } else { -0.71 });
                }
            }));
            assert!(jumps < 4.0 * steady, "key lock {keylock}: {jumps}");
            // Setting a hotcue, jumping back to it, and a seek.
            let cues = roughness(&play_tone(keylock, 60, |reg, n| match n {
                10 => ctl(reg, "hotcue_1_activate").set_bool(true),
                11 => ctl(reg, "hotcue_1_activate").set_bool(false),
                30 => ctl(reg, "hotcue_1_activate").set_bool(true),
                31 => ctl(reg, "hotcue_1_activate").set_bool(false),
                45 => ctl(reg, "playposition").set(0.123),
                _ => {}
            }));
            assert!(cues < 4.0 * steady, "key lock {keylock}: {cues}");
            // A loop of 28⅛ cycles: the tone breaks where it wraps.
            let looped = roughness(&play_tone(keylock, 100, |reg, n| {
                if n == 2 {
                    ctl(reg, "beatloop_size").set(0.375);
                    ctl(reg, "beatloop_activate").set_bool(true);
                }
            }));
            assert!(looped < 4.0 * steady, "key lock {keylock}: {looped}");
        }
    }

    #[test]
    fn key_lock_holds_through_nudges_without_clicks() {
        // Deck 1 plays 4% fast with key lock, so the stretcher and playing
        // as a record differ in pitch: switching between them hard clicks.
        let ctl = |reg: &ControlRegistry, k: &str| reg.get("[Channel1]", k).unwrap();
        let tone = |each: &dyn Fn(&ControlRegistry, usize)| {
            roughness(&play_tone(true, 60, |reg, n| {
                if n == 0 {
                    ctl(reg, "rate").set(0.5);
                }
                each(reg, n);
            }))
        };
        let steady = tone(&|_, _| {});
        assert!(steady < 0.0005, "{steady}");
        // Jog nudges of a playing deck, as many Mixxx mappings send them.
        let nudges = tone(&|reg, n| match n {
            20 | 35 => ctl(reg, "jog").set(1.0),
            45 => ctl(reg, "jog").set(-3.0),
            _ => {}
        });
        assert!(nudges < 4.0 * steady, "nudges: {nudges}");
        // Touching the platter to scratch, then letting go.
        let scratch = tone(&|reg, n| match n {
            20 => {
                ctl(reg, "scratch2").set(0.3);
                ctl(reg, "scratch2_enable").set_bool(true);
            }
            40 => ctl(reg, "scratch2_enable").set_bool(false),
            _ => {}
        });
        assert!(scratch < 4.0 * steady, "scratch: {scratch}");
    }

    #[test]
    fn filter_moves_do_not_click() {
        let steady = roughness(&play_tone(false, 100, |_, _| {}));
        let filter =
            |reg: &ControlRegistry| reg.get("[QuickEffectRack1_[Channel1]]", "super1").unwrap();
        // A slow low-pass sweep down and back, through the centre.
        let sweep = roughness(&play_tone(false, 200, |reg, n| {
            filter(reg).set((n as f64 / 100.0 - 1.0).abs() * 0.5);
        }));
        assert!(sweep < 2.0 * steady, "{sweep}");
        // Flicking between low-pass and high-pass every buffer, and a knob
        // thrown about.
        let flick = roughness(&play_tone(false, 200, |reg, n| {
            filter(reg).set(if n % 2 == 0 { 0.45 } else { 0.55 });
        }));
        assert!(flick < 0.005, "{flick}");
        let wild = roughness(&play_tone(false, 200, |reg, n| {
            filter(reg).set(0.5 + (n as f64 * 0.37).sin() * 0.4);
        }));
        assert!(wild < 0.005, "{wild}");
    }

    #[test]
    fn a_groove_box_plays_through_its_deck_channel() {
        let (reg, mut engine, _input) = setup();
        let g = |k: &str| reg.get("[Groove2]", k).unwrap();
        let ch = |k: &str| reg.get("[Channel2]", k).unwrap();
        // Off, the deck is silent even with play pressed.
        ch("play").set_bool(true);
        assert!(run(&mut engine, 0.3) < 1e-4);
        g("enabled").set_bool(true);
        ch("play").set_bool(false);
        run(&mut engine, 0.05);
        ch("play").set_bool(true);
        assert!(run(&mut engine, 0.5) > 0.05, "the groove is heard");
        assert!(g("position").get() >= 0.0);
        // The channel fader turns it down like any deck.
        ch("volume").set(0.0);
        run(&mut engine, 0.1);
        let quiet = run(&mut engine, 0.5);
        assert!(quiet < 1e-3, "{quiet}");
        // A track loaded on it makes it a track deck again.
        ch("play").set_bool(false);
        let mut input = _input;
        load(&mut input, 1, short_track(120.0, 4.0));
        run(&mut engine, 0.05);
        assert!(!g("enabled").get_bool());
    }

    #[test]
    fn a_groove_box_follows_the_playing_deck() {
        let (reg, mut engine, mut input) = setup();
        load(&mut input, 0, short_track(126.0, 20.0));
        run(&mut engine, 0.05);
        reg.get("[Channel1]", "play").unwrap().set_bool(true);
        run(&mut engine, 0.3);
        let g = |k: &str| reg.get("[Groove2]", k).unwrap();
        g("enabled").set_bool(true);
        reg.get("[Channel2]", "play").unwrap().set_bool(true);
        run(&mut engine, 2.0);
        assert!((g("bpm").get() - 126.0).abs() < 0.01, "{}", g("bpm").get());
        // The step playing matches deck 1's 16th.
        let pos = reg.get("[Channel1]", "playposition").unwrap().get() * 20.0;
        let beat = (pos - 0.1) * 126.0 / 60.0;
        let step = ((beat * 4.0).floor() as i64).rem_euclid(16) as f64;
        let shown = g("position").get();
        assert!(
            (shown - step).abs() <= 1.0 || (shown - step).abs() >= 15.0,
            "{shown} vs {step}"
        );
    }
}
