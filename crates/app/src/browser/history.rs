//! The history tab's sessions rolled up by day: one row per day, its sets
//! under it. A DJ's night runs past midnight, so a day starts at 6:00, and
//! a set ends when nothing has played for an hour.

use chrono::{Duration, Local, NaiveDate, TimeZone};

/// Plays before this time of day (seconds after midnight) count to the
/// day before.
pub const DAY_STARTS_AT: i64 = 6 * 3600;

/// After this long with nothing played (seconds), the next play starts a
/// new set.
pub const SET_GAP: i64 = 60 * 60;

/// Now, in Unix seconds.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// The day a moment belongs to, on the DJ's clock: a set at 01:30 is part
/// of the night before.
pub fn day_of(at: i64) -> NaiveDate {
    match Local.timestamp_opt(at - DAY_STARTS_AT, 0).single() {
        Some(t) => t.date_naive(),
        None => NaiveDate::default(),
    }
}

/// The day a list named only by its date is from ("2026-11-04", as
/// VirtualDJ names its history files).
pub fn day_named(name: &str) -> Option<NaiveDate> {
    let name = name.trim();
    if name.len() != 10 {
        return None;
    }
    NaiveDate::parse_from_str(name, "%Y-%m-%d").ok()
}

/// Whether a play at `now` starts a new set after one at `last`: an hour
/// went by with nothing played, or a new day began.
pub fn new_set(last: i64, now: i64) -> bool {
    now - last > SET_GAP || day_of(last) != day_of(now)
}

/// A day row's name: "Today", "Yesterday", else "Wed 4 Nov", with the year
/// when it isn't this year's.
pub fn day_label(day: NaiveDate, today: NaiveDate) -> String {
    use chrono::Datelike;
    if day == today {
        "Today".into()
    } else if Some(day) == today.checked_sub_signed(Duration::days(1)) {
        "Yesterday".into()
    } else if day.year() == today.year() {
        day.format("%a %-d %b").to_string()
    } else {
        day.format("%a %-d %b %Y").to_string()
    }
}

/// A day in full, over its tracks: "Wed 4 Nov 2026".
pub fn day_title(day: NaiveDate) -> String {
    day.format("%a %-d %b %Y").to_string()
}

/// A history session, as the tree gets it from the library.
pub struct Session<'a> {
    pub id: i64,
    pub name: &'a str,
    /// When it started (Unix seconds).
    pub started: i64,
    /// The app it was brought in from ("VirtualDJ"), for imported ones.
    pub from: Option<&'a str>,
    pub songs: usize,
}

/// A set as its day row shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Set {
    pub id: i64,
    /// Its start time ("21:20"), or for one brought in from another app
    /// without one, its name or the app's.
    pub label: String,
    pub songs: usize,
}

/// A day with its sets, oldest first.
#[derive(Clone, Debug, PartialEq)]
pub struct Day {
    pub date: NaiveDate,
    pub sets: Vec<Set>,
}

impl Day {
    pub fn songs(&self) -> usize {
        self.sets.iter().map(|s| s.songs).sum()
    }
}

