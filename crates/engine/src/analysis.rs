//! Track analysis: the 3-band waveform summary, tempo, beat phase and key
//! (see [`crate::key`] and [`crate::keynet`]).
//!
//! The tempo detector works on an onset envelope (rises in bass energy and
//! overall level, 200 per second). It picks the strongest autocorrelation lag
//! in the tempo range (70 to 180 BPM unless set otherwise), then refines it to 0.01 BPM by folding the envelope
//! at each candidate period and keeping the one whose beats line up best.
//!
//! Live and older recordings drift, so a steady grid misses their beats. A
//! dynamic-programming beat tracker (Ellis, "Beat Tracking by Dynamic
//! Programming", 2007) then follows the beats one by one; where most of them
//! stray from the steady grid, the track gets a grid of sections whose
//! tempos are fitted to the tracked beats (see [`crate::beats`]).

use std::sync::atomic::{AtomicU64, Ordering};

use crate::beats::{BeatGrid, GridMarker};
use crate::dsp::Isolator;
use crate::key::{self, Detector, Key, KeyDetector};
use crate::keynet::Spectrogram;
use crate::loudness::{Loudness, LoudnessMeter};
use crate::track::{Audio, Waveform, WAVEFORM_RATE};

/// Onset envelope rate used for tempo and phase detection.
const ONSET_RATE: f64 = 200.0;

pub struct Analysis {
    pub waveform: Waveform,
    /// Detected key, None for silence.
    pub key: Option<Key>,
    /// The detector that found `key`.
    pub key_detector: Detector,
    pub bpm: f64,
    pub first_beat: f64,
    /// The beat grid: steady at `bpm` from `first_beat`, or sections at
    /// different tempos when the track drifts. Empty without a tempo.
    pub grid: BeatGrid,
    /// EBU R128 loudness and peak, for auto gain; None for silence.
    pub loudness: Option<Loudness>,
}

/// The tempo range tracks are analysed in, unless changed with
/// [`set_bpm_range`].
pub const DEFAULT_BPM_RANGE: (f64, f64) = (70.0, 180.0);
static BPM_RANGE: AtomicU64 = AtomicU64::new(pack_range(DEFAULT_BPM_RANGE));

const fn pack_range((lo, hi): (f64, f64)) -> u64 {
    ((lo as u64) << 32) | hi as u64
}

/// The tempo range tracks are analysed in, in whole BPM.
pub fn bpm_range() -> (f64, f64) {
    let v = BPM_RANGE.load(Ordering::Relaxed);
    ((v >> 32) as f64, (v & 0xffff_ffff) as f64)
}

/// Sets the tempo range for tracks analysed from now on. The top must be
/// nearly twice the bottom, so every tempo has a place in it.
pub fn set_bpm_range(lo: f64, hi: f64) {
    let lo = lo.clamp(40.0, 200.0).round();
    let hi = hi.round().max((lo * 1.9).ceil()).min(400.0);
    BPM_RANGE.store(pack_range((lo, hi)), Ordering::Relaxed);
}
const PHASE_BINS: usize = 64;

/// Per-slice accumulators: band peaks and sums of squares (low, mid, high, all).
#[derive(Clone, Copy, Default)]
struct Slice {
    peak: [f32; 4],
    power: [f32; 4],
    frames: u32,
}

impl Slice {
    fn rms(&self) -> [f32; 4] {
        let n = self.frames.max(1) as f32;
        self.power.map(|p| (p / n).sqrt())
    }
}

/// Analyses decoded audio. A tempo from the file's tags, if any, is used
/// instead of detection; the beat phase is always detected.
pub fn analyse(audio: &Audio, bpm_hint: Option<f64>) -> Analysis {
    let mut a = Analyser::new(audio.sample_rate, audio.len());
    for chunk in audio.decoded_chunks() {
        a.feed(chunk);
    }
    a.finish(bpm_hint)
}

