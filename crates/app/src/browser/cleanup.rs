//! Clean-up in the browser (see [`mixxx_library::cleanup`]): the review
//! table of changes to names, the genres page, finding covers and looking
//! up tags online for many tracks at once, and making (and taking back)
//! changes to tags, in the library and in the files.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

use mixxx_engine::{tags, Track};
use mixxx_library::cleanup::{self, Change, Field, Logged};
use mixxx_library::health::Check;
use mixxx_library::query::Indexed;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::health::tracks;
use super::info::{tag_fields, writable};
use super::{is_browsed, Browser, Undo};
use crate::coverart::{self, Wanted};
use crate::lookup::{self, Found, Query};
use crate::{cover_scan, settings, CleanupRow, CleanupUi, GenreRow};

/// A change waiting in the review table, for a track (an index into
/// `library`).
struct Pending {
    index: usize,
    change: Change,
    on: bool,
    /// Where the new value came from, such as "MusicBrainz 92%"; empty
    /// for the clean-up rules.
    note: String,
}

/// A look-up at least this sure ticks its changes to empty fields.
const SURE: f32 = 0.8;

/// Seconds between tracks looked up, as MusicBrainz asks of apps.
const LOOKUP_SPACING: f32 = 1.1;

/// Tracks being looked up online, one at a time.
struct LookupBatch {
    /// Track ids and what was found for each, as they come back.
    done: Receiver<(usize, Option<Found>)>,
    total: usize,
    found: Vec<(usize, Found)>,
    back: usize,
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
    lookups: Option<LookupBatch>,
    /// What the review table is of: "Clean up" or "Look-up results for".
    heading: &'static str,
}

