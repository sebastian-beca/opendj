//! The side panel's Info tab: the details of the track selected in the
//! list (or loaded on a deck), editable in place. Tags are written into the
//! file and kept in the library; the library's own fields (User Tags 1 and
//! 2, the gain) only in the library. It also has a preview player for the
//! headphones, and finds and saves album art.

use std::sync::mpsc::Receiver;

use mixxx_engine::cover::{self, Cover};
use mixxx_engine::preview::PREVIEW_GROUP;
use mixxx_engine::scan::is_cloud_only;
use mixxx_engine::tags::{self, TagFields};
use mixxx_engine::{BeatGrid, Source};

use super::*;
use crate::coverart::{self, Event as CoverEvent, Found as FoundCover};
use crate::{CoverChoice, InfoField, TrackInfo};

/// The side panel's Info tab.
pub(super) const INFO_TAB: usize = 3;

/// Longest side of the cover the panel shows.
const COVER_SIZE: u32 = 160;

const TEXT: i32 = 0;
const READ: i32 = 1;
const RATING: i32 = 2;
const HEADING: i32 = 3;
const READ_WITH_BUTTON: i32 = 4;

pub(super) struct InfoState {
    /// 0 the selected track, 1 deck A, 2 deck B.
    subject: i32,
    /// The library's copy of the track shown, and where it is in `library`.
    shown: Option<(usize, Arc<Track>)>,
    fields: Rc<VecModel<InfoField>>,
    /// The cover being read for the track shown, and the generation it is for.
    cover_job: Option<(u64, Receiver<Option<Cover>>)>,
    generation: u64,
    /// The cover dialog's search and what it has found.
    search: Option<Receiver<CoverEvent>>,
    found: Vec<FoundCover>,
    /// The preview player's controls, and the track (an index into
    /// `library`) it has.
    play: ControlHandle,
    position: ControlHandle,
    duration: ControlHandle,
    previewing: Option<usize>,
    /// What the preview bar shows, to set only on change.
    shown_position: (bool, i64),
    /// Each deck's gain knob, set when the gain of its track is edited.
    pregain: Vec<ControlHandle>,
}

impl InfoState {
    pub(super) fn new(registry: &ControlRegistry) -> Self {
        let c = |k: &str| registry.get(PREVIEW_GROUP, k).expect("preview control");
        Self {
            subject: 0,
            shown: None,
            fields: Rc::new(VecModel::default()),
            cover_job: None,
            generation: 0,
            search: None,
            found: Vec::new(),
            play: c("play"),
            position: c("playposition"),
            duration: c("duration"),
            previewing: None,
            shown_position: (false, -1),
            pregain: (0..NUM_DECKS)
                .map(|d| registry.get(&deck_group(d), "pregain").expect("pregain"))
                .collect(),
        }
    }
}

/// "1.2 MB".
fn size_text(bytes: u64) -> String {
    let mb = bytes as f64 / 1_048_576.0;
    if mb >= 1.0 {
        format!("{mb:.1} MB")
    } else {
        format!("{} KB", bytes.div_ceil(1024))
    }
}

/// A Unix time as a local date, "19 Oct 2026".
fn date_text(at: i64) -> String {
    use chrono::TimeZone;
    match chrono::Local.timestamp_opt(at, 0).single() {
        Some(t) if at > 0 => t.format("%-d %b %Y").to_string(),
        _ => String::new(),
    }
}

/// "+1.5 dB", or "" for none.
fn gain_text(db: Option<f32>) -> String {
    match db {
        Some(g) if g > 0.0 => format!("+{g:.1} dB"),
        Some(g) => format!("{g:.1} dB"),
        None => String::new(),
    }
}