/// The key of decoded `audio` by `detector`, for a track whose other
/// analysis is already known.
pub fn detect_key(audio: &Audio, detector: Detector) -> Option<Key> {
    let sr = audio.sample_rate.max(1) as f64;
    let standard = || {
        let mut eq = Isolator::new(sr);
        let mut key = KeyDetector::new(sr);
        for f in audio.decoded_chunks().flatten() {
            let [lo, mid, _] = eq.split(*f);
            key.push((lo[0] + lo[1] + mid[0] + mid[1]) * 0.5);
        }
        key.key()
    };
    match detector {
        Detector::Standard => standard(),
        Detector::Neural => {
            let mut s = Spectrogram::new(sr);
            for f in audio.decoded_chunks().flatten() {
                s.push((f[0] + f[1]) * 0.5);
            }
            s.key().or_else(standard)
        }
    }
}

/// Analyses audio a piece at a time as it is decoded, so the waveform can be
/// drawn while the rest of the track is still being read.
pub struct Analyser {
    bin_len: f64,
    slice_len: f64,
    eq: Isolator,
    key: KeyDetector,
    /// The neural detector's input, when it is the one chosen.
    neural: Option<Spectrogram>,
    loudness: LoudnessMeter,
    slices: Vec<Slice>,
    energy: Vec<[f32; 2]>,
    /// Frames fed so far.
    frames: usize,
}

impl Analyser {
    /// An analyser for audio of about `expected_frames` frames; the waveform
    /// spans that length from the start, so the part not analysed yet is flat.
    pub fn new(sample_rate: u32, expected_frames: usize) -> Self {
        let sr = sample_rate.max(1) as f64;
        let bin_len = sr / ONSET_RATE;
        let slice_len = sr / WAVEFORM_RATE;
        let slices = (expected_frames as f64 / slice_len).ceil().max(1.0) as usize;
        let bins = (expected_frames as f64 / bin_len).ceil().max(1.0) as usize;
        Analyser {
            bin_len,
            slice_len,
            eq: Isolator::new(sr),
            key: KeyDetector::new(sr),
            neural: (key::detector() == Detector::Neural).then(|| Spectrogram::new(sr)),
            loudness: LoudnessMeter::new(sr),
            slices: vec![Slice::default(); slices],
            energy: vec![[0.0; 2]; bins],
            frames: 0,
        }
    }

    /// Frames analysed so far.
    pub fn frames(&self) -> usize {
        self.frames
    }

    /// Analyses the next frames of the track.
    pub fn feed(&mut self, frames: &[[f32; 2]]) {
        for f in frames {
            let i = self.frames;
            self.frames += 1;
            let b = (i as f64 / self.bin_len) as usize;
            let s = (i as f64 / self.slice_len) as usize;
            // The decoder's length can be an estimate; grow past it.
            if b >= self.energy.len() {
                self.energy.resize(b + 1, [0.0; 2]);
            }
            if s >= self.slices.len() {
                self.slices.resize(s + 1, Slice::default());
            }
            self.loudness.push(*f);
            let [lo, mid, hi] = self.eq.split(*f);
            self.key.push((lo[0] + lo[1] + mid[0] + mid[1]) * 0.5);
            if let Some(n) = self.neural.as_mut() {
                n.push((f[0] + f[1]) * 0.5);
            }
            let bands = [lo, mid, hi, *f];
            let slice = &mut self.slices[s];
            for (k, x) in bands.iter().enumerate() {
                slice.peak[k] = slice.peak[k].max(x[0].abs().max(x[1].abs()));
                slice.power[k] += (x[0] * x[0] + x[1] * x[1]) * 0.5;
            }
            slice.frames += 1;
            let lo_mono = (lo[0] + lo[1]) * 0.5;
            let all_mono = (f[0] + f[1]) * 0.5;
            self.energy[b][0] += lo_mono * lo_mono;
            self.energy[b][1] += all_mono * all_mono;
        }
    }

    /// The waveform so far, scaled to the loudest peak so far.
    pub fn waveform(&self) -> Waveform {
        summarise(&self.slices)
    }

