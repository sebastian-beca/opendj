//! Key lock and key shifting: playing a deck at any speed and in any key,
//! as Mixxx does with its RubberBand scaler. Here the time-stretcher is
//! Signalsmith Stretch.
//!
//! The deck's play position stays the position being heard. The stretcher
//! needs input ahead of that (its latency), so its read position runs ahead
//! by a fixed amount and is re-seeded whenever the deck jumps.

use signalsmith_stretch::Stretch;

use crate::engine::Frames;

/// Most source frames fed per output frame (fast forward while stretching).
const MAX_RATE: f64 = 4.0;

pub(crate) struct KeyShifter {
    stretch: Stretch,
    /// Interleaved stereo scratch buffers, sized ahead of time.
    input: Vec<f32>,
    output: Vec<f32>,
    /// Whether the stretcher holds audio continuing from the last buffer.
    active: bool,
    /// Where the next input frame is read, in source frames, unwrapped
    /// (it can run past a loop's end; reads wrap).
    read_pos: f64,
    /// Where the deck should be at the start of the next buffer if nothing jumps.
    expected_pos: f64,
}

impl KeyShifter {
    pub(crate) fn new(sample_rate: f64) -> Self {
        let stretch = Stretch::preset_default(2, sample_rate as u32);
        Self {
            stretch,
            input: vec![0.0; 2 * 8192],
            output: vec![0.0; 2 * 4096],
            active: false,
            read_pos: 0.0,
            expected_pos: 0.0,
        }
    }

    /// Forgets the audio in flight; the next render starts afresh.
    pub(crate) fn stop(&mut self) {
        self.active = false;
    }

    /// Renders `out.len()` frames of a deck playing forwards from source
    /// frame `pos` at `speed` (multiples of normal speed), shifted by
    /// `transpose` (a frequency factor: 1 keeps the original key).
    ///
    /// `ratio` is source frames per output frame at normal speed; `looping`
    /// is the active loop in source frames.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render<F: Frames + ?Sized>(
        &mut self,
        audio: &F,
        pos: f64,
        speed: f64,
        ratio: f64,
        transpose: f64,
        looping: Option<(f64, f64)>,
        out: &mut [[f32; 2]],
    ) {
        let frames = out.len();
        let step = speed * ratio;
        let in_latency = self.stretch.input_latency() as f64;
        let out_latency = self.stretch.output_latency() as f64;
        // How far ahead of the heard position the next input frame is.
        let lead = (in_latency + out_latency * speed) * ratio;
        let wrap = |x: f64| match looping {
            Some((a, b)) if x >= b && b > a => a + (x - b).rem_euclid(b - a),
            _ => x,
        };
        let tolerance = 4.0 * ratio + 1.0;
        let continuing = self.active
            && match looping {
                _ if (pos - self.expected_pos).abs() < tolerance => true,
                // The loop wrapped: carry the read position back with it.
                Some((a, b)) if (pos - (self.expected_pos - (b - a))).abs() < tolerance => {
                    self.read_pos -= b - a;
                    true
                }
                _ => false,
            };
        self.stretch.set_transpose_factor(transpose as f32, None);
        if !continuing {
            // Prime the stretcher so its output starts at `pos`.
            self.stretch.reset();
            let seed = in_latency as usize;
            let start = pos + out_latency * step;
            self.fill_input(audio, start, ratio, seed, &wrap);
            self.stretch.seek(&self.input[..seed * 2], speed);
            self.read_pos = start + seed as f64 * ratio;
            self.active = true;
        }
        let end = pos + step * frames as f64;
        let wanted = ((end + lead - self.read_pos) / ratio).round();
        let n_in = wanted.clamp(0.0, MAX_RATE * frames as f64) as usize;
        let start = self.read_pos;
        self.fill_input(audio, start, ratio, n_in, &wrap);
        self.read_pos += n_in as f64 * ratio;
        if self.output.len() < frames * 2 {
            self.output.resize(frames * 2, 0.0);
        }
        self.stretch
            .process(&self.input[..n_in * 2], &mut self.output[..frames * 2]);
        for (f, o) in out.iter_mut().enumerate() {
            *o = [self.output[2 * f], self.output[2 * f + 1]];
        }
        self.expected_pos = end;
    }

    fn fill_input<F: Frames + ?Sized>(
        &mut self,
        audio: &F,
        start: f64,
        ratio: f64,
        n: usize,
        wrap: &impl Fn(f64) -> f64,
    ) {
        if self.input.len() < n * 2 {
            self.input.resize(n * 2, 0.0);
        }
        audio.begin(start);
        for k in 0..n {
            let s = crate::engine::read(audio, wrap(start + k as f64 * ratio));
            self.input[2 * k] = s[0];
            self.input[2 * k + 1] = s[1];
        }
    }
}

