//! The Prep checklist's marks: the lines the DJ ticked on each track, and
//! the personal score they gave it. The lines themselves are the app's
//! (see the app's Checklist tab) and the DJ's own, from the settings; a
//! tick is kept by the line's name.

use std::collections::HashMap;

use rusqlite::params;

use crate::{row_id, track_id, Error, Library};

pub(crate) const CHECKLIST_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS prep_ticks (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    line TEXT NOT NULL,
    PRIMARY KEY (track_id, line)
);
CREATE TABLE IF NOT EXISTS prep_scores (
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    score INTEGER NOT NULL
);
";

/// The highest personal score.
pub const MAX_SCORE: u8 = 10;

/// What the DJ marked on one track.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    /// The lines ticked by hand, by name.
    pub ticks: Vec<String>,
    /// 1 to [`MAX_SCORE`], when given.
    pub score: Option<u8>,
}

impl Library {
    /// Every track's marks, by track id.
    pub fn prep_marks(&self) -> Result<HashMap<usize, Marks>, Error> {
        let mut out: HashMap<usize, Marks> = HashMap::new();
        let mut q = self
            .db
            .prepare("SELECT track_id, line FROM prep_ticks ORDER BY line")?;
        for row in q.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (row, line) = row?;
            out.entry(track_id(row)).or_default().ticks.push(line);
        }
        let mut q = self.db.prepare("SELECT track_id, score FROM prep_scores")?;
        for row in q.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
            let (row, score) = row?;
            out.entry(track_id(row)).or_default().score =
                Some(score.clamp(1, MAX_SCORE as i64) as u8);
        }
        Ok(out)
    }

    /// Ticks `line` on a track, or (with `on` false) takes the tick off.
    pub fn set_prep_tick(&self, id: usize, line: &str, on: bool) -> Result<(), Error> {
        let Some(row) = row_id(id) else { return Ok(()) };
        if on {
            self.db.execute(
                "INSERT OR IGNORE INTO prep_ticks (track_id, line) VALUES (?1, ?2)",
                params![row, line],
            )?;
        } else {
            self.db.execute(
                "DELETE FROM prep_ticks WHERE track_id = ?1 AND line = ?2",
                params![row, line],
            )?;
        }
        Ok(())
    }

    /// Gives a track a personal score, or takes it off.
    pub fn set_prep_score(&self, id: usize, score: Option<u8>) -> Result<(), Error> {
        let Some(row) = row_id(id) else { return Ok(()) };
        match score.filter(|&s| s > 0) {
            Some(s) => self.db.execute(
                "INSERT OR REPLACE INTO prep_scores (track_id, score) VALUES (?1, ?2)",
                params![row, s.min(MAX_SCORE) as i64],
            )?,
            None => self
                .db
                .execute("DELETE FROM prep_scores WHERE track_id = ?1", [row])?,
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_and_scores_are_kept_per_track() {
        let lib = Library::in_memory().unwrap();
        lib.db
            .execute("INSERT INTO tracks (id, path) VALUES (1, '/a.mp3')", [])
            .unwrap();
        let id = track_id(1);
        lib.set_prep_tick(id, "Vocals checked", true).unwrap();
        lib.set_prep_tick(id, "Vocals checked", true).unwrap();
        lib.set_prep_tick(id, "Intro clean", true).unwrap();
        lib.set_prep_score(id, Some(12)).unwrap();
        let marks = lib.prep_marks().unwrap();
        assert_eq!(
            marks[&id],
            Marks {
                ticks: vec!["Intro clean".into(), "Vocals checked".into()],
                score: Some(MAX_SCORE),
            }
        );
        lib.set_prep_tick(id, "Intro clean", false).unwrap();
        lib.set_prep_score(id, None).unwrap();
        assert_eq!(lib.prep_marks().unwrap()[&id].ticks, vec!["Vocals checked"]);
        assert_eq!(lib.prep_marks().unwrap()[&id].score, None);
    }
}