    /// Finishes the analysis: the final waveform, tempo and first beat.
    pub fn finish(mut self, bpm_hint: Option<f64>) -> Analysis {
        // Drop the part of an over-estimated length that never came.
        let slices = (self.frames as f64 / self.slice_len).ceil().max(1.0) as usize;
        let bins = (self.frames as f64 / self.bin_len).ceil().max(1.0) as usize;
        self.slices.truncate(slices);
        self.energy.truncate(bins);
        let bin_len = self.bin_len;
        let energy = &self.energy;

        // Onset envelope: positive changes in log energy, bass weighted.
        let level = |e: f32| (1.0 + 1000.0 * (e / bin_len as f32).sqrt()).ln();
        let mut onset = vec![0.0f32; energy.len()];
        for i in 1..energy.len() {
            let d_lo = level(energy[i][0]) - level(energy[i - 1][0]);
            let d_all = level(energy[i][1]) - level(energy[i - 1][1]);
            onset[i] = d_lo.max(0.0) + 0.5 * d_all.max(0.0);
        }

        let waveform = summarise(&self.slices);
        let tagged = matches!(bpm_hint, Some(b) if (40.0..=250.0).contains(&b));
        let bpm = match bpm_hint {
            Some(b) if tagged => b,
            _ => detect_bpm(&onset),
        };
        let first_beat = if bpm > 0.0 {
            let period = ONSET_RATE * 60.0 / bpm;
            let (phase, _) = fold(&onset, period);
            phase * period / ONSET_RATE
        } else {
            0.0
        };
        // A tempo from tags is taken as steady.
        let grid = match bpm {
            b if b <= 0.0 => BeatGrid::default(),
            _ if tagged => BeatGrid::constant(bpm, first_beat),
            _ => dynamic_grid(&onset, bpm).unwrap_or_else(|| BeatGrid::constant(bpm, first_beat)),
        };
        let (bpm, first_beat) = if grid.is_dynamic() {
            (
                grid.main_bpm(self.frames as f64 / bin_len / ONSET_RATE),
                grid.first_beat(),
            )
        } else {
            (bpm, first_beat)
        };
        let key_detector = match self.neural {
            Some(_) => Detector::Neural,
            None => Detector::Standard,
        };
        // Audio too short for the network gets the standard detector's key.
        let key = self
            .neural
            .and_then(Spectrogram::key)
            .or_else(|| self.key.key());
        Analysis {
            waveform,
            key,
            key_detector,
            bpm,
            first_beat,
            grid,
            loudness: self.loudness.finish(),
        }
    }
}

/// Quantises the slices; the coarser levels are built from the result.
fn summarise(slices: &[Slice]) -> Waveform {
    let top = slices.iter().map(|s| s.peak[3]).fold(1e-6f32, f32::max);
    // The bands carry less energy than the whole signal; lift them so the
    // colours read clearly (as Mixxx's analyzer does with its band gains).
    const BAND_GAIN: [f32; 4] = [1.0, 1.4, 2.0, 1.0];
    let q =
        |v: f32, k: usize| ((v * BAND_GAIN[k] / top).clamp(0.0, 1.0).sqrt() * 255.0).round() as u8;
    let finest = slices
        .iter()
        .map(|s| {
            let rms = s.rms();
            std::array::from_fn(|i| {
                if i < 4 {
                    q(s.peak[i], i)
                } else {
                    q(rms[i - 4], i - 4)
                }
            })
        })
        .collect();
    Waveform::from_finest(WAVEFORM_RATE, finest)
}

