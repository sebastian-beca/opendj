//! Macro lanes: a second view of an instrument or Groove Box deck's
//! scrolling waveform, its knobs drawn as automation lines up to the play
//! line, one lane each.
//!
//! An instrument deck shows the eight knobs of the page its deck shows; a
//! Groove Box deck its eight tracks' levels, kick on top as in the step
//! lanes. The values are noted each frame on the deck's live clock and
//! kept for a few seconds.

use std::collections::VecDeque;

use opendj_control::{ControlHandle, ControlRegistry};
use opendj_engine::groove::{groove_group, ROWS};
use opendj_engine::NUM_DECKS;
use opendj_waveform::{Lanes, Mark, MarkKind, MAX_MARKS};

/// Lanes on the waveform, top first.
pub const LANES: usize = 8;
/// How opaque the sound is under the lanes.
const UNDER_LINES: f32 = 0.25;
/// Values are kept this long behind the play line, in seconds.
const KEEP_SECONDS: f64 = 10.0;
/// A value is noted at most this often, in seconds.
const STEP_SECONDS: f64 = 0.05;
/// A Groove Box track's level at its fader's top.
const LEVEL_TOP: f64 = 1.5;
/// The lit colour, Electric Lilac.
const LILAC: [f32; 4] = [0.714, 0.612, 1.0, 1.0];

/// The knobs a deck's lanes follow.
pub struct KnobHandles {
    /// `[Instrument1],parameterN`, all of them.
    instrument: Vec<ControlHandle>,
    /// Each deck's Groove Box tracks' levels.
    levels: [[ControlHandle; ROWS]; NUM_DECKS],
}

/// What a deck's lanes show: the instrument's knobs from `page` (eight to
/// a page), or the Groove Box's levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Instrument { page: usize },
    Groove,
}

impl KnobHandles {
    pub fn new(reg: &ControlRegistry) -> Self {
        let instrument = (1..)
            .map_while(|k| reg.get("[Instrument1]", &format!("parameter{k}")))
            .collect();
        let levels = std::array::from_fn(|d| {
            let g = groove_group(d);
            std::array::from_fn(|r| {
                reg.get(&g, &format!("row{}_level", r + 1))
                    .unwrap_or_else(|| panic!("missing control {g},row{}_level", r + 1))
            })
        });
        Self { instrument, levels }
    }

    /// The knobs' values now, from 0 to 1, top lane first. Past the
    /// instrument's last parameter, 0.
    pub fn values(&self, d: usize, source: Source) -> [f32; LANES] {
        match source {
            Source::Instrument { page } => std::array::from_fn(|i| {
                self.instrument
                    .get(page * LANES + i)
                    .map_or(0.0, |c| c.get().clamp(0.0, 1.0) as f32)
            }),
            Source::Groove => std::array::from_fn(|i| {
                self.levels[d]
                    .get(i)
                    .map_or(0.0, |c| (c.get() / LEVEL_TOP).clamp(0.0, 1.0) as f32)
            }),
        }
    }
}

#[derive(Default)]
struct Deck {
    source: Option<Source>,
    /// (clock, values), oldest first.
    seen: VecDeque<(f64, [f32; LANES])>,
}

#[derive(Default)]
pub struct MacroLanes {
    decks: [Deck; NUM_DECKS],
}

impl MacroLanes {
    /// Forgets a deck's values, as when its lanes are hidden.
    pub fn clear(&mut self, d: usize) {
        self.decks[d] = Deck::default();
    }

    /// Notes the deck's knobs at `at` on its live clock. Values from
    /// another source, or from before the clock went back, are dropped.
    pub fn note(&mut self, d: usize, source: Source, at: f64, values: [f32; LANES]) {
        let deck = &mut self.decks[d];
        if deck.source != Some(source) || deck.seen.back().is_some_and(|&(t, _)| t > at) {
            *deck = Deck {
                source: Some(source),
                seen: VecDeque::new(),
            };
        }
        // Too soon after the one before the last: the last moves up to
        // here instead, so values are noted about every STEP_SECONDS.
        let n = deck.seen.len();
        if n > 1 && at - deck.seen[n - 2].0 < STEP_SECONDS {
            deck.seen[n - 1] = (at, values);
        } else {
            deck.seen.push_back((at, values));
        }
        while deck.seen.len() > 2 && deck.seen[1].0 < at - KEEP_SECONDS {
            deck.seen.pop_front();
        }
    }

    /// The deck's lanes for a window starting at `start` on its clock and
    /// ending at the play line `at`, over `back` seconds to its left.
    pub fn lanes(&self, d: usize, start: f64, at: f64, back: f64, color: [f32; 4]) -> Lanes {
        let deck = &self.decks[d];
        let from = at - back;
        // The last value before the view, then each one in it.
        let first = deck.seen.iter().rposition(|&(t, _)| t <= from).unwrap_or(0);
        let mut marks = Vec::new();
        for i in 0..LANES {
            let mut points: Vec<(f64, f32)> = deck
                .seen
                .iter()
                .skip(first)
                .map(|&(t, v)| (t.max(from), v[i]))
                .collect();
            if let Some(&(t, v)) = points.last() {
                if t < at {
                    points.push((at, v));
                }
            }
            for w in simplify(&points, MAX_MARKS / LANES).windows(2) {
                let ((t0, v0), (t1, v1)) = (w[0], w[1]);
                if t1 > t0 {
                    marks.push(Mark {
                        start: t0 - start,
                        end: t1 - start,
                        lane: (LANES - 1 - i) as u32,
                        kind: MarkKind::Level(v0, v1),
                    });
                }
            }
        }
        Lanes {
            count: LANES as u32,
            tinted: 0,
            marks,
            trace: UNDER_LINES,
            color,
            lit: LILAC,
        }
    }
}

