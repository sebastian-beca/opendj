//! Clean-up in the browser (see [`mixxx_library::cleanup`]): the review
//! table of changes to names, the genres page, finding covers for many
//! tracks at once, and making (and taking back) changes to tags, in the
//! library and in the files.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

use mixxx_engine::{cover, tags};
use mixxx_library::cleanup::{self, Change, Field, Logged};
use mixxx_library::health::Check;
use mixxx_library::query::Indexed;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::health::tracks;
use super::info::{tag_fields, writable};
use super::{is_browsed, Browser, Undo};
use crate::coverart::{self, Wanted};
use crate::{cover_scan, settings, CleanupRow, CleanupUi, GenreRow};

/// A change waiting in the review table, for a track (an index into
/// `library`).
struct Pending {
    index: usize,
    change: Change,
    on: bool,
}

/// Covers being looked for online, one track at a time.
struct CoverBatch {
    /// Track ids and the cover found for each, as they come back.
    done: Receiver<(usize, Option<Vec<u8>>)>,
    total: usize,
    back: usize,
    found: usize,
}

#[derive(Default)]
pub(super) struct CleanupState {
    review: Vec<Pending>,
    genres_open: bool,
    /// Every genre, its track count, and whether it is ticked.
    genres: Vec<(String, usize, bool)>,
    covers: Option<CoverBatch>,
}