/// Folds the envelope at `period` (in bins) and returns the best beat phase
/// (0..1) and how strongly the beats line up there.
fn fold(onset: &[f32], period: f64) -> (f64, f32) {
    let mut hist = [0.0f32; PHASE_BINS];
    for (i, &o) in onset.iter().enumerate() {
        if o > 0.0 {
            let ph = (i as f64 / period).fract();
            hist[(ph * PHASE_BINS as f64) as usize % PHASE_BINS] += o;
        }
    }
    let total: f32 = hist.iter().sum::<f32>().max(1e-9);
    let mut best = (0, 0.0f32);
    for k in 0..PHASE_BINS {
        let s = hist[(k + PHASE_BINS - 1) % PHASE_BINS] * 0.5
            + hist[k]
            + hist[(k + 1) % PHASE_BINS] * 0.5;
        if s > best.1 {
            best = (k, s);
        }
    }
    // Centre of mass around the peak for sub-bin accuracy.
    let (mut w, mut acc) = (0.0f64, 0.0f64);
    for d in -1i32..=1 {
        let k = (best.0 as i32 + d).rem_euclid(PHASE_BINS as i32) as usize;
        w += hist[k] as f64;
        acc += hist[k] as f64 * (best.0 as f64 + d as f64 + 0.5);
    }
    let phase = if w > 0.0 {
        acc / w
    } else {
        best.0 as f64 + 0.5
    } / PHASE_BINS as f64;
    (phase.rem_euclid(1.0), best.1 / total)
}

fn detect_bpm(onset: &[f32]) -> f64 {
    let (min_bpm, max_bpm) = bpm_range();
    let lag_of = |bpm: f64| ONSET_RATE * 60.0 / bpm;
    let (min_lag, max_lag) = (
        lag_of(max_bpm).floor() as usize,
        lag_of(min_bpm).ceil() as usize,
    );
    if onset.len() < max_lag * 4 {
        return 0.0;
    }
    let mean = onset.iter().sum::<f32>() / onset.len() as f32;
    let centred: Vec<f32> = onset.iter().map(|o| o - mean).collect();
    let mut best = (min_lag, f32::MIN);
    for lag in min_lag..=max_lag {
        // Also reward the lag's multiples so bars outvote syncopation.
        let mut s = 0.0;
        for k in 1..=4 {
            let l = lag * k;
            if l >= centred.len() {
                break;
            }
            let c: f32 = centred[l..]
                .iter()
                .zip(&centred[..centred.len() - l])
                .map(|(a, b)| a * b)
                .sum();
            s += c / k as f32;
        }
        if s > best.1 {
            best = (lag, s);
        }
    }
    let coarse = ONSET_RATE * 60.0 / best.0 as f64;

    // Refine around the coarse tempo (one lag step either side).
    let lo = (ONSET_RATE * 60.0 / (best.0 as f64 + 1.0)).max(min_bpm);
    let hi = (ONSET_RATE * 60.0 / (best.0 as f64 - 1.0)).min(max_bpm);
    let mut fine = (coarse, f32::MIN);
    let mut bpm = lo;
    while bpm <= hi {
        let (_, score) = fold(onset, lag_of(bpm));
        if score > fine.1 {
            fine = (bpm, score);
        }
        bpm += 0.01;
    }
    let b = fine.0;
    if (b - b.round()).abs() < 0.06 {
        b.round()
    } else {
        (b * 100.0).round() / 100.0
    }
}

/// Share of clear beats whose local tempo must match the track's for it to
/// keep a steady grid.
const STEADY_SHARE: f64 = 0.85;
/// Tempos closer than this (as a fraction) count as the same.
const SAME_TEMPO: f64 = 0.004;
/// How close a beat must be to a new section's grid to join it, in seconds.
const ON_NEW_GRID: f64 = 0.012;
/// Clear beats either side that give a beat its local tempo.
const NEIGHBOURS: usize = 8;
/// Fewest clear beats in a section of a dynamic grid.
const MIN_SECTION: usize = 16;
/// How strong an onset (in standard deviations) marks a beat clearly.
const CLEAR_ONSET: f64 = 2.0;

