//! The scrolling waveform of a deck playing a live source (an input, an
//! instrument plugin or the Groove Box): the deck's sound as it comes in,
//! drawn up to the play line, on the leading deck's beat grid.

use std::collections::VecDeque;
use std::time::Instant;

use opendj_engine::live_wave::LiveSlice;
use opendj_engine::track::{Track, Waveform, WaveformBin, WAVEFORM_RATE};
use opendj_engine::NUM_DECKS;
use opendj_waveform::{GridSection, GRID_SECTIONS};

use crate::smooth::SmoothClock;

/// Slices drawn: about 8 s, the half of the waveform behind the play line
/// with room to spare. A power of two, so each coarser level halves it.
pub const WINDOW: usize = 8192;

/// The bands carry less energy than the whole signal; lifted as a track's
/// waveform lifts them (see the analysis), so the colours match.
const BAND_GAIN: [f32; 4] = [1.0, 1.4, 2.0, 1.0];

/// A slice as a waveform bin: the square root of each level against full
/// scale, from 0 to 255, as a track's bins are against its loudest peak.
pub fn bin(s: &LiveSlice) -> WaveformBin {
    let q = |v: f32, k: usize| ((v * BAND_GAIN[k]).clamp(0.0, 1.0).sqrt() * 255.0).round() as u8;
    std::array::from_fn(|i| {
        if i < 4 {
            q(s.peak[i], i)
        } else {
            q(s.rms[i - 4], i - 4)
        }
    })
}

/// One deck's sound so far.
#[derive(Default)]
struct Trace {
    /// The latest bins, oldest first; at most a window and a little more.
    bins: VecDeque<WaveformBin>,
    /// Slices received since the deck went live.
    received: u64,
    clock: SmoothClock,
}

/// What a live deck's scrolling waveform draws this frame.
pub struct Window {
    /// The waveform: [`WINDOW`] slices ending at the play line, and the
    /// coarser levels made from them.
    pub levels: Vec<Vec<WaveformBin>>,
    /// The play line, in seconds into the window.
    pub play_seconds: f64,
}

/// The decks' live sound, read from the engine on every frame.
pub struct LiveTraces {
    rx: [rtrb::Consumer<LiveSlice>; NUM_DECKS],
    decks: [Trace; NUM_DECKS],
}

impl LiveTraces {
    pub fn new(rx: [rtrb::Consumer<LiveSlice>; NUM_DECKS]) -> Self {
        Self {
            rx,
            decks: Default::default(),
        }
    }

    /// Takes what came in. `live[d]` says whether deck `d` plays live now;
    /// a deck that doesn't is cleared, so its next live part starts afresh.
    pub fn poll(&mut self, live: [bool; NUM_DECKS]) {
        for (d, rx) in self.rx.iter_mut().enumerate() {
            let trace = &mut self.decks[d];
            if !live[d] {
                while rx.pop().is_ok() {}
                if trace.received > 0 {
                    *trace = Trace::default();
                }
                continue;
            }
            while let Ok(s) = rx.pop() {
                trace.push(bin(&s));
            }
        }
    }

    /// Deck `d`'s waveform to draw at `now`, once anything has come in.
    pub fn window(&mut self, d: usize, now: Instant) -> Option<Window> {
        self.decks[d].window(now)
    }
}

impl Trace {
    fn push(&mut self, b: WaveformBin) {
        self.bins.push_back(b);
        self.received += 1;
        // Room for the play line to lag what came in by a buffer or two.
        while self.bins.len() > WINDOW + 1024 {
            self.bins.pop_front();
        }
    }

    fn window(&mut self, now: Instant) -> Option<Window> {
        if self.received == 0 {
            return None;
        }
        // Slices come a buffer at a time; the play line moves on smoothly
        // between them and draws only what it has passed.
        let came = self.received as f64 / WAVEFORM_RATE;
        let shown = self.clock.next(came, 1.0, now).min(came);
        let end = (shown * WAVEFORM_RATE).floor().max(0.0) as u64;
        // The bins up to the play line, the newest last.
        let held = self.bins.len() as u64;
        let first_held = self.received - held;
        let upto = (end.max(first_held) - first_held) as usize;
        let mut finest = vec![[0u8; 8]; WINDOW];
        let take = upto.min(WINDOW);
        for (i, b) in self.bins.range(upto - take..upto).enumerate() {
            finest[WINDOW - take + i] = *b;
        }
        let into = (shown * WAVEFORM_RATE - end as f64).max(0.0) / WAVEFORM_RATE;
        Some(Window {
            levels: Waveform::from_finest(WAVEFORM_RATE, finest).levels,
            play_seconds: WINDOW as f64 / WAVEFORM_RATE + into,
        })
    }
}

