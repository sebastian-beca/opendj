//! `export.pdb` and `exportExt.pdb`, the Device Library the CDJ-3000,
//! CDJ-2000NXS2 and older players read, written with rbox's DeviceSQL engine.

use std::collections::HashMap;
use std::path::Path;

use rbox_devlib::models::NewDevContent;
use rbox_devlib::{DeviceLibrary, LibraryCreateOptions};

use crate::layout;
use crate::model::{Model, MASTER_DB_ID};
use crate::Error;

/// Ids of named rows (artists, albums, genres, labels, keys) made on first use.
struct Names {
    ids: HashMap<String, i64>,
}

impl Names {
    fn new() -> Self {
        Self {
            ids: HashMap::new(),
        }
    }

    fn get(
        &mut self,
        name: &str,
        create: impl FnOnce(&str, i64) -> rbox_devlib::Result<i64>,
    ) -> Result<i64, Error> {
        let name = name.trim();
        if name.is_empty() {
            return Ok(0);
        }
        if let Some(&id) = self.ids.get(name) {
            return Ok(id);
        }
        let id = create(name, self.ids.len() as i64 + 1)?;
        self.ids.insert(name.to_string(), id);
        Ok(id)
    }
}

/// Writes both files into `dir` (a fresh `PIONEER/rekordbox` folder).
pub fn write(dir: &Path, model: &Model) -> Result<(), Error> {
    let options = LibraryCreateOptions::new(&model.date, &model.device_name, MASTER_DB_ID);
    let mut db = DeviceLibrary::create(dir, &options)?;

    for id in 1..=model.artworks as i64 {
        let made = db.create_artwork(layout::rooted(
            &layout::artwork(id as u32, 'a', false),
            model.export_dir,
        ))?;
        debug_assert_eq!(made.id, id);
    }

    let mut artists = Names::new();
    let mut albums: HashMap<(String, i64), i64> = HashMap::new();
    let mut genres = Names::new();
    let mut labels = Names::new();
    let mut keys = Names::new();
    for (i, t) in model.tracks.iter().enumerate() {
        let artist_id = artists.get(&t.artist, |n, _| Ok(db.create_artist(n)?.id))?;
        let remixer_id = artists.get(&t.remixer, |n, _| Ok(db.create_artist(n)?.id))?;
        let composer_id = artists.get(&t.composer, |n, _| Ok(db.create_artist(n)?.id))?;
        let album = t.album.trim();
        let album_id = if album.is_empty() {
            0
        } else {
            match albums.get(&(album.to_string(), artist_id)) {
                Some(&id) => id,
                None => {
                    let id = db.create_album(album, Some(artist_id))?.id;
                    albums.insert((album.to_string(), artist_id), id);
                    id
                }
            }
        };
        let genre_id = genres.get(&t.genre, |n, _| Ok(db.create_genre(n)?.id))?;
        let label_id = labels.get(&t.label, |n, _| Ok(db.create_label(n)?.id))?;
        let key_id = keys.get(&t.key, |n, next| Ok(db.create_key(n, next)?.id))?;

        let mut row = NewDevContent::new(&t.path);
        row.artist_id = artist_id;
        row.album_id = album_id;
        row.genre_id = genre_id;
        row.label_id = label_id;
        row.key_id = key_id;
        row.artwork_id = t.artwork;
        row.composer_id = composer_id;
        row.remixer_id = remixer_id;
        row.master_content_id = t.library_id;
        row.master_db_id = MASTER_DB_ID;
        row.sample_rate = t.sample_rate;
        row.file_size = t.file_size;
        row.bitrate = t.bitrate;
        row.bpmx100 = t.bpmx100;
        row.length = t.length;
        row.track_no = t.track_number;
        row.disc_no = t.disc_number;
        row.play_count = t.play_count;
        row.release_year = t.year;
        row.bit_depth = t.bit_depth;
        row.color_id = t.colour_id;
        row.rating = t.rating;
        row.file_type = t.file_type;
        row.title = t.title.clone();
        row.comment = t.comment.clone();
        row.mix_name = t.mix_name.clone();
        row.date_added = t.date_added.clone();
        row.analysis_date = model.date.clone();
        row.analysis_path = t.anlz_path.clone();
        row.isrc = valid_isrc(&t.isrc);
        // Players load hot cues with the track only when this says "ON"
        // (as rbxport found); OneLibrary has its own flag for it.
        row.hot_cue_auto_load = "ON".into();
        let made = db.insert_content(row)?;
        if made.id != i as i64 + 1 {
            return Err(Error::Other(format!(
                "track {} got id {} in export.pdb",
                i + 1,
                made.id
            )));
        }
    }

    let mut ids = Vec::with_capacity(model.lists.len());
    for list in &model.lists {
        let parent = list.parent.map(|p| ids[p]);
        let made = if list.folder {
            db.create_playlist_folder(&list.name, parent, None)?
        } else {
            db.create_playlist(&list.name, parent, None)?
        };
        for &track in &list.tracks {
            db.create_playlist_content(made.id, track, None)?;
        }
        ids.push(made.id);
    }

    db.save()?;
    Ok(())
}

/// The ISRC when it is one (twelve letters and digits), else empty: the
/// format has room for nothing else.
pub fn valid_isrc(isrc: &str) -> String {
    let isrc: String = isrc.chars().filter(|c| *c != '-').collect();
    if isrc.len() == 12 && isrc.chars().all(|c| c.is_ascii_alphanumeric()) {
        isrc.to_ascii_uppercase()
    } else {
        String::new()
    }
}