/// A genre as compared for "spelled like another": letters and digits
/// only, in small letters.
/// The changes a look-up proposes for a track, each with whether it is
/// ticked: only for a sure match filling an empty field. Fields the DJ
/// typed in by hand are left alone.
fn proposals(t: &Track, f: &Found, hand: &HashSet<(usize, Field)>) -> Vec<(Change, bool)> {
    [
        (Field::Artist, &f.artist),
        (Field::Title, &f.title),
        (Field::Album, &f.album),
        (Field::Year, &f.year),
        (Field::Label, &f.label),
        (Field::Genre, &f.genre),
    ]
    .into_iter()
    .filter(|(field, new)| !new.trim().is_empty() && !hand.contains(&(t.id, *field)))
    .filter_map(|(field, new)| {
        let old = field.get(t).trim().to_string();
        // Same but for capitals is not worth a change.
        if old.to_lowercase() == new.trim().to_lowercase() {
            return None;
        }
        let on = old.is_empty() && f.score >= SURE;
        Some((
            Change {
                field,
                old,
                new: new.trim().to_string(),
            },
            on,
        ))
    })
    .collect()
}

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
                        note: String::new(),
                    })
            })
            .collect();
        self.cleanup.heading = "Clean up";
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
                let mut track = format!("{} - {}", t.artist, t.title);
                if !p.note.is_empty() {
                    track += &format!(" · {}", p.note);
                }
                CleanupRow {
                    track: track.into(),
                    field: p.change.field.label().into(),
                    old: p.change.old.clone().into(),
                    new: p.change.new.clone().into(),
                    on: p.on,
                }
            })
            .collect();
        let n: HashSet<usize> = review.iter().map(|p| p.index).collect();
        ui.set_open(!review.is_empty());
        ui.set_heading(format!("{} {}", self.cleanup.heading, tracks(n.len())).into());
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
        let what = if self.cleanup.heading == "Clean up" {
            "clean-up"
        } else {
            "look-up"
        };
        self.health_status = self.make_changes(changes, what);
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

    /// Looks these tracks (indices into `library`) up online, one at a
    /// time in the background; the review table opens with what was found
    /// once all are back.
    pub(super) fn batch_lookup(&mut self, indices: &[usize]) -> String {
        if self.cleanup.lookups.is_some() {
            return "Still looking up the last batch.".into();
        }
        let (key, token) = (settings::acoustid_key(), settings::discogs_token());
        let jobs: Vec<(usize, Query)> = indices
            .iter()
            .map(|&i| &self.library[i])
            .filter(|t| !is_browsed(t))
            .map(|t| {
                // Only a file that's here is fingerprinted; one in iCloud
                // would be downloaded for it.
                let path = writable(t)
                    .filter(|p| !cover_scan::offloaded(p))
                    .map(|p| p.to_path_buf());
                (
                    t.id,
                    Query {
                        path,
                        artist: t.artist.clone(),
                        title: t.title.clone(),
                        duration: t.duration,
                        acoustid_key: key.clone(),
                        discogs_token: token.clone(),
                    },
                )
            })
            .filter(|(_, q)| {
                (q.path.is_some() && !key.is_empty())
                    || !(q.artist.trim().is_empty() || q.title.trim().is_empty())
            })
            .collect();
        if jobs.is_empty() {
            return if key.is_empty() {
                "These tracks have no artist and title to look up by. An AcoustID key in Settings › \
                 Library lets openDJ recognise them by their sound."
                    .into()
            } else {
                "None of these files is on this computer, and they have no artist and title to look up by."
                    .into()
            };
        }
        let total = jobs.len();
        let (tx, done) = channel();
        std::thread::Builder::new()
            .name("lookup-batch".into())
            .spawn(move || {
                for (n, (id, q)) in jobs.into_iter().enumerate() {
                    if n > 0 {
                        std::thread::sleep(std::time::Duration::from_secs_f32(LOOKUP_SPACING));
                    }
                    if tx.send((id, lookup::look_up(&q))).is_err() {
                        break;
                    }
                }
            })
            .expect("spawn lookup batch");
        self.cleanup.lookups = Some(LookupBatch {
            done,
            total,
            found: Vec::new(),
            back: 0,
        });
        format!("Looking up {} online…", tracks(total))
    }

    /// Takes in what the look-up found since last time, and opens the
    /// review table when the batch is done.
    fn poll_lookups(&mut self) {
        let Some(batch) = &mut self.cleanup.lookups else {
            return;
        };
        let mut news = false;
        for (id, found) in batch.done.try_iter() {
            news = true;
            batch.back += 1;
            if let Some(f) = found {
                batch.found.push((id, f));
            }
        }
        if !news {
            return;
        }
        let (back, total) = (batch.back, batch.total);
        if back < total {
            let found = batch.found.len();
            self.health_status = format!("Looking up: {back} of {total}, {found} found…");
            self.refresh_health_ui();
            return;
        }
        let Some(batch) = self.cleanup.lookups.take() else {
            return;
        };
        let hand = self
            .db
            .as_ref()
            .and_then(|db| db.hand_edited().ok())
            .unwrap_or_default();
        let mut review = Vec::new();
        for (id, f) in &batch.found {
            let Some(&i) = self.index_of.get(id) else {
                continue;
            };
            review.extend(
                proposals(&self.library[i], f, &hand)
                    .into_iter()
                    .map(|(change, on)| Pending {
                        index: i,
                        change,
                        on,
                        note: format!("{} {:.0}%", f.source, f.score * 100.0),
                    }),
            );
        }
        let found = batch.found.len();
        self.health_status = if found == 0 {
            format!(
                "Nothing found online for {}. Without an AcoustID key (Settings › Library) \
                 tracks are looked up by artist and title only.",
                tracks(total)
            )
        } else if review.is_empty() {
            format!("Found {found} of {}; nothing new for them.", tracks(total))
        } else {
            format!(
                "Found {found} of {}. Ticked changes only fill empty fields from sure matches; \
                 tick others by hand.",
                tracks(total)
            )
        };
        self.cleanup.review = review;
        self.cleanup.heading = "Look-up results for";
        self.refresh_cleanup_ui();
        self.refresh_health_ui();
    }

    /// Embeds the covers found since last time, and takes in look-ups.
    pub fn poll_cleanup(&mut self) {
        self.poll_lookups();
        let Some(batch) = &mut self.cleanup.covers else {
            return;
        };
        let back: Vec<(usize, Option<Vec<u8>>)> = batch.done.try_iter().collect();
        if back.is_empty() {
            return;
        }
        batch.back += back.len();
        let mut found = 0;
        for (id, data) in back {
            let (Some(data), Some(&i)) = (data, self.index_of.get(&id)) else {
                continue;
            };
            match self.keep_cover(i, &data) {
                Ok(()) => found += 1,
                Err(e) => eprintln!("{e}"),
            }
        }
        let Some(batch) = &mut self.cleanup.covers else {
            return;
        };
        batch.found += found;
        let (found, back, total) = (batch.found, batch.back, batch.total);
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

    #[test]
    fn ticks_only_sure_fills_of_empty_fields() {
        let t = Track {
            id: 7,
            artist: "Bonobo".into(),
            title: "kerala".into(),
            album: String::new(),
            genre: "Electronica".into(),
            rating: 0,
            bpm: 0.0,
            key: String::new(),
            duration: 300.0,
            first_beat: 0.0,
            source: mixxx_engine::Source::Synth { seed: 1 },
            waveform: Default::default(),
            audio: None,
            cover: None,
            grid: Default::default(),
            info: Default::default(),
        };
        let f = Found {
            artist: "Bonobo".into(),
            title: "Kerala".into(),
            album: "Migration".into(),
            year: "2017".into(),
            label: "Ninja Tune".into(),
            genre: "Downtempo".into(),
            score: 0.93,
            source: "AcoustID",
        };
        let hand = HashSet::from([(7, Field::Label)]);
        let got: Vec<(Field, bool)> = proposals(&t, &f, &hand)
            .into_iter()
            .map(|(c, on)| (c.field, on))
            .collect();
        // Same title but for capitals: left; label typed by hand: left;
        // genre already set: shown unticked.
        assert_eq!(
            got,
            [
                (Field::Album, true),
                (Field::Year, true),
                (Field::Genre, false)
            ]
        );
        let unsure = Found { score: 0.5, ..f };
        assert!(proposals(&t, &unsure, &HashSet::new())
            .iter()
            .all(|(_, on)| !on));
    }
}