/// The leading deck's beat grid moved onto a live deck's window, so the live
/// part's beats line up with it. The leader is at beat `beat` (beat 0 a bar
/// line), at `bpm` as its grid says there, playing at `speed` times that;
/// the live deck's play line is `play_seconds` into its window.
pub fn leader_grid(
    bpm: f64,
    beat: f64,
    speed: f64,
    play_seconds: f64,
) -> [Option<GridSection>; GRID_SECTIONS] {
    let mut g = [None; GRID_SECTIONS];
    let bpm = bpm * speed;
    if bpm > 0.0 && speed > 0.0 {
        g[0] = Some(GridSection {
            seconds: play_seconds - beat * 60.0 / bpm,
            bpm,
            beat: 0.0,
        });
    }
    g
}

/// [`leader_grid`] for a leading track at `seconds`.
pub fn track_grid(
    leader: &Track,
    seconds: f64,
    speed: f64,
    play_seconds: f64,
) -> [Option<GridSection>; GRID_SECTIONS] {
    leader_grid(
        leader.bpm_at(seconds),
        leader.beat_at(seconds),
        speed,
        play_seconds,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn traces() -> (LiveTraces, [rtrb::Producer<LiveSlice>; NUM_DECKS]) {
        let mut tx = Vec::new();
        let mut rx = Vec::new();
        for _ in 0..NUM_DECKS {
            let (p, c) = rtrb::RingBuffer::new(WINDOW * 2);
            tx.push(p);
            rx.push(c);
        }
        (
            LiveTraces::new(rx.try_into().ok().unwrap()),
            tx.try_into().ok().unwrap(),
        )
    }

    fn loud() -> LiveSlice {
        LiveSlice {
            peak: [0.5; 4],
            rms: [0.25; 4],
        }
    }

    #[test]
    fn full_scale_is_the_top_of_the_waveform() {
        let full = LiveSlice {
            peak: [1.0; 4],
            rms: [1.0; 4],
        };
        assert_eq!(bin(&full), [255; 8]);
        assert_eq!(bin(&LiveSlice::default()), [0; 8]);
        // A quarter of full scale reads half way up, as on a track.
        assert_eq!(bin(&loud())[7], 128);
    }

    #[test]
    fn the_window_ends_at_the_play_line() {
        let (mut live, mut tx) = traces();
        for _ in 0..500 {
            tx[1].push(loud()).unwrap();
        }
        live.poll([false, true, false, false]);
        let w = live.window(1, Instant::now()).unwrap();
        assert_eq!(w.levels[0].len(), WINDOW);
        assert!((w.play_seconds - WINDOW as f64 / WAVEFORM_RATE).abs() < 1e-9);
        // Half a second of sound at the end, silence before it.
        assert_ne!(w.levels[0][WINDOW - 1], [0; 8]);
        assert_ne!(w.levels[0][WINDOW - 500], [0; 8]);
        assert_eq!(w.levels[0][WINDOW - 501], [0; 8]);
        assert!(live.window(0, Instant::now()).is_none());
    }

    #[test]
    fn a_deck_that_stops_playing_live_is_cleared() {
        let (mut live, mut tx) = traces();
        tx[0].push(loud()).unwrap();
        live.poll([true, false, false, false]);
        assert!(live.window(0, Instant::now()).is_some());
        tx[0].push(loud()).unwrap();
        live.poll([false; NUM_DECKS]);
        assert!(live.window(0, Instant::now()).is_none());
    }

    #[test]
    fn the_play_line_moves_on_smoothly_and_never_past_the_sound() {
        let (mut live, mut tx) = traces();
        let start = Instant::now();
        let mut last = 0.0;
        // Buffers of 10 ms, drawn at 60 frames a second.
        let mut sent = 0;
        for frame in 0..120 {
            let now = start + Duration::from_secs_f64(frame as f64 / 60.0);
            while (sent as f64) < (frame as f64 / 60.0) * 1000.0 {
                for _ in 0..10 {
                    tx[2].push(loud()).unwrap();
                }
                sent += 10;
            }
            live.poll([false, false, true, false]);
            if let Some(w) = live.window(2, now) {
                // Whatever is drawn ends at the play line.
                let into = w.play_seconds - WINDOW as f64 / WAVEFORM_RATE;
                assert!((0.0..0.001).contains(&into), "frame {frame}: {into}");
                let shown = live.decks[2].received as f64; // slices
                assert!(shown >= last);
                last = shown;
            }
        }
    }

    #[test]
    fn the_leaders_beats_line_up_on_the_live_deck() {
        // The leader is on beat 20, a bar line: so is the live play line.
        let g = leader_grid(120.0, 20.0, 1.0, 8.192)[0].unwrap();
        assert_eq!(g.bpm, 120.0);
        let beats_at_play = (8.192 - g.seconds) * g.bpm / 60.0;
        assert!((beats_at_play - 20.0).abs() < 1e-9);
        // Sped up, the beats come faster.
        let fast = leader_grid(120.0, 20.0, 1.08, 8.192)[0].unwrap();
        assert!((fast.bpm - 129.6).abs() < 1e-9);
        // No tempo, no grid.
        assert!(leader_grid(0.0, 20.0, 1.0, 8.192)[0].is_none());
    }
}
