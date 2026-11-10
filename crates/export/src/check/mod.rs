//! A second, independent reader for what an export writes, used only by the
//! tests. The writer is rbox; reading the stick back with rbox too would
//! only show rbox agrees with itself. This reads it the way another player
//! stack does, so an export that passes here loads elsewhere too.
//!
//! Ported from EngineX OS's `pkg/rekordbox` (Go), the reader its
//! `cdj3000-sim` (OpenProLink's CDJ-3000 simulator) loads sticks with:
//! <https://github.com/Cd1s/EngineX-OS>. Its licence:
//!
//! > MIT License
//! >
//! > Copyright (c) 2026 EngineX OS Contributors
//! >
//! > Permission is hereby granted, free of charge, to any person obtaining a
//! > copy of this software and associated documentation files (the
//! > "Software"), to deal in the Software without restriction, including
//! > without limitation the rights to use, copy, modify, merge, publish,
//! > distribute, sublicense, and/or sell copies of the Software, and to
//! > permit persons to whom the Software is furnished to do so, subject to
//! > the following conditions:
//! >
//! > The above copyright notice and this permission notice shall be included
//! > in all copies or substantial portions of the Software.
//! >
//! > THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
//! > OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
//! > MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
//! > IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
//! > CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
//! > TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
//! > SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

pub mod anlz;
pub mod onelib;
pub mod pdb;

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Track {
    pub id: u32,
    pub path: String,
    pub analysis_path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub label: String,
    pub composer: String,
    pub remixer: String,
    pub comment: String,
    pub key: String,
    pub artwork_path: String,
    pub bpm: f64,
    /// Seconds.
    pub duration: f64,
    pub sample_rate: u32,
    pub bit_depth: u32,
    pub bitrate: u32,
    pub year: u32,
    pub rating: u32,
    pub file_size: u64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Playlist {
    pub id: u32,
    pub parent_id: u32,
    pub sort_order: u32,
    pub name: String,
    pub folder: bool,
    /// Track ids, in order.
    pub tracks: Vec<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct Library {
    pub tracks: Vec<Track>,
    /// Sorted by parent, then place, as a player lists them.
    pub playlists: Vec<Playlist>,
    pub artwork: HashMap<u32, String>,
    pub keys: HashMap<u32, String>,
}

impl Library {
    /// Refuses broken trees rather than losing folders or tracks, as
    /// EngineX does before it syncs a stick.
    pub fn validate(&mut self) -> Result<(), String> {
        let mut tracks = HashSet::new();
        for t in &self.tracks {
            if t.id == 0 || !tracks.insert(t.id) {
                return Err(format!("duplicate/zero track id {}", t.id));
            }
            if t.path.is_empty() {
                return Err(format!("track {} has no path", t.id));
            }
            if !t.bpm.is_finite()
                || !(0.0..=1000.0).contains(&t.bpm)
                || !t.duration.is_finite()
                || !(0.0..=1e7).contains(&t.duration)
                || t.sample_rate > 768_000
            {
                return Err(format!("invalid audio metadata for track {}", t.id));
            }
        }
        let mut parents = HashMap::new();
        let mut names = HashSet::new();
        for p in &self.playlists {
            if p.id == 0 || parents.insert(p.id, p.parent_id).is_some() {
                return Err(format!("duplicate/zero playlist id {}", p.id));
            }
            if !names.insert((p.parent_id, p.name.clone())) {
                return Err(format!("duplicate sibling playlist {:?}", p.name));
            }
            if let Some(id) = p.tracks.iter().find(|id| !tracks.contains(id)) {
                return Err(format!("playlist {} references absent track {id}", p.id));
            }
        }
        for &id in parents.keys() {
            let mut seen = HashSet::new();
            let mut p = id;
            while p != 0 {
                if !seen.insert(p) {
                    return Err(format!("playlist cycle at {p}"));
                }
                p = *parents.get(&p).ok_or(format!("missing parent {p}"))?;
            }
        }
        self.playlists
            .sort_by_key(|p| (p.parent_id, p.sort_order, p.id));
        Ok(())
    }

    pub fn playlist(&self, name: &str) -> &Playlist {
        self.playlists
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("no playlist {name:?}"))
    }

    pub fn track(&self, id: u32) -> &Track {
        self.tracks
            .iter()
            .find(|t| t.id == id)
            .unwrap_or_else(|| panic!("no track {id}"))
    }

    /// The titles of a playlist's tracks, in order.
    pub fn titles(&self, name: &str) -> Vec<String> {
        self.playlist(name)
            .tracks
            .iter()
            .map(|&id| self.track(id).title.clone())
            .collect()
    }
}