/// A genre as compared for "spelled like another": letters and digits
/// only, in small letters.
fn genre_key(g: &str) -> String {
    g.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

impl Browser {
    pub(super) fn install_cleanup(
        ui: &CleanupUi,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        ui.on_toggle(move |row| {
            w(&|b| {
                if let Some(p) = b.cleanup.review.get_mut(row.max(0) as usize) {
                    p.on = !p.on;
                }
                b.refresh_cleanup_ui();
            })
        });
        let w = with.clone();
        ui.on_all(move |on| {
            w(&|b| {
                for p in &mut b.cleanup.review {
                    p.on = on;
                }
                b.refresh_cleanup_ui();
            })
        });
        let w = with.clone();
        ui.on_apply(move || w(&|b| b.apply_review()));
        let w = with.clone();
        ui.on_cancel(move || {
            w(&|b| {
                b.cleanup.review.clear();
                b.refresh_cleanup_ui();
            })
        });
        let w = with.clone();
        ui.on_open_genres(move || w(&|b| b.open_genres()));
        let w = with.clone();
        ui.on_toggle_genre(move |row| w(&|b| b.toggle_genre(row.max(0) as usize)));
        let w = with.clone();
        ui.on_merge_genres(move || w(&|b| b.merge_genres()));
        let w = with.clone();
        ui.on_close_genres(move || {
            w(&|b| {
                b.cleanup.genres_open = false;
                b.refresh_cleanup_ui();
            })
        });
    }

    /// Closes the review table and the genres page, as another list is
    /// picked.
    pub(super) fn close_cleanup(&mut self) {
        self.cleanup.review.clear();
        self.cleanup.genres_open = false;
    }

    /// Opens the review table with what the clean-up rules would change in
    /// these tracks (indices into `library`).
    pub(super) fn open_cleanup(&mut self, indices: &[usize]) -> String {
        let rules = settings::cleanup_rules();
        self.cleanup.review = indices
            .iter()
            .flat_map(|&i| {
                cleanup::changes(&self.library[i], &rules)
                    .into_iter()
                    .map(move |change| Pending {
                        index: i,
                        change,
                        on: true,
                    })
            })
            .collect();
        self.refresh_cleanup_ui();
        if self.cleanup.review.is_empty() {
            "Nothing to clean up with the rules turned on in Settings › Library.".into()
        } else {
            String::new()
        }
    }

    /// Shows the review table and the genres as things are now.
    pub(super) fn refresh_cleanup_ui(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<CleanupUi>();
        let review = &self.cleanup.review;
        let rows: Vec<CleanupRow> = review
            .iter()
            .map(|p| {
                let t = &self.library[p.index];
                CleanupRow {
                    track: format!("{} - {}", t.artist, t.title).into(),
                    field: p.change.field.label().into(),
                    old: p.change.old.clone().into(),
                    new: p.change.new.clone().into(),
                    on: p.on,
                }
            })
            .collect();
        let n: HashSet<usize> = review.iter().map(|p| p.index).collect();
        ui.set_open(!review.is_empty());
        ui.set_heading(format!("Clean up {}", tracks(n.len())).into());
        ui.set_rows(ModelRc::new(VecModel::from(rows)));
        ui.set_picked(review.iter().filter(|p| p.on).count() as i32);
        let all = self.genre_counts();
        let mut keys: HashMap<String, usize> = HashMap::new();
        for g in all.keys() {
            *keys.entry(genre_key(g)).or_default() += 1;
        }
        let alike = |g: &str| keys.get(&genre_key(g)).is_some_and(|&n| n > 1);
        ui.set_genre_count(if self.saved() { all.len() as i32 } else { 0 });
        ui.set_alike(all.keys().filter(|g| alike(g)).count() as i32);
        ui.set_genres_open(self.cleanup.genres_open);
        let genres: Vec<GenreRow> = self
            .cleanup
            .genres
            .iter()
            .map(|(name, count, picked)| GenreRow {
                name: name.clone().into(),
                count: *count as i32,
                picked: *picked,
                alike: alike(name),
            })
            .collect();
        ui.set_genres(ModelRc::new(VecModel::from(genres)));
        ui.set_genres_picked(self.cleanup.genres.iter().filter(|g| g.2).count() as i32);
    }

    /// Makes the ticked changes in the review table.
    fn apply_review(&mut self) {
        let review = std::mem::take(&mut self.cleanup.review);
        let changes: Vec<(usize, Field, String)> = review
            .into_iter()
            .filter(|p| p.on)
            .map(|p| (p.index, p.change.field, p.change.new))
            .collect();
        self.health_status = self.make_changes(changes, "clean-up");
        self.refresh_cleanup_ui();
        self.refresh_tree();
        self.refresh_rows();
    }

    /// Sets fields of tracks (indices into `library`), in the library and
    /// the files' tags, as one batch the undo line takes back. Says what
    /// was done.
    fn make_changes(&mut self, changes: Vec<(usize, Field, String)>, what: &str) -> String {
        let (logged, unwritten) = self.set_fields(changes);
        if logged.is_empty() {
            return "Nothing changed.".into();
        }
        let n = logged.iter().map(|l| l.0).collect::<HashSet<_>>().len();
        if let Some(db) = &mut self.db {
            match db.log_changes(&logged) {
                Ok(batch) => {
                    self.undo = Some(Undo {
                        batch: None,
                        text: format!("Undo {what} ({})", tracks(n)),
                        restore: Vec::new(),
                        changes: Some(batch),
                    });
                }
                Err(e) => eprintln!("can't log the changes: {e}"),
            }
        }
        let mut s = format!("Changed {}.", tracks(n));
        if unwritten > 0 {
            s += &format!(
                " {} couldn't be written to the file (not on this computer, or locked), only to the library.",
                tracks(unwritten)
            );
        }
        s
    }

    /// Sets fields of tracks (indices into `library`) in the library and,
    /// where the file is here, its tags. Returns what changed, by track
    /// id, and how many tracks' files couldn't be written.
    fn set_fields(&mut self, changes: Vec<(usize, Field, String)>) -> (Vec<Logged>, usize) {
        let mut by_track: BTreeMap<usize, Vec<(Field, String)>> = BTreeMap::new();
        for (i, field, value) in changes {
            by_track.entry(i).or_default().push((field, value));
        }
        let (mut logged, mut unwritten) = (Vec::new(), 0);
        for (i, fields) in by_track {
            let Some(current) = self.library.get(i).filter(|t| !is_browsed(t)) else {
                continue;
            };
            let mut t = (**current).clone();
            let mut these = Vec::new();
            for (field, value) in fields {
                let old = field.get(&t).to_string();
                if old != value {
                    these.push((t.id, field, old, value.clone()));
                    field.set(&mut t, value);
                }
            }
            if these.is_empty() {
                continue;
            }
            match writable(&t) {
                Some(path) => {
                    if let Err(e) = tags::write(path, &tag_fields(&t)) {
                        eprintln!("can't write tags to {}: {e}", path.display());
                        unwritten += 1;
                    }
                }
                None => unwritten += 1,
            }
            if let Some(db) = &self.db {
                if let Err(e) = db.save_info(t.id, &t) {
                    eprintln!("can't save: {e}");
                    continue;
                }
            }
            self.indexed[i] = Indexed::new(&t);
            self.library[i] = Arc::new(t);
            logged.extend(these);
        }
        (logged, unwritten)
    }

    /// Takes back a batch of changes to tags: each field gets its old
    /// value again.
    pub(super) fn undo_tag_changes(&mut self, batch: i64) {
        let Some(db) = &mut self.db else { return };
        let logged = match db.logged_changes(batch) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("can't read the changes: {e}");
                return;
            }
        };
        if let Err(e) = db.undo_changes(batch) {
            eprintln!("can't undo: {e}");
            return;
        }
        // Last change first, so a field changed twice ends as it began.
        let back: Vec<(usize, Field, String)> = logged
            .into_iter()
            .rev()
            .filter_map(|(id, field, old, _)| Some((*self.index_of.get(&id)?, field, old)))
            .collect();
        let n = back.iter().map(|b| b.0).collect::<HashSet<_>>().len();
        self.set_fields(back);
        self.health_status = format!("{} as they were.", tracks(n));
        self.refresh_cleanup_ui();
        self.refresh_tree();
        self.refresh_rows();
    }

    /// Every genre in the collection and how many tracks have it.
    fn genre_counts(&self) -> BTreeMap<String, usize> {
        let mut out = BTreeMap::new();
        for t in self.library.iter().filter(|t| !is_browsed(t)) {
            let g = t.genre.trim();
            if !g.is_empty() {
                *out.entry(g.to_string()).or_default() += 1;
            }
        }
        out
    }

    /// Opens the genres page, every genre in name order.
    fn open_genres(&mut self) {
        let mut genres: Vec<(String, usize, bool)> = self
            .genre_counts()
            .into_iter()
            .map(|(g, n)| (g, n, false))
            .collect();
        genres.sort_by(|a, b| {
            a.0.to_lowercase()
                .cmp(&b.0.to_lowercase())
                .then(a.0.cmp(&b.0))
        });
        self.cleanup.genres = genres;
        self.cleanup.genres_open = true;
        if let Some(app) = self.app.upgrade() {
            app.global::<CleanupUi>().set_into(SharedString::new());
        }
        self.refresh_cleanup_ui();
    }

    /// Ticks or unticks a genre; the first one ticked (the most used of
    /// those ticked) fills in the name to merge into.
    fn toggle_genre(&mut self, row: usize) {
        let Some(g) = self.cleanup.genres.get_mut(row) else {
            return;
        };
        g.2 = !g.2;
        if let Some(app) = self.app.upgrade() {
            let ui = app.global::<CleanupUi>();
            let picked: Vec<&(String, usize, bool)> =
                self.cleanup.genres.iter().filter(|g| g.2).collect();
            let into = ui.get_into().to_string();
            let named = picked.iter().any(|g| g.0 == into);
            if into.trim().is_empty() || named {
                // The most used; between equals, one with capitals.
                let best = picked
                    .iter()
                    .max_by_key(|g| (g.1, g.0.chars().any(char::is_uppercase)))
                    .map(|g| g.0.clone());
                ui.set_into(best.unwrap_or_default().into());
            }
        }
        self.refresh_cleanup_ui();
    }

    /// Gives every track with a ticked genre the name typed in.
    fn merge_genres(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let into = app.global::<CleanupUi>().get_into().trim().to_string();
        let picked: HashSet<String> = self
            .cleanup
            .genres
            .iter()
            .filter(|g| g.2)
            .map(|g| g.0.clone())
            .collect();
        if into.is_empty() || picked.is_empty() {
            return;
        }
        let changes: Vec<(usize, Field, String)> = (0..self.library.len())
            .filter(|&i| {
                let t = &self.library[i];
                !is_browsed(t) && picked.contains(t.genre.trim()) && t.genre != into
            })
            .map(|i| (i, Field::Genre, into.clone()))
            .collect();
        let s = self.make_changes(changes, "genre merge");
        self.health_status = format!("Merged {} into \"{into}\". {s}", genre_word(picked.len()));
        self.open_genres();
        self.refresh_tree();
        self.refresh_rows();
    }

    /// Looks online for covers for these tracks (indices into `library`),
    /// one at a time in the background, and embeds the largest found.
    pub(super) fn batch_covers(&mut self, indices: &[usize]) -> String {
        if self.cleanup.covers.is_some() {
            return "Still looking for covers for the last batch.".into();
        }
        let token = settings::discogs_token();
        let jobs: Vec<(usize, Wanted)> = indices
            .iter()
            .map(|&i| &self.library[i])
            .filter(|t| writable(t).is_some_and(|p| !cover_scan::offloaded(p)))
            .map(|t| {
                (
                    t.id,
                    Wanted {
                        path: None,
                        artist: t.artist.clone(),
                        album: t.album.clone(),
                        title: t.title.clone(),
                        discogs_token: token.clone(),
                    },
                )
            })
            .collect();
        if jobs.is_empty() {
            return "None of these files is on this computer.".into();
        }
        let total = jobs.len();
        let (tx, done) = channel();
        std::thread::Builder::new()
            .name("cover-batch".into())
            .spawn(move || {
                for (id, w) in jobs {
                    let found = if w.artist.trim().is_empty() {
                        None
                    } else {
                        coverart::best_cover(&w)
                    };
                    if tx.send((id, found)).is_err() {
                        break;
                    }
                }
            })
            .expect("spawn cover batch");
        self.cleanup.covers = Some(CoverBatch {
            done,
            total,
            back: 0,
            found: 0,
        });
        format!("Looking online for covers for {}…", tracks(total))
    }

    /// Embeds the covers found since last time.
    pub fn poll_cleanup(&mut self) {
        let Some(batch) = &mut self.cleanup.covers else {
            return;
        };
        let back: Vec<(usize, Option<Vec<u8>>)> = batch.done.try_iter().collect();
        if back.is_empty() {
            return;
        }
        batch.back += back.len();
        let mut saved = Vec::new();
        for (id, data) in back {
            let Some(data) = data else { continue };
            let Some(&i) = self.index_of.get(&id) else {
                continue;
            };
            let t = self.library[i].clone();
            let Some(path) = writable(&t) else { continue };
            if let Err(e) = tags::write_cover(path, &data) {
                eprintln!("can't save the cover of {}: {e}", path.display());
                continue;
            }
            if let Some(db) = &self.db {
                // Notes the file's new time, so a scan doesn't read it again.
                let _ = db.save_info(t.id, &t);
            }
            self.art.set(&t, &data);
            let shown = cover::decode(&data, cover::COVER_SIZE).map(Arc::new);
            self.engine.set_cover(path, shown);
            saved.push((id, cover_scan::mtime(path), true));
            self.health.set_cover(id, true);
        }
        let Some(batch) = &mut self.cleanup.covers else {
            return;
        };
        batch.found += saved.len();
        let (found, back, total) = (batch.found, batch.back, batch.total);
        if let Some(db) = &mut self.db {
            if let Err(e) = db.save_covers(&saved) {
                eprintln!("can't save covers: {e}");
            }
        }
        self.health_status = if back >= total {
            self.cleanup.covers = None;
            let mut s = format!("Found covers for {found} of {}.", tracks(total));
            if found < total {
                s += " Search the rest one by one from the track's info panel.";
            }
            s
        } else {
            format!("Looking for covers: {back} of {total}, {found} found…")
        };
        self.refresh_tree();
        if matches!(
            self.nodes.get(self.current_node).map(|n| &n.contents),
            Some(super::Contents::Health(Some(Check::NoCover)))
        ) {
            self.refresh_rows();
        } else {
            self.refresh_health_ui();
        }
    }
}

/// "1 genre" or "3 genres".
fn genre_word(n: usize) -> String {
    format!("{n} genre{}", if n == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genres_alike_ignore_case_and_punctuation() {
        assert_eq!(genre_key("Electronica"), "electronica");
        assert_eq!(genre_key("Deep-House"), genre_key("deep house"));
        assert_ne!(genre_key("Electronic"), genre_key("Electronica"));
    }
}