/// A gain typed in: "-2.5", "+3 dB"; empty for none.
fn parse_gain(text: &str) -> Result<Option<f32>, String> {
    let t = text
        .trim()
        .trim_end_matches("dB")
        .trim_end_matches("db")
        .trim();
    if t.is_empty() {
        return Ok(None);
    }
    t.trim_start_matches('+')
        .replace(',', ".")
        .replace('−', "-")
        .parse::<f32>()
        .ok()
        .filter(|g| (-24.0..=12.0).contains(g))
        .map(Some)
        .ok_or_else(|| "Gain is a number of dB from -24 to +12.".to_string())
}

fn field(id: &str, label: &str, value: impl Into<SharedString>, kind: i32) -> InfoField {
    InfoField {
        id: id.into(),
        label: label.into(),
        value: value.into(),
        kind,
        hint: SharedString::new(),
        action: SharedString::new(),
    }
}

fn heading(label: &str) -> InfoField {
    field("", label, "", HEADING)
}

/// The info list for `t`, in the order the DJ reads it.
fn fields_of(t: &Track, camelot: bool, file_manager: &str) -> Vec<InfoField> {
    let names = query::field_names();
    let text = |id: &str, label: &str, value: &str| field(id, label, value, TEXT);
    let mut out = vec![
        text("title", "TRACK", &t.title),
        text("artist", "ARTIST", &t.artist),
        text("remix", "REMIX", &t.info.remix),
        text("album", "ALBUM", &t.album),
        text("year", "YEAR", &t.info.year),
        text("genre", "GENRE", &t.genre),
        text(
            "track_number",
            "TRACK #",
            &if t.info.track_number > 0 {
                t.info.track_number.to_string()
            } else {
                String::new()
            },
        ),
        text("grouping", &names.grouping.to_uppercase(), &t.info.grouping),
        text(
            "bpm",
            "BPM",
            &if t.bpm > 0.0 {
                format!("{:.2}", t.bpm)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_string()
            } else {
                String::new()
            },
        ),
        InfoField {
            hint: "e.g. 8A or Am".into(),
            ..text("key", "KEY", &show_key(&t.key, camelot))
        },
        InfoField {
            hint: "0.0 dB".into(),
            ..text("gain", "GAIN", &gain_text(t.info.gain_db))
        },
        field("rating", "RATING", t.rating.to_string(), RATING),
    ];
    // The DJ's tag fields, by the names they gave them.
    for (i, name) in names.tags.iter().enumerate() {
        out.push(InfoField {
            hint: "words, with spaces between".into(),
            ..text(
                &format!("tag{}", i + 1),
                &name.to_uppercase(),
                t.info.user_tag(i),
            )
        });
    }
    out.extend([
        text("comment", "COMMENT", &t.info.comment),
        heading("STATS"),
        field("", "DURATION", short_time(t.duration), READ),
        field("", "PLAY COUNT", t.info.play_count.to_string(), READ),
        field("", "FIRST SEEN", date_text(t.info.added), READ),
        field(
            "",
            "LAST PLAYED",
            t.info.last_played.map(date_text).unwrap_or_default(),
            READ,
        ),
    ]);
    if let Source::File(path) = &t.source {
        out.push(heading("FILE"));
        let f = if t.info.missing || is_cloud_only(path) {
            tags::FileInfo::default()
        } else {
            tags::file_info(path)
        };
        let format = match f.bitrate {
            Some(b) if !f.format.is_empty() => format!("{} · {b} kbps", f.format),
            _ => f.format.clone(),
        };
        out.push(field("", "FORMAT", format, READ));
        let mut audio = Vec::new();
        if let Some(r) = f.sample_rate {
            audio.push(format!("{:.1} kHz", r as f64 / 1000.0).replace(".0 ", " "));
        }
        if let Some(b) = f.bit_depth {
            audio.push(format!("{b}-bit"));
        }
        match f.channels {
            Some(1) => audio.push("Mono".into()),
            Some(2) => audio.push("Stereo".into()),
            Some(n) => audio.push(format!("{n} channels")),
            None => {}
        }
        out.push(field("", "AUDIO", audio.join(" · "), READ));
        if f.size > 0 {
            out.push(field("", "SIZE", size_text(f.size), READ));
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        out.push(InfoField {
            action: format!("Show in {file_manager}").into(),
            ..field(
                "",
                "FILE",
                if t.info.missing {
                    format!("{name} (missing)")
                } else {
                    name
                },
                READ_WITH_BUTTON,
            )
        });
        let folder = path
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        out.push(field("", "FOLDER", folder, READ));
    }
    out
}

/// The tags of `t` as they go in its file.
pub(super) fn tag_fields(t: &Track) -> TagFields {
    TagFields {
        title: t.title.clone(),
        artist: t.artist.clone(),
        remix: t.info.remix.clone(),
        album: t.album.clone(),
        year: t.info.year.clone(),
        genre: t.genre.clone(),
        track_number: t.info.track_number,
        grouping: t.info.grouping.clone(),
        bpm: t.bpm,
        key: t.key.clone(),
        rating: t.rating,
        comment: t.info.comment.clone(),
    }
}

/// The file whose tags can be written, if `t` has one on disk.
fn writable(t: &Track) -> Option<&Path> {
    match &t.source {
        Source::File(p) if !t.info.missing && p.exists() && !is_cloud_only(p) => Some(p),
        _ => None,
    }
}

/// `edit` applied to `t`; Err says why the value can't be taken.
fn apply(t: &mut Track, id: &str, value: &str) -> Result<(), String> {
    let v = value.trim().to_string();
    match id {
        "title" => t.title = v,
        "artist" => t.artist = v,
        "remix" => t.info.remix = v,
        "album" => t.album = v,
        "year" => t.info.year = v,
        "genre" => t.genre = v,
        "grouping" => t.info.grouping = v,
        "comment" => t.info.comment = v,
        tag if tag.starts_with("tag") => match tag[3..].parse::<usize>() {
            Ok(n) if n >= 1 => {
                t.info.set_user_tag(n - 1, v);
                // As the library saves them, for `tag:` searches.
                t.info.labels = t.info.user_words();
            }
            _ => return Err(format!("No field {tag}.")),
        },
        "key" => t.key = v,
        "track_number" => {
            t.info.track_number = if v.is_empty() {
                0
            } else {
                v.parse()
                    .map_err(|_| "Track # is a whole number.".to_string())?
            }
        }
        "rating" => t.rating = v.parse::<u8>().unwrap_or(0).min(5),
        "gain" => t.info.gain_db = parse_gain(&v)?,
        "bpm" => {
            let bpm = if v.is_empty() {
                0.0
            } else {
                v.replace(',', ".")
                    .parse::<f64>()
                    .ok()
                    .filter(|b| (20.0..=400.0).contains(b))
                    .ok_or_else(|| "BPM is a tempo from 20 to 400.".to_string())?
            };
            if bpm > 0.0 {
                // The grid keeps its first beat and takes the new tempo.
                let grid = t.beat_grid();
                let grid = if grid.is_dynamic() {
                    grid.with_bpm_at(grid.first_beat(), bpm)
                } else {
                    BeatGrid::constant(bpm, t.first_beat)
                };
                t.set_grid(grid);
                t.bpm = bpm;
            } else {
                t.bpm = 0.0;
                t.grid = BeatGrid::default();
            }
        }
        _ => return Err(String::new()),
    }
    Ok(())
}

impl Browser {
    pub(super) fn install_info(app: &AppWindow, b: &Rc<RefCell<Self>>) {
        let ui = app.global::<TrackInfo>();
        ui.set_fields(ModelRc::from(b.borrow().info.fields.clone()));
        let weak = Rc::downgrade(b);
        let with = move |f: &dyn Fn(&mut Browser)| {
            if let Some(b) = weak.upgrade() {
                f(&mut b.borrow_mut());
            }
        };
        let w = with.clone();
        ui.on_set_subject(move |s| {
            w(&|b| {
                b.info.subject = s.clamp(0, NUM_DECKS as i32);
                if let Some(app) = b.app.upgrade() {
                    app.global::<TrackInfo>().set_subject(b.info.subject);
                }
            })
        });
        let w = with.clone();
        ui.on_edit(move |id, value| w(&|b| b.info_edit(&id, &value)));
        let w = with.clone();
        ui.on_reveal(move || {
            w(&|b| {
                if let Some((_, t)) = &b.info.shown {
                    if let Source::File(p) = &t.source {
                        files::reveal(p);
                    }
                }
            })
        });
        let w = with.clone();
        ui.on_preview_toggle(move || w(&|b| b.preview_toggle()));
        let w = with.clone();
        ui.on_preview_seek(move |at| {
            w(&|b| {
                if b.info.previewing.is_some() && b.info.previewing == b.info_index() {
                    b.info.position.set(at.clamp(0.0, 1.0) as f64);
                } else if let Some(i) = b.info_index() {
                    // Seeking a track not in the player starts it there.
                    b.engine.preview(Some(&b.library[i]), true);
                    b.info.previewing = Some(i);
                    b.info.shown_position = (true, -1);
                }
            })
        });
        let w = with.clone();
        ui.on_find_covers(move || w(&|b| b.find_covers()));
        let w = with.clone();
        ui.on_save_cover(move |i| w(&|b| b.save_cover(i.max(0) as usize)));
    }

    /// Where in `library` the track the Info tab shows is.
    fn info_index(&self) -> Option<usize> {
        self.info.shown.as_ref().map(|(i, _)| *i)
    }

    /// The track the Info tab is to show: the one selected in the list,
    /// or the one on the deck it follows.
    fn info_subject(&self, app: &AppWindow) -> Option<usize> {
        match self.info.subject {
            0 => usize::try_from(app.get_current_track())
                .ok()
                .and_then(|r| self.rows.get(r).copied().flatten()),
            d => self.loaded[(d - 1) as usize].and_then(|id| self.index_of.get(&id).copied()),
        }
    }

    /// Keeps the Info tab up to date; called every frame.
    pub(super) fn poll_info(&mut self, app: &AppWindow, tab: usize) {
        if tab != INFO_TAB {
            return;
        }
        let subject = self.info_subject(app);
        let now = subject.map(|i| (i, self.library[i].clone()));
        let changed = match (&now, &self.info.shown) {
            (Some((i, t)), Some((j, u))) => i != j || !Arc::ptr_eq(t, u),
            (None, None) => false,
            _ => true,
        };
        if changed {
            let other_track = now.as_ref().map(|(i, _)| *i) != self.info_index();
            self.info.shown = now;
            if other_track {
                self.info_switched(app);
            }
            self.refresh_info(app);
        }
        // The cover for the panel.
        if let Some((generation, rx)) = &self.info.cover_job {
            if let Ok(cover) = rx.try_recv() {
                if *generation == self.info.generation {
                    let ui = app.global::<TrackInfo>();
                    ui.set_has_cover(cover.is_some());
                    ui.set_cover(cover.map(|c| image(&c)).unwrap_or_default());
                }
                self.info.cover_job = None;
            }
        }
        self.poll_cover_search(app);
        self.poll_preview(app);
    }

    /// Another track is shown: the preview stops and the cover is read.
    fn info_switched(&mut self, app: &AppWindow) {
        let ui = app.global::<TrackInfo>();
        if self.info.previewing.take().is_some() {
            self.engine.preview(None, false);
        }
        self.fill_info_wave(app);
        ui.set_status(SharedString::new());
        ui.set_cover_open(false);
        self.info.search = None;
        self.info.found.clear();
        self.read_info_cover();
    }

    /// Reads the cover of the track shown, off the UI thread.
    fn read_info_cover(&mut self) {
        self.info.generation += 1;
        let thumb = self.info.shown.as_ref().and_then(|(_, t)| self.art.get(t));
        if let Some(app) = self.app.upgrade() {
            // The list's thumbnail until the larger one is read.
            let ui = app.global::<TrackInfo>();
            ui.set_has_cover(thumb.is_some());
            ui.set_cover(thumb.unwrap_or_default());
        }
        self.info.cover_job = None;
        let Some((_, t)) = &self.info.shown else {
            return;
        };
        let Source::File(path) = &t.source else {
            return;
        };
        if t.info.missing || is_cloud_only(path) {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let (path, album) = (path.clone(), t.album.clone());
        let spawned = std::thread::Builder::new()
            .name("info-cover".into())
            .spawn(move || {
                let _ = tx.send(cover::find_sized(&path, &album, COVER_SIZE));
            });
        if spawned.is_ok() {
            self.info.cover_job = Some((self.info.generation, rx));
        }
    }

    /// Fills the Info tab for the track shown.
    pub(super) fn refresh_info(&mut self, app: &AppWindow) {
        let ui = app.global::<TrackInfo>();
        ui.set_subject(self.info.subject);
        let Some((_, t)) = &self.info.shown else {
            ui.set_has_track(false);
            self.info.fields.set_vec(Vec::new());
            return;
        };
        ui.set_has_track(true);
        ui.set_editable(writable(t).is_some() || mixxx_library::row_id(t.id).is_some());
        ui.set_title(t.title.clone().into());
        ui.set_artist(t.artist.clone().into());
        ui.set_bpm(if t.bpm > 0.0 {
            format!("{:.0}", t.bpm).into()
        } else {
            SharedString::new()
        });
        ui.set_key(show_key(&t.key, self.camelot).into());
        ui.set_length(short_time(t.duration));
        let fields = fields_of(t, self.camelot, files::file_manager());
        // Rows change in place, so a field being typed in keeps its focus.
        let model = &self.info.fields;
        if model.row_count() == fields.len() {
            for (i, f) in fields.into_iter().enumerate() {
                if model.row_data(i).as_ref() != Some(&f) {
                    model.set_row_data(i, f);
                }
            }
        } else {
            model.set_vec(fields);
        }
    }

    /// The DJ changed field `id` of the track shown to `value`: it is saved
    /// to the file's tags and the library.
    fn info_edit(&mut self, id: &str, value: &str) {
        let Some((index, _)) = self.info.shown.clone() else {
            return;
        };
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<TrackInfo>();
        // A file shown in Files joins the collection, as rating it does.
        self.adopt(&[index]);
        let mut t = (*self.library[index]).clone();
        let before = tag_fields(&t);
        if let Err(why) = apply(&mut t, id, value) {
            ui.set_status(why.into());
            // Puts the field back as it was.
            self.refresh_info(&app);
            return;
        }
        let mut status = String::new();
        if tag_fields(&t) != before {
            match writable(&t) {
                Some(path) => {
                    if let Err(e) = tags::write(path, &tag_fields(&t)) {
                        status = format!("Couldn't write the tags to the file: {e}");
                    }
                }
                None => {
                    status = "The file isn't on this computer, so only the library has the change."
                        .into()
                }
            }
        }
        if let Some(db) = &self.db {
            if let Err(e) = db.save_info(t.id, &t) {
                status = format!("Couldn't save to the library: {e}");
            }
            if id == "bpm" && t.bpm > 0.0 {
                if let Err(e) = db.save_grid(t.id, &t.beat_grid(), t.bpm) {
                    status = format!("Couldn't save the beat grid: {e}");
                }
            }
        }
        if id == "bpm" {
            t.info.fixed_grid = t.bpm > 0.0;
        }
        // A deck with the track takes a new gain at once.
        if id == "gain" {
            for (d, loaded) in self.loaded.iter().enumerate() {
                if *loaded == Some(t.id) {
                    let gain = 10f64.powf(t.info.gain_db.unwrap_or(0.0) as f64 / 20.0);
                    self.info.pregain[d].set(gain.clamp(0.0, 4.0));
                }
            }
        }
        ui.set_status(status.into());
        self.indexed[index] = Indexed::new(&t);
        self.library[index] = Arc::new(t);
        self.refresh_rows();
        self.refresh_side();
    }

    /// Starts looking for covers for the track shown.
    fn find_covers(&mut self) {
        let Some((_, t)) = &self.info.shown else {
            return;
        };
        let path = match &t.source {
            Source::File(p) if !t.info.missing && !is_cloud_only(p) => Some(p.clone()),
            _ => None,
        };
        self.info.found.clear();
        self.info.search = Some(coverart::search(coverart::Wanted {
            path,
            artist: t.artist.clone(),
            album: t.album.clone(),
            title: t.title.clone(),
            discogs_token: crate::settings::discogs_token(),
        }));
        if let Some(app) = self.app.upgrade() {
            let ui = app.global::<TrackInfo>();
            ui.set_cover_searching(true);
            ui.set_covers(ModelRc::new(VecModel::from(Vec::<CoverChoice>::new())));
        }
    }

    fn poll_cover_search(&mut self, app: &AppWindow) {
        let Some(rx) = &self.info.search else {
            return;
        };
        let mut changed = false;
        let mut done = false;
        while let Ok(e) = rx.try_recv() {
            match e {
                CoverEvent::Found(f) => {
                    // The same picture from two places shows once.
                    if !self.info.found.iter().any(|o| o.data == f.data) {
                        self.info.found.push(f);
                        changed = true;
                    }
                }
                CoverEvent::Done => done = true,
            }
        }
        let ui = app.global::<TrackInfo>();
        if changed {
            let choices: Vec<CoverChoice> = self
                .info
                .found
                .iter()
                .map(|f| CoverChoice {
                    image: image(&f.thumb),
                    source: f.source.clone().into(),
                    size: format!("{}x{}", f.width, f.height).into(),
                })
                .collect();
            ui.set_covers(ModelRc::new(VecModel::from(choices)));
        }
        if done {
            self.info.search = None;
            ui.set_cover_searching(false);
        }
    }

    /// Embeds the cover found at `index` in the track's tags.
    fn save_cover(&mut self, index: usize) {
        let (Some((i, t)), Some(found)) = (self.info.shown.clone(), self.info.found.get(index))
        else {
            return;
        };
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<TrackInfo>();
        let Some(path) = writable(&t) else {
            ui.set_status("The file isn't on this computer, so the cover can't be saved.".into());
            return;
        };
        if let Err(e) = tags::write_cover(path, &found.data) {
            ui.set_status(format!("Couldn't save the cover: {e}").into());
            return;
        }
        let data = found.data.clone();
        ui.set_status(SharedString::new());
        ui.set_cover_open(false);
        self.adopt(&[i]);
        let t = self.library[i].clone();
        if let Some(db) = &self.db {
            // Notes the file's new time, so a scan doesn't read it again.
            if let Err(e) = db.save_info(t.id, &t) {
                eprintln!("can't save the track: {e}");
            }
        }
        // Every list showing the track, and any deck it's on, shows the
        // new cover at once.
        self.art.set(&t, &data);
        let cover = cover::decode(&data, cover::COVER_SIZE).map(Arc::new);
        self.engine.set_cover(path, cover);
        self.read_info_cover();
        self.refresh_rows();
        self.refresh_side();
    }

    /// Plays or pauses the track shown in the headphones.
    fn preview_toggle(&mut self) {
        let Some(i) = self.info_index() else {
            return;
        };
        if self.info.previewing == Some(i) {
            self.info.play.set_bool(!self.info.play.get_bool());
        } else {
            self.engine.preview(Some(&self.library[i]), true);
            self.info.previewing = Some(i);
        }
        self.info.shown_position = (true, -1);
    }

    /// Shows the waveform of the track shown in the preview bar, once its
    /// overview has been read (see [`crate::waves`]).
    pub(super) fn fill_info_wave(&self, app: &AppWindow) {
        let wave = self
            .info
            .shown
            .as_ref()
            .and_then(|(_, t)| self.waves.get(t));
        let ui = app.global::<TrackInfo>();
        ui.set_has_wave(wave.is_some());
        ui.set_wave(wave.unwrap_or_default());
    }

    fn poll_preview(&mut self, app: &AppWindow) {
        let mine = self.info.previewing.is_some() && self.info.previewing == self.info_index();
        let playing = mine && self.info.play.get_bool();
        let at = if mine { self.info.position.get() } else { 0.0 };
        // Updated about 20 times a second at most.
        let step = (at * 1000.0) as i64;
        if (playing, step) == self.info.shown_position {
            return;
        }
        self.info.shown_position = (playing, step);
        let ui = app.global::<TrackInfo>();
        ui.set_preview_playing(playing);
        ui.set_preview_position(at as f32);
        let duration = self.info.duration.get();
        ui.set_preview_time(if mine && duration > 0.0 {
            short_time(at * duration)
        } else {
            SharedString::new()
        });
    }
}

fn image(c: &Cover) -> slint::Image {
    let pixels =
        slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&c.rgba, c.width, c.height);
    slint::Image::from_rgba8(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track() -> Track {
        Track {
            id: 1,
            artist: "Maribou State".into(),
            title: "Home".into(),
            album: "Portraits".into(),
            genre: String::new(),
            rating: 0,
            bpm: 100.0,
            key: "10B".into(),
            duration: 196.0,
            first_beat: 0.5,
            source: Source::Synth { seed: 1 },
            waveform: Default::default(),
            audio: None,
            cover: None,
            grid: Default::default(),
            info: Default::default(),
        }
    }

    #[test]
    fn takes_edits() {
        let mut t = track();
        apply(&mut t, "remix", " Club Mix ").unwrap();
        assert_eq!(t.info.remix, "Club Mix");
        apply(&mut t, "track_number", "7").unwrap();
        assert_eq!(t.info.track_number, 7);
        assert!(apply(&mut t, "track_number", "seven").is_err());
        apply(&mut t, "gain", "+2.5 dB").unwrap();
        assert_eq!(t.info.gain_db, Some(2.5));
        apply(&mut t, "gain", "").unwrap();
        assert_eq!(t.info.gain_db, None);
        assert!(apply(&mut t, "gain", "40").is_err());
        apply(&mut t, "rating", "4").unwrap();
        apply(&mut t, "tag3", "#dusk").unwrap();
        assert_eq!(t.info.user_tags[2], "#dusk");
        assert!(apply(&mut t, "tag0", "x").is_err());
        assert_eq!(t.rating, 4);
        apply(&mut t, "bpm", "124").unwrap();
        assert_eq!(t.bpm, 124.0);
        // The beats stay where they were: one still falls at 0.5 s.
        let beat = t.beat_at(0.5);
        assert!((beat - beat.round()).abs() < 1e-9, "{beat}");
        assert!(apply(&mut t, "bpm", "1000").is_err());
    }

    #[test]
    fn lists_the_fields_in_order() {
        let mut t = track();
        t.info.gain_db = Some(-1.5);
        let f = fields_of(&t, true, "Finder");
        let labels: Vec<&str> = f.iter().map(|f| f.label.as_str()).collect();
        assert_eq!(
            &labels[..15],
            [
                "TRACK", "ARTIST", "REMIX", "ALBUM", "YEAR", "GENRE", "TRACK #", "GROUPING", "BPM",
                "KEY", "GAIN", "RATING", "TAG 1", "TAG 2", "COMMENT"
            ]
        );
        assert_eq!(f[8].value, "100");
        assert_eq!(f[10].value, "-1.5 dB");
        assert!(labels.contains(&"PLAY COUNT"));
    }

    #[test]
    fn writes_sizes_and_gains() {
        assert_eq!(size_text(9_856_000), "9.4 MB");
        assert_eq!(size_text(2_000), "2 KB");
        assert_eq!(gain_text(Some(1.5)), "+1.5 dB");
        assert_eq!(parse_gain("−3").unwrap(), Some(-3.0));
    }
}
