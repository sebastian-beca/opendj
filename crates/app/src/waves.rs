//! Mini waveforms for the browser's Waveform column: each analysed track's
//! overview (see [`mixxx_engine::cache::overview`]), read on a background
//! thread and drawn once into a small image. Only the rows in view hold an
//! image, so a long list costs no more than a screenful; the list draws
//! them like album art, with no work of its own between changes.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use mixxx_engine::cache::{self, OVERVIEW_SLICES};
use mixxx_engine::{OverviewSlice, Source, Track};

/// The image's size in pixels: a pixel column per slice, and tall enough
/// for a row on a HiDPI screen. The list stretches it to the column.
const WIDTH: usize = OVERVIEW_SLICES;
const HEIGHT: usize = 44;

/// How opaque the parts of a slice are: the list colours the image, so
/// the body (RMS) shows in full colour and the peaks around it fainter.
const BODY: f32 = 1.0;
const PEAK: f32 = 0.42;

pub struct Waves {
    requests: Sender<PathBuf>,
    found: Receiver<(PathBuf, Option<Vec<OverviewSlice>>)>,
    /// `None` while being read, or when the track isn't analysed.
    overviews: RefCell<HashMap<PathBuf, Option<Arc<[OverviewSlice]>>>>,
}

impl Waves {
    pub fn start() -> Self {
        let (requests, rx) = mpsc::channel::<PathBuf>();
        let (tx, found) = mpsc::channel();
        std::thread::Builder::new()
            .name("mini-waveforms".into())
            .spawn(move || {
                // Newest first: the rows asked for last are the ones in view.
                let mut queue = Vec::new();
                while let Ok(first) = rx.recv() {
                    queue.push(first);
                    while let Some(path) = queue.pop() {
                        queue.extend(rx.try_iter());
                        let overview = cache::overview(&path);
                        if tx.send((path, overview)).is_err() {
                            return;
                        }
                    }
                }
            })
            .expect("spawn mini waveform thread");
        Self {
            requests,
            found,
            overviews: RefCell::default(),
        }
    }

    /// The track's mini waveform if its overview has been read; otherwise
    /// it is read, and [`Waves::poll`] reports when it arrives.
    pub fn get(&self, track: &Track) -> Option<slint::Image> {
        let Source::File(path) = &track.source else {
            return None;
        };
        let mut overviews = self.overviews.borrow_mut();
        match overviews.get(path) {
            Some(o) => o.as_deref().map(image),
            None => {
                overviews.insert(path.clone(), None);
                let _ = self.requests.send(path.clone());
                None
            }
        }
    }

    /// Takes the overview of a track just analysed (on a deck or in the
    /// background), so its row shows it without reading it back. True if
    /// it is new: the track had none before.
    pub fn learn(&self, track: &Track) -> bool {
        let Source::File(path) = &track.source else {
            return false;
        };
        let mut overviews = self.overviews.borrow_mut();
        if track.waveform.is_empty() || overviews.get(path).is_some_and(|o| o.is_some()) {
            return false;
        }
        let overview = track.waveform.overview(OVERVIEW_SLICES);
        overviews.insert(path.clone(), Some(overview.into()));
        true
    }

    /// Forgets a track's overview, as its analysis was redone: it is read
    /// again when next asked for.
    pub fn forget(&self, path: &std::path::Path) {
        self.overviews.borrow_mut().remove(path);
    }

    /// Takes in overviews read since the last call; true if any arrived.
    pub fn poll(&mut self) -> bool {
        let mut any = false;
        while let Ok((path, overview)) = self.found.try_recv() {
            let Some(o) = overview else { continue };
            // Only while still being read: one learnt since then is newer.
            if let Some(slot @ None) = self.overviews.borrow_mut().get_mut(&path) {
                *slot = Some(o.into());
                any = true;
            }
        }
        any
    }
}

/// Draws an overview as a waveform mirrored about its middle, in white
/// with the opacity saying how loud: the list colours it.
fn image(overview: &[OverviewSlice]) -> slint::Image {
    let mut pixels =
        slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(WIDTH as u32, HEIGHT as u32);
    let buf = pixels.make_mut_slice();
    let half = HEIGHT as f32 / 2.0;
    for x in 0..WIDTH {
        let [peak, rms] = overview[x * overview.len() / WIDTH];
        let (peak, rms) = (peak as f32 / 255.0 * half, rms as f32 / 255.0 * half);
        for y in 0..HEIGHT {
            // How far this pixel's middle is from the centre line, and how
            // much of it each part covers, for soft edges.
            let d = (y as f32 + 0.5 - half).abs() - 0.5;
            let cover = |reach: f32| (reach - d).clamp(0.0, 1.0);
            let a = (cover(rms) * BODY).max(cover(peak.max(0.5)) * PEAK);
            buf[y * WIDTH + x] = slint::Rgba8Pixel {
                r: 255,
                g: 255,
                b: 255,
                a: (a * 255.0).round() as u8,
            };
        }
    }
    slint::Image::from_rgba8(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_body_and_peaks_about_the_middle() {
        let quiet = [[0u8, 0]; 4];
        let loud = [[255u8, 128]; 4];
        let alpha = |o: &[OverviewSlice], y: usize| {
            let img = image(o);
            let buf = img.to_rgba8().unwrap();
            buf.as_slice()[y * WIDTH].a
        };
        // A silent track is a hairline; a loud one fills the height, the
        // body solid and the peaks faint.
        assert_eq!(alpha(&quiet, 0), 0);
        assert!(alpha(&quiet, HEIGHT / 2) > 0);
        assert_eq!(alpha(&loud, HEIGHT / 2), 255);
        assert_eq!(alpha(&loud, 0), (PEAK * 255.0).round() as u8);
        assert_eq!(alpha(&loud, 1), alpha(&loud, HEIGHT - 2));
    }
}
