//! Sample rate conversion for encoders that need a rate the engine may not
//! run at (Opus only takes 48 kHz; MP3 tops out at 48 kHz).
//!
//! A polyphase windowed-sinc resampler for the fixed ratio between two
//! rates: for each output frame, a set of taps picked by where it falls
//! between two input frames. Plenty for a stream: about 100 dB of stop-band
//! rejection, flat to 90% of the lower rate's half.

/// Taps per phase when going up; more when going down, so the filter cuts
/// at the lower rate.
const TAPS: usize = 32;

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// Converts interleaved stereo from one rate to another.
pub struct Resampler {
    /// Output frames per `down` input frames.
    up: usize,
    down: usize,
    taps: usize,
    /// `up` phases of `taps` coefficients.
    filters: Vec<f32>,
    /// Input frames not used up yet, the oldest first.
    history: Vec<[f32; 2]>,
    /// Where the next output frame falls, in `1 / up` steps from the start of
    /// `history`.
    position: usize,
}

impl Resampler {
    /// None when the rates are the same: nothing to do.
    pub fn new(from: u32, to: u32) -> Option<Self> {
        if from == to || from == 0 || to == 0 {
            return None;
        }
        let g = gcd(from as u64, to as u64);
        let (up, down) = ((to as u64 / g) as usize, (from as u64 / g) as usize);
        // Cut a little under the lower rate's Nyquist frequency.
        let cutoff = 0.9 * (to.min(from) as f64 / from as f64);
        let taps = (TAPS as f64 / cutoff.min(1.0)).ceil() as usize;
        let half = taps as f64 / 2.0;
        let mut filters = vec![0.0f32; up * taps];
        for phase in 0..up {
            let frac = phase as f64 / up as f64;
            let row = &mut filters[phase * taps..(phase + 1) * taps];
            let mut sum = 0.0;
            for (j, c) in row.iter_mut().enumerate() {
                // Tap j weighs the input frame j - half + 1 from the output's
                // left neighbour; t is its distance from the output.
                let t = j as f64 - half + 1.0 - frac;
                let x = std::f64::consts::PI * t * cutoff;
                let sinc = if x.abs() < 1e-12 { 1.0 } else { x.sin() / x };
                // Blackman window over the span.
                let w = {
                    let n = (t + half) / taps as f64;
                    0.42 - 0.5 * (std::f64::consts::TAU * n).cos()
                        + 0.08 * (2.0 * std::f64::consts::TAU * n).cos()
                };
                let v = sinc * w.max(0.0);
                *c = v as f32;
                sum += v;
            }
            for c in row.iter_mut() {
                *c = (*c as f64 / sum) as f32;
            }
        }
        Some(Self {
            up,
            down,
            taps,
            filters,
            history: vec![[0.0; 2]; taps],
            position: 0,
        })
    }

    /// Frames of delay the filter adds, in input frames.
    pub fn latency(&self) -> usize {
        self.taps / 2
    }

    /// Resamples interleaved stereo `input`, appending to `out`.
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        self.history
            .extend(input.as_chunks::<2>().0.iter().copied());
        // Output frames need `taps` input frames from where they fall.
        loop {
            let base = self.position / self.up;
            if base + self.taps > self.history.len() {
                break;
            }
            let phase = self.position % self.up;
            let row = &self.filters[phase * self.taps..(phase + 1) * self.taps];
            let mut y = [0.0f32; 2];
            for (c, f) in row.iter().zip(&self.history[base..base + self.taps]) {
                y[0] += c * f[0];
                y[1] += c * f[1];
            }
            out.extend(y);
            self.position += self.down;
        }
        let used = (self.position / self.up).min(self.history.len());
        self.history.drain(..used);
        self.position -= used * self.up;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: f64, freq: f64, seconds: f64) -> Vec<f32> {
        (0..(rate * seconds) as usize)
            .flat_map(|i| {
                let v = (0.5 * (std::f64::consts::TAU * freq * i as f64 / rate).sin()) as f32;
                [v, v]
            })
            .collect()
    }

    /// Amplitude of `freq` in the left channel of `x`.
    fn level(x: &[f32], rate: f64, freq: f64) -> f64 {
        let left: Vec<f32> = x.iter().step_by(2).copied().collect();
        let left = &left[left.len() / 4..];
        let w = std::f64::consts::TAU * freq / rate;
        let (re, im) = left
            .iter()
            .enumerate()
            .fold((0.0, 0.0), |(re, im), (i, &v)| {
                (
                    re + v as f64 * (w * i as f64).cos(),
                    im + v as f64 * (w * i as f64).sin(),
                )
            });
        2.0 * (re * re + im * im).sqrt() / left.len() as f64
    }

    #[test]
    fn converts_44_1_to_48_khz_keeping_the_tone() {
        let mut r = Resampler::new(44_100, 48_000).unwrap();
        let input = tone(44_100.0, 1000.0, 1.0);
        let mut out = Vec::new();
        // In uneven pieces, as the audio thread hands them over.
        for chunk in input.chunks(2 * 333) {
            r.process(chunk, &mut out);
        }
        let frames = out.len() / 2;
        assert!((frames as i64 - 48_000).abs() < 64, "{frames}");
        let a = level(&out, 48_000.0, 1000.0);
        assert!((a - 0.5).abs() < 0.01, "{a}");
    }

    #[test]
    fn halves_96_khz_and_drops_what_48_khz_cannot_carry() {
        let mut r = Resampler::new(96_000, 48_000).unwrap();
        let mut out = Vec::new();
        r.process(&tone(96_000.0, 30_000.0, 0.5), &mut out);
        // 30 kHz can't exist at 48 kHz; it must not fold down to 18 kHz.
        assert!(level(&out, 48_000.0, 18_000.0) < 0.001);
        let mut out = Vec::new();
        let mut r = Resampler::new(96_000, 48_000).unwrap();
        r.process(&tone(96_000.0, 1000.0, 0.5), &mut out);
        assert!((level(&out, 48_000.0, 1000.0) - 0.5).abs() < 0.01);
    }

    #[test]
    fn same_rate_needs_nothing() {
        assert!(Resampler::new(48_000, 48_000).is_none());
    }
}