/// Beat times (in onset bins) found by dynamic programming: each beat is
/// the strongest onset reachable from the best path to it, with a penalty
/// for spacing away from `period` (Ellis 2007). Each beat comes with whether
/// a clear onset marks it; through a breakdown the path only guesses.
fn track_beats(onset: &[f32], period: f64) -> Vec<(usize, bool)> {
    let n = onset.len();
    if n < (period * 8.0) as usize {
        return Vec::new();
    }
    let mean = onset.iter().sum::<f32>() / n as f32;
    let sd = (onset.iter().map(|o| (o - mean).powi(2)).sum::<f32>() / n as f32)
        .sqrt()
        .max(1e-9);
    let norm: Vec<f64> = onset.iter().map(|o| (*o / sd) as f64).collect();
    // How tightly beats keep to the period; Ellis's default.
    const TIGHTNESS: f64 = 100.0;
    let (lo, hi) = (
        (period * 0.5).round() as usize,
        (period * 2.0).round() as usize,
    );
    let mut score = vec![0.0f64; n];
    let mut from = vec![usize::MAX; n];
    for i in 0..n {
        let mut best = (0.0, usize::MAX);
        let first = i.saturating_sub(hi);
        let last = i.saturating_sub(lo).min(i);
        for (prev, &sp) in score[first..last]
            .iter()
            .enumerate()
            .map(|(k, s)| (k + first, s))
        {
            let gap = (i - prev) as f64 / period;
            let s = sp - TIGHTNESS * gap.ln().powi(2);
            if best.1 == usize::MAX || s > best.0 {
                best = (s, prev);
            }
        }
        score[i] = norm[i] + best.0.max(0.0);
        from[i] = if best.0 > 0.0 { best.1 } else { usize::MAX };
    }
    // End on the best-scoring beat in the last period.
    let tail = n.saturating_sub(period.ceil() as usize);
    let mut i = (tail..n)
        .max_by(|&a, &b| score[a].total_cmp(&score[b]))
        .unwrap();
    let mut beats = vec![i];
    while from[i] != usize::MAX {
        i = from[i];
        beats.push(i);
    }
    beats.reverse();
    // A beat is clear when a strong onset sits right on it.
    beats
        .into_iter()
        .map(|b| {
            let around = &norm[b.saturating_sub(2)..(b + 3).min(n)];
            (b, around.iter().copied().fold(0.0, f64::max) >= CLEAR_ONSET)
        })
        .collect()
}

