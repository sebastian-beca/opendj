//! `exportLibrary.db`, the OneLibrary database the CDJ-3000X, XDJ-AZ,
//! OPUS-QUAD and OMNIS-DUO read, written with rbox. It is SQLCipher with
//! the passphrase every stick shares, which rbox supplies.

use std::collections::HashMap;
use std::path::Path;

use chrono::{NaiveDate, TimeZone, Utc};
use diesel::connection::SimpleConnection;
use diesel::{Connection, RunQueryDsl};
use rbox_db::connection::{ConnectionOptions, JournalMode, OpenMode, Synchronous};
use rbox_db::ModelInsert;
use rbox_onelib::{
    establish_connection, initialize_schema, DbConn, NewAlbum, NewArtist, NewContent, NewGenre,
    NewImage, NewKey, NewLabel, NewPlaylist, PlaylistContent, Property,
};

use crate::layout;
use crate::model::{Model, ANALYSED_BITS, CONTENT_LINK, MASTER_DB_ID};
use crate::Error;

type DieselResult<T> = Result<T, diesel::result::Error>;

/// Ids of named rows made on first use; 0 is none.
#[derive(Default)]
struct Names(HashMap<String, i64>);

impl Names {
    fn get(
        &mut self,
        name: &str,
        create: impl FnOnce(&str) -> DieselResult<i64>,
    ) -> DieselResult<Option<i64>> {
        let name = name.trim();
        if name.is_empty() {
            return Ok(None);
        }
        if let Some(&id) = self.0.get(name) {
            return Ok(Some(id));
        }
        let id = create(name)?;
        self.0.insert(name.to_string(), id);
        Ok(Some(id))
    }
}

fn some_text(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_string())
}

/// Writes the database to `path`, which must not exist yet.
pub fn write(path: &Path, model: &Model) -> Result<(), Error> {
    // One transaction and no syncing per row: the file is built on the
    // computer's own disk and copied to the stick once it is complete.
    let options = ConnectionOptions {
        open_mode: OpenMode::ReadWriteCreate,
        journal_mode: JournalMode::Delete,
        synchronous: Synchronous::Normal,
        fullfsync: false,
        // The schema keeps rekordbox's own foreign keys, some of which name
        // columns that don't exist (album.artist_id -> artist.id), so
        // SQLite would refuse any album with an artist. Players don't
        // check them either.
        foreign_keys: false,
        ..ConnectionOptions::default()
    };
    let mut conn = establish_connection(path, &options)?;
    initialize_schema(&mut conn)?;
    // rbox 0.3.0's schema names one column differently from rekordbox (and
    // from rbox's own queries) and leaves out the play count, which
    // rekordbox's sticks have.
    conn.batch_execute(
        "ALTER TABLE content RENAME COLUMN originalartist_id TO artist_id_originalArtist;
         ALTER TABLE content ADD COLUMN djPlayCount INTEGER;",
    )?;
    conn.transaction(|conn| fill(conn, model))?;
    Ok(())
}

