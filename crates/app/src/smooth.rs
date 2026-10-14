//! Steady waveform motion from play positions that arrive with timing noise.
//!
//! The engine's position is exact, but when the UI reads it the answer jumps
//! a little with audio-callback and frame timing, a millisecond or two each
//! way, which shows as jitter when the waveform moves hundreds of pixels a
//! second. Like a video player's clock, this advances the drawn position by
//! the speed times a steady frame interval, and only eases it towards the
//! engine's position. Jumps (seeks, loops, cues) and speed changes (scratching,
//! starting and stopping) are followed at once.

use std::time::Instant;

/// Bigger differences from the engine are jumps, followed at once (seconds).
const JUMP: f64 = 0.04;
/// Share of the remaining difference corrected each frame.
const CORRECTION: f64 = 0.1;

#[derive(Default)]
pub struct SmoothClock {
    /// What was drawn last, the speed then, and when.
    last: Option<(f64, f64, Instant)>,
    /// Running estimate of the time between frames, in seconds.
    frame: Option<f64>,
}

impl SmoothClock {
    /// The position to draw at `now`, given the engine's `position` and
    /// `speed` (track seconds per second) for that moment.
    pub fn next(&mut self, position: f64, speed: f64, now: Instant) -> f64 {
        let shown = match self.last {
            Some((shown, last_speed, at)) if speed != 0.0 && (speed - last_speed).abs() < 0.02 => {
                let dt = now.saturating_duration_since(at).as_secs_f64();
                // Frames come at the display's rate, but timing a single
                // frame adds noise; step by whole average frame intervals (more
                // than one if frames were dropped), and let the average follow
                // the measured intervals slowly.
                let f = self.frame.unwrap_or(dt).max(1e-4);
                let step = f * (dt / f).round().max(1.0);
                self.frame = Some(f + (dt.clamp(0.5 * f, 1.5 * f) - f) * 0.05);
                let predicted = shown + speed * step;
                let error = position - predicted;
                if error.abs() > JUMP {
                    position
                } else {
                    predicted + error * CORRECTION
                }
            }
            _ => position,
        };
        self.last = Some((shown, speed, now));
        shown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn evens_out_noisy_timing() {
        let mut clock = SmoothClock::default();
        let start = Instant::now();
        let frame = 1.0 / 120.0;
        let mut previous: Option<f64> = None;
        for i in 0..600 {
            // The display shows frame i at a steady `vsync`, but the UI draws
            // it up to 2 ms early or late, and the engine's answer is off by up
            // to 1.5 ms: what an audio callback and a busy UI thread give.
            let vsync = 0.01 + i as f64 * frame;
            let wobble = [0.0, 0.002, -0.0015, 0.001, -0.002][i % 5];
            let now = start + Duration::from_secs_f64(vsync + wobble);
            let noise = [0.0015, -0.001, 0.0005, -0.0015][i % 4];
            let reported = 10.0 + vsync + wobble + noise;
            let shown = clock.next(reported, 1.0, now);
            if i > 120 {
                // Each frame moves almost exactly one frame's worth...
                let step = shown - previous.unwrap();
                assert!((step - frame).abs() < 0.0005, "frame {i}: step {step}");
                // ...and stays with the engine.
                let truth = 10.0 + vsync;
                assert!(
                    (shown - truth).abs() < 0.004,
                    "frame {i}: off by {}",
                    shown - truth
                );
            }
            previous = Some(shown);
        }
    }

    #[test]
    fn follows_jumps_and_stops_at_once() {
        let mut clock = SmoothClock::default();
        let t = Instant::now();
        let ms = |n: u64| t + Duration::from_millis(n);
        clock.next(5.0, 1.0, ms(0));
        clock.next(5.008, 1.0, ms(8));
        // A seek.
        assert_eq!(clock.next(42.0, 1.0, ms(16)), 42.0);
        // Stopped.
        assert_eq!(clock.next(42.5, 0.0, ms(24)), 42.5);
        // Scratching: the speed changes every frame.
        assert_eq!(clock.next(42.4, -1.3, ms(32)), 42.4);
        assert_eq!(clock.next(42.39, -0.4, ms(40)), 42.39);
    }
}
