//! Deezer's Spleeter 4-stem model (MIT): one U-Net per stem predicts a
//! magnitude spectrogram, and the four predictions become ratio masks on the
//! mix's spectrogram, so the stems add up to the mix exactly.
//!
//! Weights come from `scripts/models/convert_spleeter.py`. The network follows
//! `spleeter/model/functions/unet.py` with the 4-stem settings (ELU in both
//! halves, TensorFlow "same" padding). It runs on burn ([`gpu::Net`]) or on
//! plain CPU matrix products ([`cpu::Net`]), which are much faster than
//! burn's CPU backend for these convolutions.

pub mod cpu;
pub mod gpu;

use realfft::num_complex::Complex32;
use safetensors::SafeTensors;

use crate::stft::{Stft, BINS, FRAME, HOP};

/// The stems in the order the model and the weights file have them.
pub const STEMS: [&str; 4] = ["vocals", "drums", "bass", "other"];

/// Frames per window the network sees (about 11.9 s).
pub const T: usize = 512;
/// Frequency bins the network sees (up to 11 kHz); the rest of the mask is
/// the average of these.
pub const F: usize = 1024;
const EPSILON: f32 = 1e-10;

/// Separated stereo stems, in [`STEMS`] order: `[stem][channel][sample]`.
pub type Stems = Vec<[Vec<f32>; 2]>;

/// The four U-Nets on some device.
pub trait Net {
    /// Each stem's estimated magnitudes for one window of the mix's
    /// magnitudes (`[channel][frame][bin]`, 2 × [`T`] × [`F`]), in the same
    /// layout, one after the other in [`STEMS`] order.
    fn estimate(&self, magnitudes: Vec<f32>) -> Vec<f32>;
}

/// A tensor from the weights file as f32, with its shape.
pub(crate) fn tensor(st: &SafeTensors, name: &str) -> Result<(Vec<f32>, Vec<usize>), String> {
    let t = st.tensor(name).map_err(|e| format!("{name}: {e}"))?;
    let data = t
        .data()
        .chunks_exact(2)
        .map(|b| half::f16::from_le_bytes([b[0], b[1]]).to_f32())
        .collect();
    Ok((data, t.shape().to_vec()))
}

/// Spleeter's masks from the four estimates, in place: each stem's share of
/// the summed squares.
fn to_masks(estimates: &mut [f32]) {
    let n = estimates.len() / STEMS.len();
    for i in 0..n {
        let total: f32 = (0..STEMS.len())
            .map(|s| estimates[s * n + i].powi(2))
            .sum::<f32>()
            + EPSILON;
        for s in 0..STEMS.len() {
            let e = &mut estimates[s * n + i];
            *e = (e.powi(2) + EPSILON / STEMS.len() as f32) / total;
        }
    }
}

/// Separates a stereo track at 44.1 kHz with `net`. Windows are separated
/// starting with the one holding sample `from`, then onwards to the end, then
/// back to the start; `done(window, windows)` is called after each.
pub fn separate(
    net: &impl Net,
    channels: [&[f32]; 2],
    from: usize,
    mut done: impl FnMut(usize, usize),
) -> Stems {
    let stft = Stft::new();
    let len = channels[0].len();
    // Spleeter pads the front with a frame of zeros so the first samples
    // are covered by as many frames as the rest.
    let specs: Vec<Vec<Complex32>> = channels
        .iter()
        .map(|c| {
            let mut p = vec![0.0; FRAME];
            p.extend_from_slice(c);
            stft.analyse(&p)
        })
        .collect();
    let frames = specs[0].len() / BINS;
    let windows = frames.div_ceil(T).max(1);
    let first = ((from + FRAME) / HOP / T).min(windows - 1);
    let order = (first..windows).chain((0..first).rev());

    let mut out: Stems = (0..STEMS.len())
        .map(|_| [vec![0.0; len + FRAME], vec![0.0; len + FRAME]])
        .collect();
    let mut masked = vec![Complex32::default(); T * BINS];
    for (n, w) in order.enumerate() {
        let f0 = w * T;
        let count = (frames - f0).min(T);
        let mut mags = vec![0.0f32; 2 * T * F];
        for (c, spec) in specs.iter().enumerate() {
            for f in 0..count {
                let row = &spec[(f0 + f) * BINS..][..F];
                let dst = &mut mags[(c * T + f) * F..][..F];
                for (d, z) in dst.iter_mut().zip(row) {
                    *d = z.norm();
                }
            }
        }
        let mut masks = net.estimate(mags);
        to_masks(&mut masks);
        for (s, stem) in out.iter_mut().enumerate() {
            for (c, spec) in specs.iter().enumerate() {
                for f in 0..count {
                    let m = &masks[((s * 2 + c) * T + f) * F..][..F];
                    let high = m.iter().sum::<f32>() / F as f32;
                    let src = &spec[(f0 + f) * BINS..][..BINS];
                    let dst = &mut masked[f * BINS..][..BINS];
                    for (b, (d, z)) in dst.iter_mut().zip(src).enumerate() {
                        *d = z * m.get(b).copied().unwrap_or(high);
                    }
                }
                stft.synthesise(&masked[..count * BINS], f0, &mut stem[c]);
            }
        }
        done(n, windows);
    }
    for stem in &mut out {
        for c in stem.iter_mut() {
            c.drain(..FRAME);
        }
    }
    out
}