/// The line through `points` in at most `segments` straight pieces: the
/// points furthest from the line so far are kept first, so a sweep keeps
/// its shape and a knob left alone is one piece.
fn simplify(points: &[(f64, f32)], segments: usize) -> Vec<(f64, f32)> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let mut keep = vec![0, points.len() - 1];
    while keep.len() <= segments {
        // The point furthest (in value) from the piece it falls in.
        let mut best: Option<(f32, usize)> = None;
        for w in keep.windows(2) {
            let (a, b) = (points[w[0]], points[w[1]]);
            for (j, &(t, v)) in points.iter().enumerate().take(w[1]).skip(w[0] + 1) {
                let f = if b.0 > a.0 {
                    ((t - a.0) / (b.0 - a.0)) as f32
                } else {
                    0.0
                };
                let off = (v - (a.1 + (b.1 - a.1) * f)).abs();
                if best.is_none_or(|(o, _)| off > o) {
                    best = Some((off, j));
                }
            }
        }
        match best {
            Some((off, j)) if off > 0.002 => {
                let at = keep.partition_point(|&k| k < j);
                keep.insert(at, j);
            }
            _ => break,
        }
    }
    keep.into_iter().map(|k| points[k]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];

    fn levels(lanes: &Lanes, lane: u32) -> Vec<(f64, f64, f32, f32)> {
        lanes
            .marks
            .iter()
            .filter(|m| m.lane == lane)
            .map(|m| match m.kind {
                MarkKind::Level(a, b) => (m.start, m.end, a, b),
                k => panic!("not a level: {k:?}"),
            })
            .collect()
    }

    #[test]
    fn a_knob_left_alone_is_one_line_to_the_play_line() {
        let mut m = MacroLanes::default();
        let source = Source::Instrument { page: 0 };
        for f in 0..300 {
            m.note(0, source, 10.0 + f as f64 / 60.0, [0.25; LANES]);
        }
        let at = 15.0;
        let l = m.lanes(0, 7.0, at, 4.0, RED);
        assert_eq!(l.count, LANES as u32);
        // Top lane (knob 1) is row 7; one piece from 4 s back to the play line.
        let top = levels(&l, 7);
        assert_eq!(top.len(), 1, "{top:?}");
        let (s, e, a, b) = top[0];
        assert!((s - 4.0).abs() < 1e-9 && (e - 8.0).abs() < 1e-9, "{s} {e}");
        assert_eq!((a, b), (0.25, 0.25));
        assert_eq!(l.marks.len(), LANES);
    }

    #[test]
    fn a_sweep_keeps_its_shape_within_the_budget() {
        let mut m = MacroLanes::default();
        let source = Source::Groove;
        // Knob 3 sweeps up and down over 4 s; the rest stay put.
        for f in 0..240 {
            let t = f as f64 / 60.0;
            let mut v = [0.5; LANES];
            v[2] = (0.5 + 0.5 * (t * 3.0).sin()) as f32;
            m.note(1, source, t, v);
        }
        let l = m.lanes(1, 0.0, 4.0, 4.0, RED);
        assert!(l.marks.len() <= MAX_MARKS);
        let sweep = levels(&l, (LANES - 3) as u32);
        assert!(
            sweep.len() > 8 && sweep.len() <= MAX_MARKS / LANES,
            "{}",
            sweep.len()
        );
        // The pieces join up and follow the sine closely.
        for w in sweep.windows(2) {
            assert!((w[0].1 - w[1].0).abs() < 1e-9 && (w[0].3 - w[1].2).abs() < 1e-6);
        }
        for &(s, _, a, _) in &sweep {
            let want = 0.5 + 0.5 * (s * 3.0).sin();
            assert!((a as f64 - want).abs() < 0.05, "at {s}: {a} vs {want}");
        }
    }

    #[test]
    fn another_source_or_a_clock_going_back_starts_over() {
        let mut m = MacroLanes::default();
        m.note(0, Source::Instrument { page: 0 }, 1.0, [0.1; LANES]);
        m.note(0, Source::Instrument { page: 0 }, 2.0, [0.1; LANES]);
        m.note(0, Source::Instrument { page: 1 }, 3.0, [0.9; LANES]);
        let l = m.lanes(0, 0.0, 4.0, 4.0, RED);
        assert_eq!(levels(&l, 7), vec![(3.0, 4.0, 0.9, 0.9)]);
        m.note(0, Source::Instrument { page: 1 }, 0.5, [0.3; LANES]);
        let l = m.lanes(0, 0.0, 1.0, 4.0, RED);
        assert_eq!(levels(&l, 7), vec![(0.5, 1.0, 0.3, 0.3)]);
    }

    #[test]
    fn old_values_are_dropped() {
        let mut m = MacroLanes::default();
        for f in 0..(30 * 60) {
            m.note(2, Source::Groove, f as f64 / 60.0, [0.0; LANES]);
        }
        let kept = m.decks[2].seen.len() as f64;
        assert!(kept < 2.0 * (KEEP_SECONDS + 1.0) / STEP_SECONDS, "{kept}");
    }
}
