//! Album art thumbnails for the browser's first column. Covers are found
//! the same way the decks find them (see [`mixxx_engine::cover`]) on a
//! background thread, and kept once per album folder, so a library of
//! whole albums decodes each cover once.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use mixxx_engine::cover::{self, Cover};
use mixxx_engine::{Source, Track};

/// Longest side of a thumbnail: twice the column, for HiDPI screens.
const THUMB_SIZE: u32 = 56;

pub struct Art {
    requests: Sender<(String, PathBuf, String)>,
    found: Receiver<(String, Option<Cover>)>,
    /// `None` while being looked for or when the track has none.
    images: RefCell<HashMap<String, Option<slint::Image>>>,
}

impl Art {
    pub fn start() -> Self {
        let (requests, rx) = mpsc::channel::<(String, PathBuf, String)>();
        let (tx, found) = mpsc::channel();
        std::thread::Builder::new()
            .name("album-art".into())
            .spawn(move || {
                for (key, path, album) in rx {
                    let cover = cover::find_sized(&path, &album, THUMB_SIZE);
                    if tx.send((key, cover)).is_err() {
                        break;
                    }
                }
            })
            .expect("spawn album art thread");
        Self {
            requests,
            found,
            images: RefCell::default(),
        }
    }

    /// The key a track's art is kept under: its folder and album, or the
    /// file itself when it has no album.
    pub fn key(track: &Track) -> Option<String> {
        let Source::File(path) = &track.source else {
            return None;
        };
        Some(if track.album.is_empty() {
            path.to_string_lossy().into_owned()
        } else {
            format!(
                "{}\u{0}{}",
                path.parent().unwrap_or(Path::new("")).to_string_lossy(),
                track.album
            )
        })
    }

    /// The track's thumbnail if it has been found; otherwise it is looked
    /// for, and [`Art::poll`] reports when it arrives.
    pub fn get(&self, track: &Track) -> Option<slint::Image> {
        let key = Self::key(track)?;
        let mut images = self.images.borrow_mut();
        if let Some(image) = images.get(&key) {
            return image.clone();
        }
        if let Source::File(path) = &track.source {
            images.insert(key.clone(), None);
            let _ = self.requests.send((key, path.clone(), track.album.clone()));
        }
        None
    }

    /// Drops the track's thumbnail, so it is read again (after its cover
    /// changed).
    pub fn forget(&self, track: &Track) {
        if let Some(key) = Self::key(track) {
            self.images.borrow_mut().remove(&key);
        }
    }

    /// Takes in covers found since the last call; true if any arrived.
    pub fn poll(&mut self) -> bool {
        let mut any = false;
        while let Ok((key, cover)) = self.found.try_recv() {
            if let Some(c) = cover {
                let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    &c.rgba, c.width, c.height,
                );
                self.images
                    .borrow_mut()
                    .insert(key, Some(slint::Image::from_rgba8(pixels)));
                any = true;
            }
        }
        any
    }
}
