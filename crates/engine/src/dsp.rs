//! Filters used by the mixer and the analyser.

use std::f64::consts::PI;

/// A stereo biquad (RBJ cookbook), transposed direct form II.
#[derive(Clone, Copy, Debug)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: [f32; 2],
    z2: [f32; 2],
}

impl Default for Biquad {
    fn default() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: [0.0; 2],
            z2: [0.0; 2],
        }
    }
}

impl Biquad {
    pub fn lowpass(sample_rate: f64, freq: f64, q: f64) -> Self {
        let mut f = Self::default();
        f.set_lowpass(sample_rate, freq, q);
        f
    }

    pub fn highpass(sample_rate: f64, freq: f64, q: f64) -> Self {
        let mut f = Self::default();
        f.set_highpass(sample_rate, freq, q);
        f
    }

    /// Changes the coefficients, keeping the state so a sweep does not click.
    pub fn set_lowpass(&mut self, sample_rate: f64, freq: f64, q: f64) {
        let (cos, alpha) = Self::prewarp(sample_rate, freq, q);
        self.set(
            (1.0 - cos) / 2.0,
            1.0 - cos,
            (1.0 - cos) / 2.0,
            1.0 + alpha,
            -2.0 * cos,
            1.0 - alpha,
        );
    }

    pub fn set_highpass(&mut self, sample_rate: f64, freq: f64, q: f64) {
        let (cos, alpha) = Self::prewarp(sample_rate, freq, q);
        self.set(
            (1.0 + cos) / 2.0,
            -(1.0 + cos),
            (1.0 + cos) / 2.0,
            1.0 + alpha,
            -2.0 * cos,
            1.0 - alpha,
        );
    }

    /// A bell boosting or cutting `db` around `freq`.
    pub fn set_peaking(&mut self, sample_rate: f64, freq: f64, q: f64, db: f64) {
        let (cos, alpha) = Self::prewarp(sample_rate, freq, q);
        let a = 10f64.powf(db / 40.0);
        self.set(
            1.0 + alpha * a,
            -2.0 * cos,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * cos,
            1.0 - alpha / a,
        );
    }

    fn prewarp(sample_rate: f64, freq: f64, q: f64) -> (f64, f64) {
        let w = 2.0 * PI * freq.clamp(10.0, sample_rate * 0.45) / sample_rate;
        (w.cos(), w.sin() / (2.0 * q))
    }

    fn set(&mut self, b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) {
        self.b0 = (b0 / a0) as f32;
        self.b1 = (b1 / a0) as f32;
        self.b2 = (b2 / a0) as f32;
        self.a1 = (a1 / a0) as f32;
        self.a2 = (a2 / a0) as f32;
    }

    #[inline]
    pub fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        let mut y = [0.0; 2];
        for c in 0..2 {
            y[c] = self.b0 * x[c] + self.z1[c];
            self.z1[c] = self.b1 * x[c] - self.a1 * y[c] + self.z2[c];
            self.z2[c] = self.b2 * x[c] - self.a2 * y[c];
        }
        y
    }
}

/// Fourth-order Linkwitz-Riley filter: two cascaded Butterworth biquads.
#[derive(Clone, Copy, Debug, Default)]
pub struct Lr4([Biquad; 2]);

impl Lr4 {
    pub fn lowpass(sample_rate: f64, freq: f64) -> Self {
        let f = Biquad::lowpass(sample_rate, freq, std::f64::consts::FRAC_1_SQRT_2);
        Self([f, f])
    }

    pub fn highpass(sample_rate: f64, freq: f64) -> Self {
        let f = Biquad::highpass(sample_rate, freq, std::f64::consts::FRAC_1_SQRT_2);
        Self([f, f])
    }

    /// Changes the cutoff, keeping the state so a sweep does not click.
    pub fn set_lowpass(&mut self, sample_rate: f64, freq: f64) {
        for b in &mut self.0 {
            b.set_lowpass(sample_rate, freq, std::f64::consts::FRAC_1_SQRT_2);
        }
    }

    pub fn set_highpass(&mut self, sample_rate: f64, freq: f64) {
        for b in &mut self.0 {
            b.set_highpass(sample_rate, freq, std::f64::consts::FRAC_1_SQRT_2);
        }
    }

