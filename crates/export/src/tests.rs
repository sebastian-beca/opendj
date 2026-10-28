//! An export of real (made-up) audio to a folder, read back with rbox.

use super::*;
use mixxx_engine::scan;
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

    // Stopping leaves the library as it was.
    let err = export(&tracks, &lists, &options, &mut |_| false).unwrap_err();
    assert!(matches!(err, Error::Cancelled));
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
