//! Background analysis: tempo, beat grid, key and phrases for tracks in
//! triage, and for tracks the DJ asks to analyse, on a thread of its own and
//! one track at a time, so preparing tracks doesn't wait for each to be
//! loaded on a deck. Results are cached as a deck load would cache them,
//! and handed back to the browser to save. Tracks the DJ asks to analyse
//! that have no cover also get the first one found online, on a thread of
//! its own so the next track's analysis doesn't wait for it.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use opendj_engine::loader::analyse_file;
use opendj_engine::{cover, Source, Track};

use crate::coverart;

/// What `running` holds while nothing is being analysed.
const IDLE: usize = usize::MAX;

struct Request {
    track: Arc<Track>,
    /// Analyse again even when the cache has the track.
    fresh: bool,
    /// Look for a cover online if the track has none.
    cover: bool,
    /// The queue's generation when sent; a cancel moves it on, and the
    /// thread skips requests from before.
    generation: u64,
}

pub struct AnalysisQueue {
    requests: Sender<Request>,
    done: Receiver<Track>,
    /// Covers found online: the track's id and the image file's bytes.
    covers: Receiver<(usize, Vec<u8>)>,
    /// Ids sent and not yet back, so a track isn't queued twice.
    queued: HashSet<usize>,
    /// The id of the track being analysed now, or [`IDLE`].
    running: Arc<AtomicUsize>,
    generation: Arc<AtomicU64>,
    /// Tracks queued, and those back, since the queue was last empty: the
    /// footer's "Analyzing 3 of 12".
    total: usize,
    finished: usize,
}

impl AnalysisQueue {
    pub fn start() -> Self {
        let (requests, rx) = channel::<Request>();
        let (tx, done) = channel();
        let running = Arc::new(AtomicUsize::new(IDLE));
        let generation = Arc::new(AtomicU64::new(0));
        let (now, current) = (running.clone(), generation.clone());
        let (lookups, wanted) = channel::<(usize, coverart::Wanted)>();
        let (found, covers) = channel();
        std::thread::Builder::new()
            .name("cover-lookup".into())
            .spawn(move || {
                for (id, w) in wanted {
                    let Some(path) = &w.path else { continue };
                    if cover::has_cover(path, &w.album) {
                        continue;
                    }
                    if let Some(data) = coverart::first_online(&w, coverart::AUTO_MIN) {
                        if found.send((id, data)).is_err() {
                            break;
                        }
                    }
                }
            })
            .expect("spawn cover lookup thread");
        std::thread::Builder::new()
            .name("background-analysis".into())
            .spawn(move || {
                for r in rx {
                    if r.generation != current.load(Ordering::Relaxed) {
                        continue;
                    }
                    now.store(r.track.id, Ordering::Relaxed);
                    let result = analyse_file(&r.track, r.fresh);
                    now.store(IDLE, Ordering::Relaxed);
                    let t = match result {
                        Ok(t) => {
                            if r.cover {
                                if let Some(w) = wanted_cover(&t) {
                                    let _ = lookups.send((t.id, w));
                                }
                            }
                            t
                        }
                        // Sent back as it was, so it leaves the queue.
                        Err(e) => {
                            eprintln!("can't analyse {}: {e}", r.track.title);
                            (*r.track).clone()
                        }
                    };
                    if tx.send(t).is_err() {
                        break;
                    }
                }
            })
            .expect("spawn background analysis thread");
        Self {
            requests,
            done,
            covers,
            queued: HashSet::new(),
            running,
            generation,
            total: 0,
            finished: 0,
        }
    }

    /// Whether a track still needs its tempo or key found.
    pub fn needs(t: &Track) -> bool {
        !t.info.missing && (t.bpm <= 0.0 || t.key.is_empty() || t.grid.is_empty())
    }

    /// Queues tracks that need analysing and aren't queued yet.
    pub fn add<'a>(&mut self, tracks: impl IntoIterator<Item = &'a Arc<Track>>) {
        for t in tracks {
            if Self::needs(t) {
                self.send(t, false, false);
            }
        }
    }

    /// Queues tracks the DJ asked to analyse, to be analysed afresh even
    /// when they were before, and given a cover if they have none. Returns
    /// how many were queued.
    pub fn add_fresh<'a>(&mut self, tracks: impl IntoIterator<Item = &'a Arc<Track>>) -> usize {
        tracks
            .into_iter()
            .filter(|t| !t.info.missing && self.send(t, true, true))
            .count()
    }

    fn send(&mut self, t: &Arc<Track>, fresh: bool, cover: bool) -> bool {
        if !self.queued.insert(t.id) {
            return false;
        }
        let _ = self.requests.send(Request {
            track: t.clone(),
            fresh,
            cover,
            generation: self.generation.load(Ordering::Relaxed),
        });
        self.total += 1;
        true
    }

    /// Drops the tracks still waiting; the one being analysed finishes.
    pub fn cancel(&mut self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.queued.clear();
        self.total = 0;
        self.finished = 0;
    }

    /// Tracks analysed since the last call.
    pub fn take(&mut self) -> Vec<Track> {
        let done: Vec<Track> = self.done.try_iter().collect();
        for t in &done {
            if self.queued.remove(&t.id) {
                self.finished += 1;
            }
        }
        if self.queued.is_empty() {
            self.total = 0;
            self.finished = 0;
        }
        done
    }

    /// Covers found online since the last call, by track id.
    pub fn take_covers(&mut self) -> Vec<(usize, Vec<u8>)> {
        self.covers.try_iter().collect()
    }

    /// Whether a track is waiting (1) or being analysed (2); 0 if neither.
    pub fn state(&self, id: usize) -> i32 {
        if !self.queued.contains(&id) {
            0
        } else if self.running.load(Ordering::Relaxed) == id {
            2
        } else {
            1
        }
    }

    /// Changes whenever a track is queued, finishes or starts.
    pub fn signature(&self) -> (usize, usize, usize) {
        (
            self.queued.len(),
            self.finished,
            self.running.load(Ordering::Relaxed),
        )
    }

    /// The track being analysed, counting from 1, and how many there are
    /// in all; None when the queue is empty.
    pub fn progress(&self) -> Option<(usize, usize)> {
        (!self.queued.is_empty()).then(|| ((self.finished + 1).min(self.total), self.total))
    }
}

/// What to look for a cover of `t` by, if it is a file with something to
/// search by.
fn wanted_cover(t: &Track) -> Option<coverart::Wanted> {
    let Source::File(path) = &t.source else {
        return None;
    };
    if t.artist.trim().is_empty() && t.album.trim().is_empty() && t.title.trim().is_empty() {
        return None;
    }
    Some(coverart::Wanted {
        path: Some(path.clone()),
        artist: t.artist.clone(),
        album: t.album.clone(),
        title: t.title.clone(),
        discogs_token: crate::settings::discogs_token(),
    })
}