    #[inline]
    pub fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        let y = self.0[0].process(x);
        self.0[1].process(y)
    }
}

/// Crossover frequencies of the 3-band EQ, as in Mixxx's default isolator.
pub const EQ_LOW_HZ: f64 = 246.0;
pub const EQ_HIGH_HZ: f64 = 2484.0;

/// Three-band isolator EQ built from two Linkwitz-Riley crossovers. With every
/// gain at 1 the bands sum to a flat magnitude response, and a gain of 0 kills
/// a band completely.
#[derive(Clone, Copy, Debug)]
pub struct Isolator {
    low: Lr4,
    rest: Lr4,
    mid: Lr4,
    high: Lr4,
    // Puts the low band through the same phase shift as the upper crossover.
    low_ap: (Lr4, Lr4),
}

impl Isolator {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            low: Lr4::lowpass(sample_rate, EQ_LOW_HZ),
            rest: Lr4::highpass(sample_rate, EQ_LOW_HZ),
            mid: Lr4::lowpass(sample_rate, EQ_HIGH_HZ),
            high: Lr4::highpass(sample_rate, EQ_HIGH_HZ),
            low_ap: (
                Lr4::lowpass(sample_rate, EQ_HIGH_HZ),
                Lr4::highpass(sample_rate, EQ_HIGH_HZ),
            ),
        }
    }

    /// Splits a frame into (low, mid, high).
    #[inline]
    pub fn split(&mut self, x: [f32; 2]) -> [[f32; 2]; 3] {
        let lo = self.low.process(x);
        let (a, b) = (self.low_ap.0.process(lo), self.low_ap.1.process(lo));
        let lo = [a[0] + b[0], a[1] + b[1]];
        let rest = self.rest.process(x);
        [lo, self.mid.process(rest), self.high.process(rest)]
    }

    #[inline]
    pub fn process(&mut self, x: [f32; 2], gains: [f32; 3]) -> [f32; 2] {
        let [lo, mid, hi] = self.split(x);
        let mut y = [0.0; 2];
        for c in 0..2 {
            y[c] = lo[c] * gains[0] + mid[c] * gains[1] + hi[c] * gains[2];
        }
        y
    }
}

/// The deck's quick filter knob: low-pass below the centre, high-pass above it.
#[derive(Clone, Copy, Debug)]
pub struct SweepFilter {
    sample_rate: f64,
    knob: f64,
    lp: Biquad,
    hp: Biquad,
}

impl SweepFilter {
    pub fn new(sample_rate: f64) -> Self {
        let mut f = Self {
            sample_rate,
            knob: -1.0,
            lp: Biquad::default(),
            hp: Biquad::default(),
        };
        f.set(0.5);
        f
    }

    /// `knob` is 0..1 with 0.5 meaning off.
    pub fn set(&mut self, knob: f64) {
        if (knob - self.knob).abs() < 1e-6 {
            return;
        }
        self.knob = knob;
        // Exponential sweeps: 20 kHz down to 60 Hz, and 20 Hz up to 10 kHz.
        let lp = 60.0 * (20_000.0f64 / 60.0).powf((knob * 2.0).min(1.0));
        let hp = 20.0 * (10_000.0f64 / 20.0).powf(((knob - 0.5) * 2.0).max(0.0));
        let q = 0.85;
        self.lp.set_lowpass(self.sample_rate, lp, q);
        self.hp.set_highpass(self.sample_rate, hp, q);
    }

    pub fn active(&self) -> bool {
        (self.knob - 0.5).abs() > 0.01
    }

    #[inline]
    pub fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        if self.knob < 0.49 {
            self.lp.process(x)
        } else if self.knob > 0.51 {
            self.hp.process(x)
        } else {
            x
        }
    }
}

/// The channel of PlayDifferently's Model 1 mixer, in place of a three-band
/// EQ (see notes/model1-sculpt.md in the project files): "contour" high- and
/// low-pass filters that only cut, with no resonance, and a wide swept bell
/// between them. The filters are 4th-order Linkwitz-Riley, steep enough that
/// both fully closed leave almost nothing, as on the Model 1.
pub struct Sculpt {
    sample_rate: f64,
    knobs: [f64; 4],
    hp: Lr4,
    lp: Lr4,
    bell: Biquad,
    hp_on: bool,
    lp_on: bool,
    bell_on: bool,
}

