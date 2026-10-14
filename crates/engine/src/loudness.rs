//! Track loudness for auto gain: EBU R128 / ITU-R BS.1770 integrated
//! loudness, measured during analysis, and the sample peak.
//!
//! The meter K-weights the signal (a high shelf for the head, then a high
//! pass), takes its power in 400 ms blocks every 100 ms, and averages the
//! blocks that pass two gates: above -70 LUFS, then above 10 LU below the
//! average of those. Silence and quiet intros so don't pull a track's
//! loudness down.

/// How loud a track is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Loudness {
    /// Integrated loudness in LUFS.
    pub lufs: f32,
    /// The loudest sample, as a linear level (1 = 0 dBFS).
    pub peak: f32,
}

/// Highest auto gain boost and cut, in dB.
const MAX_BOOST_DB: f32 = 12.0;
const MAX_CUT_DB: f32 = 24.0;

impl Loudness {
    /// The gain that brings this track to `target` LUFS. Quiet tracks are
    /// only boosted until their peak reaches `ceiling` (a linear level), so
    /// auto gain alone never drives a track into the master limiter.
    pub fn gain(&self, target: f32, ceiling: f32) -> f32 {
        let db = (target - self.lufs).clamp(-MAX_CUT_DB, MAX_BOOST_DB);
        let gain = 10f32.powf(db / 20.0);
        if gain > 1.0 && self.peak > 0.0 {
            gain.min(ceiling / self.peak).max(1.0)
        } else {
            gain
        }
    }
}

/// A biquad on two channels, in f64: the K-weighting high pass sits at
/// 38 Hz, where f32 coefficients lose precision at high sample rates.
#[derive(Clone, Copy)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    z: [[f64; 2]; 2],
}

