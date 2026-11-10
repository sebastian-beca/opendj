//! An export of real (made-up) audio to a folder, read back with rbox.

use super::*;
use opendj_engine::scan;
use rbox_devlib::DeviceLibrary;
use rbox_onelib::OneLibrary;

/// A 16-bit mono WAV of clicks at 120 BPM.
fn click_track(path: &Path, seconds: f64) {
    let rate = 44100u32;
    let n = (seconds * rate as f64) as usize;
    let beat = rate as usize / 2;
    let samples: Vec<i16> = (0..n)
        .map(|i| {
            let t = i % beat;
            if t < 2000 {
                let env = 1.0 - t as f64 / 2000.0;
                let s = (i as f64 * 2.0 * std::f64::consts::PI * 80.0 / rate as f64).sin();
                (s * env * 30000.0) as i16
            } else {
                0
            }
        })
        .collect();
    let mut wav = Vec::new();
    let data = (samples.len() * 2) as u32;
    wav.extend(b"RIFF");
    wav.extend((36 + data).to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16u32.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(rate.to_le_bytes());
    wav.extend((rate * 2).to_le_bytes());
    wav.extend(2u16.to_le_bytes());
    wav.extend(16u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend(data.to_le_bytes());
    for s in samples {
        wav.extend(s.to_le_bytes());
    }
    fs::write(path, wav).unwrap();
}

fn hotcue(number: usize, seconds: f64) -> Hotcue {
    Hotcue {
        number,
        seconds,
        name: format!("cue {number}"),
        end: None,
        color: Some(0xE62828),
    }
}

#[test]
fn an_export_reads_back_in_both_formats() {
    let music = tempfile::tempdir().unwrap();
    let stick = tempfile::tempdir().unwrap();
    // The analysis cache lives beside the music, not in the real one.
    std::env::set_var("OPENDJ_CACHE", music.path().join("cache"));

    let mut tracks = Vec::new();
    for (i, (artist, title)) in [("Ana", "One"), ("Ben", "Two"), ("Ana", "Three")]
        .iter()
        .enumerate()
    {
        let path = music.path().join(format!("{title}.wav"));
        click_track(&path, 6.0);
        let mut t = scan::read_track(i + 10, &path);
        t.artist = artist.to_string();
        t.title = title.to_string();
        t.album = "Clicks".into();
        t.key = "Am".into();
        t.info.hotcues = (0..10).map(|n| hotcue(n, 0.5 + n as f64 * 0.5)).collect();
        tracks.push(t);
    }
    let mut gone = scan::read_track(99, &music.path().join("gone.wav"));
    gone.title = "Gone".into();
    tracks.push(gone);

    let lists = vec![
        List {
            name: "Gigs".into(),
            folder: true,
            ..List::default()
        },
        List {
            name: "Friday".into(),
            parent: Some(0),
            tracks: vec![0, 1, 0, 3],
            ..List::default()
        },
        List {
            name: "Warm-up".into(),
            tracks: vec![2, 1],
            ..List::default()
        },
    ];
    let options = Options {
        root: stick.path().to_path_buf(),
        name: "TEST".into(),
        device_library: true,
        one_library: true,
        engine_dj: false,
        camelot: true,
    };
    let mut steps = Vec::new();
    let report = export(&tracks, &lists, &options, &mut |s| {
        steps.push(s);
        true
    })
    .unwrap();
    assert_eq!(report.tracks, 3);
    assert_eq!(report.copied, 3);
    assert_eq!(report.lists, 2);
    assert_eq!(report.skipped.len(), 1, "{:?}", report.skipped);
    assert_eq!(report.hot_cues_dropped, 3 * 2, "cues I and J on each track");
    assert_eq!(steps.last(), Some(&Step::Library));

    let rekordbox = stick.path().join("PIONEER/rekordbox");
    let mut pdb = DeviceLibrary::new(&rekordbox).unwrap();
    let contents = pdb.get_contents().unwrap();
    let titles: Vec<_> = contents.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(titles, ["One", "Two", "Three"]);
    let one = &contents[0];
    assert_eq!(one.master_content_id, 10);
    assert!(
        (11900..=12100).contains(&one.bpmx100),
        "bpm {}",
        one.bpmx100
    );
    assert_eq!(pdb.get_key_by_id(one.key_id).unwrap().unwrap().name, "8A");
    assert_eq!(
        pdb.get_artist_by_id(one.artist_id).unwrap().unwrap().name,
        "Ana"
    );
    assert_eq!(contents[2].artist_id, one.artist_id);
    assert_eq!(one.hot_cue_auto_load, "ON");
    let playlists = pdb.get_playlists().unwrap();
    let friday = playlists.iter().find(|p| p.name == "Friday").unwrap();
    let gigs = playlists.iter().find(|p| p.name == "Gigs").unwrap();
    assert_eq!(friday.parent_id, gigs.id);
    let in_friday: Vec<_> = pdb
        .get_playlist_contents(friday.id)
        .unwrap()
        .into_iter()
        .map(|c| c.title)
        .collect();
    assert_eq!(in_friday, ["One", "Two"]);

    let one_lib = OneLibrary::new(&rekordbox).unwrap();
    let contents = one_lib.get_contents().unwrap();
    assert_eq!(contents.len(), 3);
    assert_eq!(contents[1].title.as_deref(), Some("Two"));
    let playlists = one_lib.get_playlists().unwrap();
    let warm = playlists.iter().find(|p| p.name == "Warm-up").unwrap();
    let in_warm: Vec<_> = one_lib
        .get_playlist_contents(warm.id)
        .unwrap()
        .into_iter()
        .map(|c| c.title.unwrap_or_default())
        .collect();
    assert_eq!(in_warm, ["Three", "Two"]);

    // Audio, analysis files and the paths the databases point at line up.
    for c in pdb.get_contents().unwrap() {
        let path = pdb.get_content_by_id(c.id).unwrap().unwrap().path;
        assert!(on_stick(stick.path(), &path).is_file(), "{path}");
        let anlz = on_stick(stick.path(), &c.analysis_path);
        assert!(anlz.is_file(), "{}", c.analysis_path);
    }
    let audio = stick.path().join("Contents/Ana/Clicks/One.wav");
    assert!(audio.is_file());
    let anlz: Vec<_> = walk(&stick.path().join("PIONEER/USBANLZ"));
    assert_eq!(anlz.len(), 9, "{anlz:?}");

    // Windows won't replace files that are still open.
    drop((pdb, one_lib));

    // A second export keeps the audio and replaces the library.
    let lists = vec![List {
        name: "Only".into(),
        tracks: vec![1],
        ..List::default()
    }];
    let options = Options {
        one_library: false,
        ..options
    };
    let report = export(&tracks, &lists, &options, &mut |_| true).unwrap();
    assert_eq!((report.tracks, report.copied), (1, 0));
    assert!(!rekordbox.join("exportLibrary.db").exists());
    let mut pdb = DeviceLibrary::new(&rekordbox).unwrap();
    assert_eq!(pdb.get_contents().unwrap().len(), 1);
    drop(pdb);

    // Nothing to export leaves the stick alone.
    let empty = tempfile::tempdir().unwrap();
    let only_gone = vec![List {
        name: "Gone".into(),
        tracks: vec![3],
        ..List::default()
    }];
    let options_empty = Options {
        root: empty.path().to_path_buf(),
        ..options.clone()
    };
    assert!(export(&tracks, &only_gone, &options_empty, &mut |_| true).is_err());
    assert!(!empty.path().join("PIONEER/rekordbox").exists());

    // A stick rekordbox exported hidden, as .PIONEER, is written there.
    let hidden = tempfile::tempdir().unwrap();
    fs::create_dir_all(hidden.path().join(".PIONEER/rekordbox")).unwrap();
    fs::write(hidden.path().join(".PIONEER/rekordbox/export.pdb"), b"old").unwrap();
    let options_hidden = Options {
        root: hidden.path().to_path_buf(),
        ..options.clone()
    };
    export(&tracks, &lists, &options_hidden, &mut |_| true).unwrap();
    assert!(!hidden.path().join("PIONEER").exists());
    let mut pdb = DeviceLibrary::new(hidden.path().join(".PIONEER/rekordbox")).unwrap();
    let c = &pdb.get_contents().unwrap()[0];
    assert!(
        c.analysis_path.starts_with("/.PIONEER/USBANLZ/"),
        "{}",
        c.analysis_path
    );
    assert!(on_stick(hidden.path(), &c.analysis_path).is_file());

    // Stopping leaves the library as it was.
    let err = export(&tracks, &lists, &options, &mut |_| false).unwrap_err();
    assert!(matches!(err, Error::Cancelled));
}

#[test]
fn an_engine_dj_export_reads_back() {
    use rusqlite::Connection;
    let music = tempfile::tempdir().unwrap();
    let stick = tempfile::tempdir().unwrap();
    std::env::set_var("OPENDJ_CACHE", music.path().join("cache"));

    let mut tracks = Vec::new();
    for (i, title) in ["One", "Two"].iter().enumerate() {
        let path = music.path().join(format!("{title}.wav"));
        click_track(&path, 6.0);
        let mut t = scan::read_track(i + 1, &path);
        t.artist = "Ana".into();
        t.title = title.to_string();
        t.album = "Clicks".into();
        t.key = "Am".into();
        t.rating = 4;
        t.info.hotcues = vec![hotcue(0, 0.5), hotcue(9, 1.0)];
        tracks.push(t);
    }
    let lists = vec![
        List {
            name: "Gigs".into(),
            folder: true,
            ..List::default()
        },
        List {
            name: "Friday".into(),
            parent: Some(0),
            tracks: vec![1, 0],
            ..List::default()
        },
        List {
            name: "Friday".into(),
            parent: Some(0),
            tracks: vec![0],
            ..List::default()
        },
    ];
    let options = Options {
        root: stick.path().to_path_buf(),
        name: "TEST".into(),
        device_library: false,
        one_library: false,
        engine_dj: true,
        camelot: true,
    };
    let report = export(&tracks, &lists, &options, &mut |_| true).unwrap();
    assert_eq!((report.tracks, report.lists), (2, 2));
    assert!(!stick.path().join("PIONEER").exists(), "no Pioneer files");

    let db = Connection::open(stick.path().join("Engine Library/Database2/m.db")).unwrap();
    let version: (i64, i64, i64) = db
        .query_row(
            "SELECT schemaVersionMajor, schemaVersionMinor, schemaVersionPatch FROM Information",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(version, (3, 0, 2));
    let rows: Vec<(String, String, i64, i64, i64, bool)> = db
        .prepare("SELECT title, path, key, rating, bpm, isAnalyzed FROM Track ORDER BY id")
        .unwrap()
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(rows.len(), 2);
    // Tracks come in the order the lists first hold them: Friday is Two, One.
    let (title, path, key, rating, bpm, analysed) = &rows[1];
    assert_eq!(title, "One");
    assert_eq!(path, "../Contents/Ana/Clicks/One.wav");
    assert!(stick.path().join("Engine Library").join(path).is_file());
    assert_eq!((*key, *rating, *bpm), (1, 80, 120));
    assert!(*analysed, "a grid and a waveform are there");

    // The performance data decodes: grid from the first beat, cue A only.
    let (beats, cues): (Vec<u8>, Vec<u8>) = db
        .query_row(
            "SELECT beatData, quickCues FROM PerformanceData WHERE trackId = 2",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let beats = engine::perf::tests::unq(&beats);
    assert_eq!(f64::from_be_bytes(beats[..8].try_into().unwrap()), 44100.0);
    assert!(u64::from_be_bytes(beats[17..25].try_into().unwrap()) >= 2);
    let cues = engine::perf::tests::unq(&cues);
    assert_eq!(&cues[8..14], b"\x05cue 0");

    // Lists in their folder, in order, with unique names.
    let lists: Vec<(i64, String, i64)> = db
        .prepare("SELECT id, title, parentListId FROM Playlist ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let names: Vec<&str> = lists.iter().map(|l| l.1.as_str()).collect();
    assert_eq!(names, ["Gigs", "Friday", "Friday (2)"]);
    assert_eq!(lists[1].2, lists[0].0);
    let order: Vec<String> = db
        .prepare(
            "WITH RECURSIVE chain(id, trackId, nextEntityId) AS ( \
               SELECT id, trackId, nextEntityId FROM PlaylistEntity \
                 WHERE listId = ?1 AND id NOT IN \
                   (SELECT nextEntityId FROM PlaylistEntity WHERE listId = ?1) \
               UNION ALL SELECT e.id, e.trackId, e.nextEntityId FROM PlaylistEntity e \
                 JOIN chain c ON e.id = c.nextEntityId) \
             SELECT t.title FROM chain JOIN Track t ON t.id = chain.trackId",
        )
        .unwrap()
        .query_map([lists[1].0], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(order, ["Two", "One"]);
    drop(db);

    // Exporting again replaces openDJ's own library.
    export(&tracks, &lists_one(), &options, &mut |_| true).unwrap();
    let db = Connection::open(stick.path().join("Engine Library/Database2/m.db")).unwrap();
    let n: i64 = db
        .query_row("SELECT count(*) FROM Track", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
    drop(db);

    // A library Engine DJ wrote is left alone.
    let other = tempfile::tempdir().unwrap();
    let db2 = other.path().join("Engine Library/Database2");
    fs::create_dir_all(&db2).unwrap();
    fs::write(db2.join("m.db"), b"engine's").unwrap();
    let options_other = Options {
        root: other.path().to_path_buf(),
        ..options.clone()
    };
    assert!(export(&tracks, &lists_one(), &options_other, &mut |_| true).is_err());
    assert_eq!(fs::read(db2.join("m.db")).unwrap(), b"engine's");
}

fn lists_one() -> Vec<List> {
    vec![List {
        name: "Only".into(),
        tracks: vec![0],
        ..List::default()
    }]
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}