/// The median of `v` (which it reorders); 0 for none.
fn median(v: &mut [f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// The beat phase (in seconds, 0..period) most of `times` agree on, so beats
/// that a syncopated part pulls off the beat don't drag it.
fn common_phase(times: &[f64], period: f64) -> f64 {
    const BINS: usize = 32;
    let mut hist = [0usize; BINS];
    let bin = |t: f64| ((t.rem_euclid(period) / period * BINS as f64) as usize).min(BINS - 1);
    for &t in times {
        hist[bin(t)] += 1;
    }
    let best = (0..BINS)
        .max_by_key(|&k| hist[(k + BINS - 1) % BINS] + 2 * hist[k] + hist[(k + 1) % BINS])
        .unwrap();
    // Average the beats in and beside the peak, unwrapped around it.
    let centre = (best as f64 + 0.5) / BINS as f64 * period;
    let (mut sum, mut n) = (0.0, 0.0);
    for &t in times {
        let k = bin(t);
        if k == best || k == (best + 1) % BINS || k == (best + BINS - 1) % BINS {
            let mut d = t.rem_euclid(period) - centre;
            if d > period / 2.0 {
                d -= period;
            } else if d < -period / 2.0 {
                d += period;
            }
            sum += d;
            n += 1.0;
        }
    }
    (centre + if n > 0.0 { sum / n } else { 0.0 }).rem_euclid(period)
}

/// A grid of tempo sections for a track whose tempo drifts; None if the
/// steady grid at `bpm` fits.
fn dynamic_grid(onset: &[f32], bpm: f64) -> Option<BeatGrid> {
    let (min_bpm, max_bpm) = bpm_range();
    let period = ONSET_RATE * 60.0 / bpm;
    let tracked = track_beats(onset, period);
    let beat = 60.0 / bpm;
    // Number the tracked beats, counting a missed beat where a gap is long,
    // and keep the clearly marked ones.
    let mut numbered: Vec<(f64, f64)> = Vec::with_capacity(tracked.len());
    let mut n = 0.0;
    let mut last: Option<f64> = None;
    for &(b, clear) in &tracked {
        let t = b as f64 / ONSET_RATE;
        if let Some(prev) = last {
            n += ((t - prev) / beat).round().max(1.0);
        }
        last = Some(t);
        if clear {
            numbered.push((n, t));
        }
    }
    if numbered.len() < MIN_SECTION * 2 {
        return None;
    }
    // Each beat's local beat length: the median slope to its neighbours.
    // A stretch pushed off the beat (syncopation) shifts every beat in it
    // alike, so it barely moves the median, while a real tempo change does.
    let local: Vec<f64> = (0..numbered.len())
        .map(|i| {
            let (ni, ti) = numbered[i];
            let lo = i.saturating_sub(NEIGHBOURS);
            let hi = (i + NEIGHBOURS + 1).min(numbered.len());
            let mut slopes: Vec<f64> = numbered[lo..hi]
                .iter()
                .filter(|(nj, _)| (nj - ni).abs() >= 2.0)
                .map(|(nj, tj)| (tj - ti) / (nj - ni))
                .collect();
            median(&mut slopes)
        })
        .collect();
    let steady = local
        .iter()
        .filter(|&&l| (l / beat - 1.0).abs() < SAME_TEMPO)
        .count();
    if steady as f64 >= local.len() as f64 * STEADY_SHARE {
        return None;
    }

    // Split where the local tempo moves away from the section's for a while.
    let mut sections: Vec<(usize, usize)> = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < numbered.len() {
        let mut lens: Vec<f64> = local[start..=i].to_vec();
        let here = median(&mut lens);
        let away = |k: usize| (local[k] / here - 1.0).abs() > SAME_TEMPO;
        let run = (i..(i + MIN_SECTION / 2).min(numbered.len())).all(away);
        if i - start >= MIN_SECTION / 2 && i + MIN_SECTION / 2 <= numbered.len() && run {
            sections.push((start, i));
            start = i;
        }
        i += 1;
    }
    sections.push((start, numbered.len()));
    // Short sections (often a breakdown with no clear beat) join the one before.
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (a, b) in sections {
        match merged.last_mut() {
            Some(last) if b - a < MIN_SECTION => last.1 = b,
            _ => merged.push((a, b)),
        }
    }
    let mut markers: Vec<GridMarker> = Vec::new();
    for (a, b) in merged {
        let mut lens: Vec<f64> = local[a..b].to_vec();
        let len = median(&mut lens);
        if len.is_nan() || len <= 0.0 {
            continue;
        }
        let times: Vec<f64> = numbered[a..b].iter().map(|(_, t)| *t).collect();
        let phase = common_phase(&times, len);
        // The section starts on its beat nearest its first tracked beat.
        let t0 = times[0];
        let seconds = t0 - (t0 - phase).rem_euclid(len);
        let mut seconds = if t0 - seconds > len / 2.0 {
            seconds + len
        } else {
            seconds
        };
        // The change was only noticed a few beats in: take the section back
        // over the beats before it that already keep its time.
        if let Some(prev) = markers.last() {
            let earliest = prev.seconds + 4.0 * 60.0 / prev.bpm;
            while seconds - len > earliest
                && numbered[..a]
                    .iter()
                    .any(|(_, t)| (t - (seconds - len)).abs() < ON_NEW_GRID)
            {
                seconds -= len;
            }
        }
        let m = GridMarker {
            seconds,
            bpm: 60.0 / len,
        };
        match markers.last() {
            // Neighbours within a hair of each other's tempo are one section.
            Some(last) if (m.bpm / last.bpm - 1.0).abs() < SAME_TEMPO => {}
            _ if (min_bpm * 0.9..=max_bpm * 1.1).contains(&m.bpm) => markers.push(m),
            _ => {}
        }
    }
    let grid = BeatGrid::new(markers);
    grid.is_dynamic().then_some(grid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth;

    #[test]
    fn finds_tempo_and_first_beat_of_synth_track() {
        for (bpm, first_beat) in [(124.0, 0.21), (128.0, 0.0), (97.0, 0.35)] {
            let audio = synth::render_with(bpm, first_beat, 60.0, 7);
            let a = analyse(&audio, None);
            assert!((a.bpm - bpm).abs() < 0.02, "{bpm}: got {}", a.bpm);
            let beat = 60.0 / bpm;
            let err = (a.first_beat - first_beat).rem_euclid(beat);
            let err = err.min(beat - err);
            assert!(
                err < 0.015,
                "{bpm}: first beat {} vs {first_beat}",
                a.first_beat
            );
            let w = &a.waveform;
            assert_eq!(w.levels[0].len(), (60.0 * WAVEFORM_RATE) as usize);
            assert_eq!(w.levels.last().unwrap().len(), 1);
            let max = |k: usize| w.levels[0].iter().map(|b| b[k]).max().unwrap();
            assert!(
                max(0) > 180 && max(1) > 80 && max(2) > 80,
                "{} {} {}",
                max(0),
                max(1),
                max(2)
            );
            // Peaks are never below RMS, and coarser levels keep the peak.
            assert!(w.levels[0].iter().all(|b| (0..4).all(|k| b[k] >= b[k + 4])));
            assert_eq!(w.levels.last().unwrap()[0][3], max(3));
        }
    }

    #[test]
    fn analysing_piece_by_piece_matches_all_at_once() {
        let audio = synth::render_with(128.0, 0.05, 30.0, 5);
        let whole = analyse(&audio, None);
        let frames: Vec<[f32; 2]> = audio.decoded_chunks().flatten().copied().collect();
        // Overestimate the length, as a decoder's estimate may.
        let mut a = Analyser::new(audio.sample_rate, audio.len() + 5_000);
        let partial = {
            a.feed(&frames[..1000]);
            a.waveform()
        };
        assert_eq!(
            partial.levels[0].len(),
            whole.waveform.levels[0].len() + 114
        );
        for piece in frames[1000..].chunks(777) {
            a.feed(piece);
        }
        let pieces = a.finish(None);
        assert_eq!(pieces.bpm, whole.bpm);
        assert_eq!(pieces.first_beat, whole.first_beat);
        assert_eq!(pieces.waveform.levels, whole.waveform.levels);
    }

    #[test]
    fn steady_tracks_get_a_steady_grid() {
        for bpm in [124.0, 97.0] {
            let audio = synth::render_with(bpm, 0.2, 60.0, 9);
            let a = analyse(&audio, None);
            assert!(!a.grid.is_dynamic(), "{bpm}: {:?}", a.grid);
            assert!((a.grid.bpm_at(30.0) - a.bpm).abs() < 1e-9);
        }
    }

    #[test]
    fn follows_a_tempo_change() {
        // 64 beats at 120 BPM (32 s), then 60 s at 126 BPM.
        let first = synth::render_with(120.0, 0.0, 32.0, 4);
        let second = synth::render_with(126.0, 0.0, 60.0, 4);
        let frames: Vec<[f32; 2]> = first
            .decoded_chunks()
            .chain(second.decoded_chunks())
            .flatten()
            .copied()
            .collect();
        let audio = Audio::from_frames(first.sample_rate, frames);
        let a = analyse(&audio, None);
        assert!(a.grid.is_dynamic(), "{:?}", a.grid);
        assert!((a.grid.bpm_at(10.0) - 120.0).abs() < 0.3, "{:?}", a.grid);
        assert!((a.grid.bpm_at(70.0) - 126.0).abs() < 0.3, "{:?}", a.grid);
        assert!((a.bpm - a.grid.bpm_at(70.0)).abs() < 1e-9);
        // Beats on the second part land on the grid.
        for k in 0..20 {
            let t = 32.0 + 60.0 / 126.0 * (k as f64 * 5.0);
            let off = (t - a.grid.snap(t, false)).abs();
            assert!(
                off < 0.02,
                "beat at {t} is {off} s off the grid {:?}",
                a.grid
            );
        }
    }

    #[test]
    fn uses_tag_tempo() {
        let audio = synth::render_with(126.0, 0.1, 20.0, 3);
        assert_eq!(analyse(&audio, Some(126.0)).bpm, 126.0);
    }
}