/// The sessions by day, newest day first, each day's sets in the order
/// they were played. A list named by its date alone (a whole day brought
/// in from VirtualDJ) goes under that day, after the day's own sets.
pub fn by_day(sessions: &[Session]) -> Vec<Day> {
    let mut keyed: Vec<(NaiveDate, i64, i64, Set)> = sessions
        .iter()
        .map(|s| {
            let named = day_named(s.name);
            let date = named.unwrap_or_else(|| day_of(s.started));
            let label = match (named, s.from) {
                (Some(_), Some(app)) => app.to_string(),
                (Some(_), None) => s.name.to_string(),
                (None, Some(_)) => s.name.to_string(),
                (None, None) => super::columns::time_of_day(s.started),
            };
            let at = if named.is_some() { i64::MAX } else { s.started };
            (
                date,
                at,
                s.id,
                Set {
                    id: s.id,
                    label,
                    songs: s.songs,
                },
            )
        })
        .collect();
    keyed.sort_by_key(|(date, at, id, _)| (std::cmp::Reverse(*date), *at, *id));
    let mut days: Vec<Day> = Vec::new();
    for (date, _, _, set) in keyed {
        match days.last_mut() {
            Some(d) if d.date == date => d.sets.push(set),
            _ => days.push(Day {
                date,
                sets: vec![set],
            }),
        }
    }
    days
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;

    /// Unix seconds at a time on the local clock.
    fn at(text: &str) -> i64 {
        let t = NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M").unwrap();
        Local.from_local_datetime(&t).single().unwrap().timestamp()
    }

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn nights_run_until_six() {
        assert_eq!(day_of(at("2026-11-03 00:22")), date("2026-11-02"));
        assert_eq!(day_of(at("2026-11-03 05:59")), date("2026-11-02"));
        assert_eq!(day_of(at("2026-11-03 06:00")), date("2026-11-03"));
        assert_eq!(day_of(at("2026-11-03 23:59")), date("2026-11-03"));
    }

    #[test]
    fn an_hour_of_quiet_ends_a_set() {
        let t = at("2026-11-02 21:00");
        assert!(!new_set(t, t + 3 * 60));
        assert!(!new_set(t, t + SET_GAP));
        assert!(new_set(t, t + SET_GAP + 1));
        assert!(new_set(t, t + 340 * 60));
        // Playing on past 6:00 starts the next day's set.
        assert!(new_set(at("2026-11-03 05:50"), at("2026-11-03 06:05")));
        assert!(!new_set(at("2026-11-02 23:50"), at("2026-11-03 00:10")));
    }

    #[test]
    fn day_labels() {
        let today = date("2026-11-05");
        assert_eq!(day_label(today, today), "Today");
        assert_eq!(day_label(date("2026-11-04"), today), "Yesterday");
        assert_eq!(day_label(date("2026-11-02"), today), "Mon 2 Nov");
        assert_eq!(day_label(date("2025-12-31"), today), "Wed 31 Dec 2025");
        assert_eq!(day_title(date("2026-11-02")), "Mon 2 Nov 2026");
        assert_eq!(day_named("2026-11-04"), Some(date("2026-11-04")));
        assert_eq!(day_named("Thu  5 Nov 2026, 12:55"), None);
    }

    #[test]
    fn sessions_roll_up_by_day() {
        let s = |id, name, started, from, songs| Session {
            id,
            name,
            started,
            from,
            songs,
        };
        let days = by_day(&[
            s(
                1,
                "Mon  2 Nov 2026, 21:20",
                at("2026-11-02 21:20"),
                None,
                26,
            ),
            s(2, "Mon  2 Nov 2026, 23:39", at("2026-11-02 23:39"), None, 9),
            s(3, "Tue  3 Nov 2026, 00:22", at("2026-11-03 00:22"), None, 4),
            s(
                4,
                "Wed  4 Nov 2026, 18:02",
                at("2026-11-04 18:02"),
                None,
                25,
            ),
            // A VirtualDJ day whose plays carry a later time.
            s(
                5,
                "2026-11-04",
                at("2026-11-05 09:00"),
                Some("VirtualDJ"),
                12,
            ),
            s(6, "Thu  5 Nov 2026, 12:55", at("2026-11-05 12:55"), None, 1),
        ]);
        let ids: Vec<(NaiveDate, Vec<i64>)> = days
            .iter()
            .map(|d| (d.date, d.sets.iter().map(|s| s.id).collect()))
            .collect();
        assert_eq!(
            ids,
            vec![
                (date("2026-11-05"), vec![6]),
                (date("2026-11-04"), vec![4, 5]),
                (date("2026-11-02"), vec![1, 2, 3]),
            ]
        );
        assert_eq!(days[1].sets[0].label, "18:02");
        assert_eq!(days[1].sets[1].label, "VirtualDJ");
        assert_eq!(days[2].songs(), 39);
    }
}