impl Biquad {
    fn process(&mut self, x: f64, c: usize) -> f64 {
        let y = self.b[0] * x + self.z[c][0];
        self.z[c][0] = self.b[1] * x - self.a[0] * y + self.z[c][1];
        self.z[c][1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

/// The two K-weighting stages of BS.1770 for any sample rate, as libebur128
/// derives them from the 48 kHz filters.
fn k_weighting(sample_rate: f64) -> [Biquad; 2] {
    use std::f64::consts::PI;
    let shelf = {
        let (f0, g, q) = (1681.974450955533, 3.999843853973347, 0.7071752369554196);
        let k = (PI * f0 / sample_rate).tan();
        let vh = 10f64.powf(g / 20.0);
        let vb = vh.powf(0.4996667741545416);
        let a0 = 1.0 + k / q + k * k;
        Biquad {
            b: [
                (vh + vb * k / q + k * k) / a0,
                2.0 * (k * k - vh) / a0,
                (vh - vb * k / q + k * k) / a0,
            ],
            a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
            z: [[0.0; 2]; 2],
        }
    };
    let high_pass = {
        let (f0, q) = (38.13547087602444, 0.5003270373238773);
        let k = (PI * f0 / sample_rate).tan();
        let a0 = 1.0 + k / q + k * k;
        Biquad {
            b: [1.0, -2.0, 1.0],
            a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
            z: [[0.0; 2]; 2],
        }
    };
    [shelf, high_pass]
}

/// Measures loudness a frame at a time.
pub struct LoudnessMeter {
    filters: [Biquad; 2],
    /// Frames in 100 ms.
    step: usize,
    /// Summed power of each finished 100 ms step, both channels.
    steps: Vec<f64>,
    current: f64,
    in_step: usize,
    peak: f32,
}

impl LoudnessMeter {
    pub fn new(sample_rate: f64) -> Self {
        let sample_rate = sample_rate.max(1.0);
        LoudnessMeter {
            filters: k_weighting(sample_rate),
            step: ((sample_rate / 10.0).round() as usize).max(1),
            steps: Vec::new(),
            current: 0.0,
            in_step: 0,
            peak: 0.0,
        }
    }

    pub fn push(&mut self, frame: [f32; 2]) {
        for (c, &x) in frame.iter().enumerate() {
            self.peak = self.peak.max(x.abs());
            let y = self.filters[0].process(x as f64, c);
            let y = self.filters[1].process(y, c);
            self.current += y * y;
        }
        self.in_step += 1;
        if self.in_step == self.step {
            self.steps.push(self.current);
            self.current = 0.0;
            self.in_step = 0;
        }
    }

    /// The loudness of everything pushed, or None if it is shorter than a
    /// block or silent.
    pub fn finish(&self) -> Option<Loudness> {
        let block_frames = (4 * self.step) as f64;
        let blocks: Vec<f64> = self
            .steps
            .windows(4)
            .map(|w| w.iter().sum::<f64>() / block_frames)
            .collect();
        let lufs = |power: f64| -0.691 + 10.0 * power.log10();
        let mean_above = |gate: f64| {
            let (sum, n) = blocks
                .iter()
                .filter(|&&p| p > 0.0 && lufs(p) > gate)
                .fold((0.0, 0usize), |(s, n), &p| (s + p, n + 1));
            (n > 0).then(|| sum / n as f64)
        };
        let ungated = mean_above(-70.0)?;
        let integrated = mean_above(lufs(ungated) - 10.0)?;
        Some(Loudness {
            lufs: lufs(integrated) as f32,
            peak: self.peak,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measure(sample_rate: f64, seconds: f64, f: impl Fn(f64) -> [f32; 2]) -> Option<Loudness> {
        let mut m = LoudnessMeter::new(sample_rate);
        for i in 0..(seconds * sample_rate) as usize {
            m.push(f(i as f64 / sample_rate));
        }
        m.finish()
    }

    #[test]
    fn full_scale_1k_sine_reads_minus_3_lufs() {
        // EBU Tech 3341: a 0 dBFS 997 Hz sine in one channel is -3.01 LUFS.
        for sr in [44_100.0, 48_000.0, 96_000.0] {
            let l = measure(sr, 5.0, |t| {
                let v = (std::f64::consts::TAU * 997.0 * t).sin() as f32;
                [v, 0.0]
            })
            .unwrap();
            assert!((l.lufs + 3.01).abs() < 0.05, "{sr}: {}", l.lufs);
            assert!((l.peak - 1.0).abs() < 1e-3);
        }
    }

    #[test]
    fn gates_out_silence() {
        // Tech 3341 case 3 idea: loud passages with silence between read as
        // the loud passages alone.
        let tone = |t: f64| {
            let v = 0.1 * (std::f64::consts::TAU * 1000.0 * t).sin() as f32;
            [v, v]
        };
        let steady = measure(48_000.0, 10.0, tone).unwrap();
        let gapped = measure(
            48_000.0,
            30.0,
            |t| if t < 10.0 { tone(t) } else { [0.0; 2] },
        )
        .unwrap();
        assert!((steady.lufs - gapped.lufs).abs() < 0.1);
        assert!(measure(48_000.0, 2.0, |_| [0.0; 2]).is_none());
    }

    #[test]
    fn gain_reaches_target_but_caps_boost_at_peak() {
        let loud = Loudness {
            lufs: -8.0,
            peak: 1.0,
        };
        assert!((20.0 * loud.gain(-14.0, 0.97).log10() + 6.0).abs() < 1e-3);
        // Needs +10 dB, but its peak allows only +6 dB.
        let quiet = Loudness {
            lufs: -24.0,
            peak: 0.5,
        };
        assert!((quiet.gain(-14.0, 1.0) - 2.0).abs() < 1e-4);
        // Never cuts a quiet track because of its peak.
        let peaky = Loudness {
            lufs: -16.0,
            peak: 1.0,
        };
        assert_eq!(peaky.gain(-14.0, 0.97), 1.0);
    }
}