/// The sculpt frequency at `knob` (0..1): exponential between the Model 1's
/// markings.
pub fn sculpt_freq(knob: f64) -> f64 {
    const MARKS: [(f64, f64); 5] = [
        (0.0, 70.0),
        (1.0 / 6.0, 100.0),
        (0.5, 500.0),
        (5.0 / 6.0, 2_500.0),
        (1.0, 7_000.0),
    ];
    let k = knob.clamp(0.0, 1.0);
    let i = MARKS.iter().rposition(|m| m.0 <= k).unwrap_or(0).min(3);
    let ((k0, f0), (k1, f1)) = (MARKS[i], MARKS[i + 1]);
    f0 * (f1 / f0).powf((k - k0) / (k1 - k0))
}

impl Sculpt {
    pub fn new(sample_rate: f64) -> Self {
        let mut s = Self {
            sample_rate,
            knobs: [f64::NAN; 4],
            hp: Lr4::default(),
            lp: Lr4::default(),
            bell: Biquad::default(),
            hp_on: false,
            lp_on: false,
            bell_on: false,
        };
        s.set(0.0, 0.5, 0.5, 1.0);
        s
    }

    /// The knobs, all 0..1, calibrated to the Model 1's panel markings:
    /// `hpf` (0 flat, up to 1 kHz), the sculpt band's `freq` (70 Hz, 100 Hz
    /// at a sixth, 500 Hz at the centre, 2.5 kHz at five sixths, 7 kHz) and
    /// `gain` (−20 dB to +8 dB, evenly spaced each side of 0 at the centre),
    /// and `lpf` (1 flat, down to 500 Hz).
    pub fn set(&mut self, hpf: f64, freq: f64, gain: f64, lpf: f64) {
        let knobs = [hpf, freq, gain, lpf];
        if knobs == self.knobs {
            return;
        }
        self.knobs = knobs;
        let sr = self.sample_rate;
        self.hp_on = hpf > 0.005;
        if self.hp_on {
            self.hp.set_highpass(sr, 20.0 * 50f64.powf(hpf));
        }
        self.lp_on = lpf < 0.995;
        if self.lp_on {
            self.lp.set_lowpass(sr, 500.0 * 40f64.powf(lpf));
        }
        let db = if gain < 0.5 {
            -20.0 * (1.0 - gain * 2.0)
        } else {
            8.0 * (gain * 2.0 - 1.0)
        };
        self.bell_on = db.abs() > 0.05;
        if self.bell_on {
            // A wide bell (Q 0.5, as DJ TechTools' Model 1 recipe uses).
            self.bell.set_peaking(sr, sculpt_freq(freq), 0.5, db);
        }
    }