/// The frequency factor for a deck: `semitones` of key shift, plus the
/// speed's own pitch change unless key lock holds the key.
pub(crate) fn transpose_factor(semitones: f64, speed: f64, keylock: bool) -> f64 {
    let shift = (semitones / 12.0).exp2();
    if keylock {
        shift
    } else {
        shift * speed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::Audio;

    /// The strongest frequency in `x` by a plain DFT scan.
    fn pitch(x: &[f32], sr: f64) -> f64 {
        (100..2000)
            .step_by(2)
            .map(|hz| {
                let w = 2.0 * std::f64::consts::PI * hz as f64 / sr;
                let (mut re, mut im) = (0.0, 0.0);
                for (i, &v) in x.iter().enumerate() {
                    re += v as f64 * (w * i as f64).cos();
                    im += v as f64 * (w * i as f64).sin();
                }
                (hz as f64, re * re + im * im)
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap()
            .0
    }

    fn tone(hz: f64, sr: f64, seconds: f64) -> Audio {
        let n = (sr * seconds) as usize;
        let frames: Vec<[f32; 2]> = (0..n)
            .map(|i| {
                let v = (2.0 * std::f64::consts::PI * hz * i as f64 / sr).sin() as f32 * 0.5;
                [v, v]
            })
            .collect();
        Audio::from_frames(sr as u32, frames)
    }

    /// Plays `audio` for `seconds` and returns the last half second, mono.
    fn play(audio: &Audio, speed: f64, transpose: f64, sr: f64) -> Vec<f32> {
        let mut k = KeyShifter::new(sr);
        let mut pos = 0.0;
        let mut out = vec![[0.0f32; 2]; 512];
        let mut all = Vec::new();
        for _ in 0..(sr as usize * 2 / 512) {
            k.render(audio, pos, speed, 1.0, transpose, None, &mut out);
            pos += speed * 512.0;
            all.extend(out.iter().map(|s| s[0]));
        }
        all.split_off(all.len() - sr as usize / 2)
    }

    #[test]
    fn output_lines_up_with_the_play_position() {
        // A click one second in must be heard one second in, at any speed.
        let sr = 48_000.0;
        let mut frames = vec![[0.0f32; 2]; sr as usize * 3];
        for f in &mut frames[48_000..48_200] {
            *f = [0.8, 0.8];
        }
        let audio = Audio::from_frames(sr as u32, frames);
        for speed in [1.0, 1.1] {
            let mut k = KeyShifter::new(sr);
            let (mut pos, mut heard) = (0.0, Vec::new());
            let mut out = vec![[0.0f32; 2]; 256];
            while pos < 1.5 * sr {
                k.render(&audio, pos, speed, 1.0, 1.0, None, &mut out);
                for s in &out {
                    heard.push((pos, s[0].abs()));
                    pos += speed;
                }
            }
            let at = heard.iter().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap().0;
            assert!(
                (at - 48_100.0).abs() < 0.01 * sr,
                "speed {speed}: click heard at {at}"
            );
        }
    }

    #[test]
    fn key_lock_keeps_the_pitch_and_shifts_move_it() {
        let sr = 48_000.0;
        let audio = tone(440.0, sr, 6.0);
        // Faster with key lock: same pitch.
        let p = pitch(
            &play(&audio, 1.08, transpose_factor(0.0, 1.08, true), sr),
            sr,
        );
        assert!((p - 440.0).abs() < 8.0, "{p}");
        // Two semitones up: 494 Hz.
        let p = pitch(&play(&audio, 1.0, transpose_factor(2.0, 1.0, true), sr), sr);
        assert!((p - 494.0).abs() < 8.0, "{p}");
        // Without key lock the speed raises the pitch too.
        let p = pitch(
            &play(&audio, 1.08, transpose_factor(0.0, 1.08, false), sr),
            sr,
        );
        assert!((p - 475.0).abs() < 8.0, "{p}");
    }
}
