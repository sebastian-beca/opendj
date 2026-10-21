//! Background analysis: tempo, beat grid and key for tracks in triage, on
//! a thread of its own and one track at a time, so preparing tracks
//! doesn't wait for each to be loaded on a deck. Results are cached as a
//! deck load would cache them, and handed back to the browser to save.

use std::collections::HashSet;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use mixxx_engine::loader::analyse_file;
use mixxx_engine::Track;

pub struct AnalysisQueue {
    requests: Sender<Arc<Track>>,
    done: Receiver<Track>,
    /// Ids sent and not yet back, so a track isn't queued twice.
    queued: HashSet<usize>,
}

impl AnalysisQueue {
    pub fn start() -> Self {
        let (requests, rx) = channel::<Arc<Track>>();
        let (tx, done) = channel();
        std::thread::Builder::new()
            .name("background-analysis".into())
            .spawn(move || {
                for track in rx {
                    match analyse_file(&track) {
                        Ok(t) => {
                            if tx.send(t).is_err() {
                                break;
                            }
                        }
                        // Sent back as it was, so it leaves the queue.
                        Err(e) => {
                            eprintln!("can't analyse {}: {e}", track.title);
                            if tx.send((*track).clone()).is_err() {
                                break;
                            }
                        }
                    }
                }
            })
            .expect("spawn background analysis thread");
        Self {
            requests,
            done,
            queued: HashSet::new(),
        }
    }

    /// Whether a track still needs its tempo or key found.
    pub fn needs(t: &Track) -> bool {
        !t.info.missing && (t.bpm <= 0.0 || t.key.is_empty() || t.grid.is_empty())
    }

    /// Queues tracks that need analysing and aren't queued yet.
    pub fn add<'a>(&mut self, tracks: impl IntoIterator<Item = &'a Arc<Track>>) {
        for t in tracks {
            if Self::needs(t) && self.queued.insert(t.id) {
                let _ = self.requests.send(t.clone());
            }
        }
    }

    /// Tracks analysed since the last call.
    pub fn take(&mut self) -> Vec<Track> {
        let done: Vec<Track> = self.done.try_iter().collect();
        for t in &done {
            self.queued.remove(&t.id);
        }
        done
    }
}