    #[inline]
    pub fn process(&mut self, mut x: [f32; 2]) -> [f32; 2] {
        if self.hp_on {
            x = self.hp.process(x);
        }
        if self.bell_on {
            x = self.bell.process(x);
        }
        if self.lp_on {
            x = self.lp.process(x);
        }
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RMS of a sine at `freq` after the sculpt, skipping the first 0.1 s.
    fn sculpted(freq: f32, knobs: [f64; 4]) -> f32 {
        let sr = 44_100.0;
        let mut s = Sculpt::new(sr as f64);
        s.set(knobs[0], knobs[1], knobs[2], knobs[3]);
        let mut energy = 0.0f32;
        for i in 0..44_100 {
            let x = (i as f32 / sr * freq * std::f32::consts::TAU).sin();
            let y = s.process([x, x]);
            if i >= 4_410 {
                energy += y[0] * y[0];
            }
        }
        (energy / 39_690.0).sqrt()
    }

    #[test]
    fn sculpt_filters_and_band() {
        let flat = std::f32::consts::FRAC_1_SQRT_2;
        // Flat at rest.
        for f in [50.0, 1_000.0, 10_000.0] {
            assert!((sculpted(f, [0.0, 0.5, 0.5, 1.0]) - flat).abs() < 0.01);
        }
        // Half-way high-pass (about 141 Hz) takes out the sub, keeps the rest.
        assert!(sculpted(30.0, [0.5, 0.5, 0.5, 1.0]) < flat * 0.05);
        assert!((sculpted(2_000.0, [0.5, 0.5, 0.5, 1.0]) - flat).abs() < 0.05);
        // Half-way low-pass (about 3.2 kHz) the other way round.
        assert!(sculpted(15_000.0, [0.0, 0.5, 0.5, 0.5]) < flat * 0.05);
        assert!((sculpted(300.0, [0.0, 0.5, 0.5, 0.5]) - flat).abs() < 0.05);
        // Both closed leave almost nothing.
        assert!(sculpted(700.0, [1.0, 0.5, 0.5, 0.0]) < flat * 0.25);
        // The band at 500 Hz: a 20 dB cut, an 8 dB boost, and little effect
        // far away.
        let cut = sculpted(500.0, [0.0, 0.5, 0.0, 1.0]) / flat;
        assert!((cut - 0.1).abs() < 0.01, "{cut}");
        let boost = sculpted(500.0, [0.0, 0.5, 1.0, 1.0]) / flat;
        assert!((boost - 10f32.powf(8.0 / 20.0)).abs() < 0.05, "{boost}");
        assert!((sculpted(10_000.0, [0.0, 0.5, 0.0, 1.0]) - flat).abs() < 0.05);
        // The frequency knob's markings.
        for (k, f) in [
            (0.0, 70.0),
            (1.0 / 6.0, 100.0),
            (0.5, 500.0),
            (5.0 / 6.0, 2_500.0),
            (1.0, 7_000.0),
        ] {
            assert!((sculpt_freq(k) - f).abs() < 1e-6, "{k}");
        }
    }

    /// RMS of a sine at `freq` after the EQ, skipping the first 0.1 s.
    fn rms_after(freq: f32, gains: [f32; 3]) -> f32 {
        let sr = 44_100.0;
        let mut eq = Isolator::new(sr as f64);
        let mut energy = 0.0f32;
        for i in 0..44_100 {
            let x = (i as f32 / sr * freq * std::f32::consts::TAU).sin();
            let y = eq.process([x, x], gains);
            if i >= 4_410 {
                energy += y[0] * y[0];
            }
        }
        (energy / 39_690.0).sqrt()
    }

    #[test]
    fn isolator_at_unity_is_flat() {
        for freq in [40.0, 150.0, 246.0, 600.0, 2484.0, 6000.0, 12000.0] {
            let db = 20.0 * (rms_after(freq, [1.0; 3]) / std::f32::consts::FRAC_1_SQRT_2).log10();
            assert!(db.abs() < 0.25, "{freq} Hz: {db} dB");
        }
    }

    #[test]
    fn kills_remove_their_band() {
        assert!(rms_after(50.0, [0.0, 1.0, 1.0]) < 0.02);
        assert!(rms_after(800.0, [1.0, 0.0, 1.0]) < 0.08);
        assert!(rms_after(10_000.0, [1.0, 1.0, 0.0]) < 0.02);
    }
}

/// Makes the current thread's floating point treat denormal numbers as zero.
/// Feedback effects (reverb, echo, phaser) decay towards zero through
/// denormals, which many CPUs handle very slowly: a fading tail could then
/// cost many times its usual CPU. Called at the start of every audio buffer,
/// as Mixxx does on its audio thread.
pub fn flush_denormals() {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: only sets the MXCSR flush-to-zero (bit 15) and
    // denormals-are-zero (bit 6) flags of this thread.
    unsafe {
        let mut csr: u32 = 0;
        std::arch::asm!("stmxcsr [{}]", in(reg) &mut csr, options(nostack));
        csr |= 0x8040;
        std::arch::asm!("ldmxcsr [{}]", in(reg) &csr, options(nostack, readonly));
    }
    #[cfg(target_arch = "aarch64")]
    // SAFETY: only sets the FPCR flush-to-zero flag (bit 24) of this thread.
    unsafe {
        let mut fpcr: u64;
        std::arch::asm!("mrs {}, fpcr", out(reg) fpcr, options(nomem, nostack));
        fpcr |= 1 << 24;
        std::arch::asm!("msr fpcr, {}", in(reg) fpcr, options(nomem, nostack));
    }
}

#[cfg(test)]
mod denormal_tests {
    #[test]
    fn denormals_flush_to_zero() {
        super::flush_denormals();
        let tiny = std::hint::black_box(f32::MIN_POSITIVE);
        let half = std::hint::black_box(0.5f32);
        assert_eq!(tiny * half, 0.0);
    }
}