fn fill(conn: &mut DbConn, model: &Model) -> DieselResult<()> {
    let created = NaiveDate::parse_from_str(&model.date, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|d| Utc.from_utc_datetime(&d))
        .unwrap_or_else(Utc::now);
    let mut property = Property::new(&model.device_name, MASTER_DB_ID);
    property.number_of_contents = model.tracks.len() as i32;
    property.created_date = created;
    property.insert(conn)?;

    for id in 1..=model.artworks {
        NewImage::new(layout::artwork(id, 'b', false)).insert(conn)?;
    }

    let mut artists = Names::default();
    let mut albums: HashMap<(String, Option<i64>), i64> = HashMap::new();
    let mut genres = Names::default();
    let mut labels = Names::default();
    let mut keys = Names::default();
    for (i, t) in model.tracks.iter().enumerate() {
        let mut artist = |name: &str| NewArtist::new(name).insert(conn).map(|a| a.id);
        let artist_id = artists.get(&t.artist, &mut artist)?;
        let remixer_id = artists.get(&t.remixer, &mut artist)?;
        let composer_id = artists.get(&t.composer, &mut artist)?;
        let album = t.album.trim();
        let album_id = if album.is_empty() {
            None
        } else {
            let key = (album.to_string(), artist_id);
            Some(match albums.get(&key) {
                Some(&id) => id,
                None => {
                    let mut row = NewAlbum::new(album);
                    row.artist_id = artist_id;
                    let id = row.insert(conn)?.id;
                    albums.insert(key, id);
                    id
                }
            })
        };
        let genre_id = genres.get(&t.genre, |n| NewGenre::new(n).insert(conn).map(|r| r.id))?;
        let label_id = labels.get(&t.label, |n| NewLabel::new(n).insert(conn).map(|r| r.id))?;
        let key_id = keys.get(&t.key, |n| NewKey::new(n).insert(conn).map(|r| r.id))?;

        let added = NaiveDate::parse_from_str(&t.date_added, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|d| Utc.from_utc_datetime(&d))
            .unwrap_or(created);
        let mut row = NewContent::new(&t.path);
        row.date_created = added;
        row.date_added = added;
        row.title = some_text(&t.title);
        row.subtitle = some_text(&t.mix_name);
        row.bpmx100 = Some(t.bpmx100);
        row.length = Some(t.length);
        row.track_no = Some(t.track_number);
        row.disc_no = Some(t.disc_number);
        row.artist_id = artist_id;
        row.remixer_id = remixer_id;
        row.composer_id = composer_id;
        row.album_id = album_id;
        row.genre_id = genre_id;
        row.label_id = label_id;
        row.key_id = key_id;
        row.color_id = Some(t.colour_id);
        row.image_id = (t.artwork != 0).then_some(t.artwork);
        row.dj_comment = some_text(&t.comment);
        row.rating = Some(t.rating);
        row.release_year = Some(t.year);
        row.file_name = t.path.rsplit('/').next().map(str::to_string);
        row.file_size = Some(t.file_size);
        row.file_type = Some(t.file_type);
        row.bitrate = Some(t.bitrate);
        row.bit_depth = Some(t.bit_depth);
        row.sampling_rate = Some(t.sample_rate);
        row.isrc = Some(crate::pdb::valid_isrc(&t.isrc));
        row.is_hot_cue_auto_load_on = Some(1);
        row.is_kuvo_deliver_status_on = Some(1);
        row.kuvo_delivery_comment = Some(String::new());
        row.master_db_id = Some(MASTER_DB_ID);
        row.master_content_id = Some(t.library_id);
        row.analysis_data_file_path = Some(t.anlz_path.clone());
        row.analysed_bits = Some(ANALYSED_BITS);
        row.content_link = Some(CONTENT_LINK);
        row.has_modified = Some(0);
        let made = row.insert(conn)?;
        diesel::sql_query("UPDATE content SET djPlayCount = ? WHERE content_id = ?")
            .bind::<diesel::sql_types::Integer, _>(t.play_count)
            .bind::<diesel::sql_types::BigInt, _>(made.id)
            .execute(conn)?;
        if made.id != i as i64 + 1 {
            return Err(diesel::result::Error::QueryBuilderError(
                format!("track {} got id {} in OneLibrary", i + 1, made.id).into(),
            ));
        }
    }

    let mut ids: Vec<i64> = Vec::with_capacity(model.lists.len());
    for list in &model.lists {
        let mut row = if list.folder {
            NewPlaylist::folder(&list.name)
        } else {
            NewPlaylist::playlist(&list.name)
        };
        row.parent_id = list.parent.map_or(0, |p| ids[p]);
        let made = row.insert(conn)?;
        for &track in &list.tracks {
            PlaylistContent {
                playlist_id: made.id,
                content_id: track,
                seq: -1,
            }
            .insert(conn)?;
        }
        ids.push(made.id);
    }
    Ok(())
}
