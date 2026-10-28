//! Looking up a track's tags online: its audio fingerprint (Chromaprint,
//! as AcoustID uses) finds the recording on MusicBrainz when the DJ gave
//! an AcoustID key; otherwise MusicBrainz is searched by artist and title.
//! Discogs adds the label, year and style when the DJ gave a Discogs
//! token. Nothing is saved here: what is found goes to the clean-up's
//! review table.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use mixxx_engine::decode::Decoder;
use mixxx_plugins::encode;
use rusty_chromaprint::{Configuration, Fingerprinter};
use serde_json::Value;

use crate::coverart::{get_json, lucene};

/// Seconds of audio fingerprinted, as `fpcalc` does.
const FINGERPRINT_SECONDS: usize = 120;

/// What to look up a track by.
#[derive(Clone, Debug, Default)]
pub struct Query {
    pub path: Option<PathBuf>,
    pub artist: String,
    pub title: String,
    /// Seconds.
    pub duration: f64,
    pub acoustid_key: String,
    pub discogs_token: String,
}

/// What was found for a track, and how sure the match is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    pub artist: String,
    pub title: String,
    pub album: String,
    pub year: String,
    pub label: String,
    pub genre: String,
    /// From 0 to 1.
    pub score: f32,
    /// Where the match came from: "AcoustID" or "MusicBrainz".
    pub source: &'static str,
}

/// Looks a track up; `None` when nothing matched.
pub fn look_up(q: &Query) -> Option<Found> {
    let mut found = by_fingerprint(q).or_else(|| by_name(q))?;
    if !q.discogs_token.trim().is_empty() {
        if let Some(d) = discogs(q, &found) {
            if found.year.is_empty() {
                found.year = d.year;
            }
            found.label = d.label;
            found.genre = d.genre;
        }
    }
    Some(found)
}

/// The recording AcoustID knows by the track's fingerprint, with its
/// releases from MusicBrainz.
fn by_fingerprint(q: &Query) -> Option<Found> {
    if q.acoustid_key.trim().is_empty() {
        return None;
    }
    let (fingerprint, seconds) = fingerprint(q.path.as_ref()?)?;
    let duration = if q.duration > 0.0 {
        q.duration
    } else {
        seconds
    };
    let url = format!(
        "https://api.acoustid.org/v2/lookup?client={}&meta=recordings&duration={}&fingerprint={}",
        encode(q.acoustid_key.trim()),
        duration.round() as u64,
        fingerprint
    );
    let (id, score) = acoustid_best(&get_json(&url, None)?)?;
    let v = get_json(
        &format!(
            "https://musicbrainz.org/ws/2/recording/{id}?inc=artist-credits+releases&fmt=json"
        ),
        None,
    )?;
    let mut found = recording(&v)?;
    found.score = score;
    found.source = "AcoustID";
    Some(found)
}

/// The recording MusicBrainz finds by artist and title.
fn by_name(q: &Query) -> Option<Found> {
    if q.artist.trim().is_empty() || q.title.trim().is_empty() {
        return None;
    }
    let query = format!(
        "recording:\"{}\" AND artist:\"{}\"",
        lucene(q.title.trim()),
        lucene(q.artist.trim())
    );
    let url = format!(
        "https://musicbrainz.org/ws/2/recording?fmt=json&limit=5&query={}",
        encode(&query)
    );
    musicbrainz_best(&get_json(&url, None)?, q.duration)
}

/// The track's Chromaprint fingerprint, compressed and encoded as
/// AcoustID takes it, and the file's length in seconds.
pub fn fingerprint(path: &Path) -> Option<(String, f64)> {
    let audio = Decoder::open(path).ok()?.run(|_| true).ok()?;
    let rate = audio.sample_rate;
    let config = Configuration::preset_test2();
    let mut printer = Fingerprinter::new(&config);
    printer.start(rate, 2).ok()?;
    let frames = audio.len().min(FINGERPRINT_SECONDS * rate as usize);
    let mut samples = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let [l, r] = audio.frame(i);
        samples.push((l.clamp(-1.0, 1.0) * 32767.0) as i16);
        samples.push((r.clamp(-1.0, 1.0) * 32767.0) as i16);
    }
    printer.consume(&samples);
    printer.finish();
    let compressed =
        rusty_chromaprint::FingerprintCompressor::from(&config).compress(printer.fingerprint());
    let text = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(compressed);
    Some((text, audio.duration()))
}

/// The best recording in an AcoustID answer: its MusicBrainz id and score.
fn acoustid_best(v: &Value) -> Option<(String, f32)> {
    v["results"]
        .as_array()?
        .iter()
        .filter_map(|r| {
            let score = r["score"].as_f64()? as f32;
            let id = r["recordings"].as_array()?.first()?["id"].as_str()?;
            Some((id.to_string(), score))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
}

/// A MusicBrainz recording's artist, title, and its first official album
/// (the earliest), from a lookup or a search result.
fn recording(r: &Value) -> Option<Found> {
    let title = r["title"].as_str()?.to_string();
    let artist: String = r["artist-credit"]
        .as_array()?
        .iter()
        .map(|c| {
            format!(
                "{}{}",
                c["name"].as_str().unwrap_or(""),
                c["joinphrase"].as_str().unwrap_or("")
            )
        })
        .collect();
    let mut releases: Vec<&Value> = r["releases"].as_array().into_iter().flatten().collect();
    // Official albums first, then the earliest.
    releases.sort_by_key(|rel| {
        let official = rel["status"].as_str() == Some("Official");
        let date = rel["date"]
            .as_str()
            .filter(|d| !d.is_empty())
            .unwrap_or("9999");
        (!official, date.to_string())
    });
    let first = releases.first();
    let album = first
        .and_then(|rel| rel["title"].as_str())
        .unwrap_or("")
        .to_string();
    let year = first
        .and_then(|rel| rel["date"].as_str())
        .and_then(|d| d.get(..4))
        .unwrap_or("")
        .to_string();
    Some(Found {
        artist: artist.trim().to_string(),
        title,
        album,
        year,
        source: "MusicBrainz",
        ..Default::default()
    })
}

/// The best search result: the highest score, then the one closest in
/// length. A length more than 10 seconds off counts against it.
fn musicbrainz_best(v: &Value, duration: f64) -> Option<Found> {
    let best = v["recordings"].as_array()?.iter().max_by_key(|r| {
        let mut score = r["score"].as_i64().unwrap_or(0);
        if let (Some(ms), true) = (r["length"].as_f64(), duration > 0.0) {
            if (ms / 1000.0 - duration).abs() > 10.0 {
                score -= 30;
            }
        }
        score
    })?;
    let mut found = recording(best)?;
    let mut score = best["score"].as_f64().unwrap_or(0.0) / 100.0;
    if let (Some(ms), true) = (best["length"].as_f64(), duration > 0.0) {
        if (ms / 1000.0 - duration).abs() > 10.0 {
            score *= 0.7;
        }
    }
    found.score = score as f32;
    Some(found)
}

/// What Discogs adds: the year, label and style of the release.
fn discogs(q: &Query, found: &Found) -> Option<Found> {
    let artist = if found.artist.is_empty() {
        &q.artist
    } else {
        &found.artist
    };
    let mut url = format!(
        "https://api.discogs.com/database/search?type=release&per_page=5&artist={}",
        encode(artist.trim())
    );
    if !found.album.is_empty() {
        url += &format!("&release_title={}", encode(&found.album));
    } else {
        url += &format!("&track={}", encode(&found.title));
    }
    let auth = format!("Discogs token={}", q.discogs_token.trim());
    discogs_first(&get_json(&url, Some(&auth))?)
}

fn discogs_first(v: &Value) -> Option<Found> {
    let r = v["results"].as_array()?.first()?;
    let first = |key: &str| {
        r[key]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string()
    };
    let style = first("style");
    Some(Found {
        year: r["year"].as_str().unwrap_or("").to_string(),
        label: first("label"),
        genre: if style.is_empty() {
            first("genre")
        } else {
            style
        },
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_acoustid_and_musicbrainz_answers() {
        let v = json!({"status": "ok", "results": [
            {"score": 0.42, "recordings": [{"id": "a"}]},
            {"score": 0.97, "recordings": [{"id": "b"}]},
            {"score": 0.99}
        ]});
        assert_eq!(acoustid_best(&v), Some(("b".into(), 0.97)));
        let r = json!({
            "title": "Kerala",
            "artist-credit": [{"name": "Bonobo", "joinphrase": " feat. "}, {"name": "X"}],
            "releases": [
                {"title": "Kerala (Remixes)", "date": "2017-06-02", "status": "Official"},
                {"title": "Migration", "date": "2017-01-13", "status": "Official"},
                {"title": "Bootleg", "date": "2016", "status": "Bootleg"}
            ]
        });
        let f = recording(&r).unwrap();
        assert_eq!(
            (
                f.artist.as_str(),
                f.title.as_str(),
                f.album.as_str(),
                f.year.as_str()
            ),
            ("Bonobo feat. X", "Kerala", "Migration", "2017")
        );
    }

    #[test]
    fn prefers_matches_of_the_right_length() {
        let v = json!({"recordings": [
            {"score": 100, "title": "Clouds", "length": 600000, "artist-credit": [{"name": "A"}]},
            {"score": 95, "title": "Clouds", "length": 271000, "artist-credit": [{"name": "A"}]}
        ]});
        let f = musicbrainz_best(&v, 271.5).unwrap();
        assert!((f.score - 0.95).abs() < 1e-6);
        let v = json!({"recordings": [
            {"score": 100, "title": "Clouds", "length": 600000, "artist-credit": [{"name": "A"}]}
        ]});
        assert!(musicbrainz_best(&v, 271.5).unwrap().score < 0.8);
    }

    #[test]
    fn reads_discogs_styles() {
        let v = json!({"results": [{"year": "2014", "label": ["Ninja Tune", "x"],
            "genre": ["Electronic"], "style": ["Downtempo", "Ambient"]}]});
        let d = discogs_first(&v).unwrap();
        assert_eq!(
            (d.year.as_str(), d.label.as_str(), d.genre.as_str()),
            ("2014", "Ninja Tune", "Downtempo")
        );
    }

    #[test]
    fn fingerprints_audio() {
        let dir = std::env::temp_dir().join(format!("opendj-fp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tone.wav");
        let rate = 44100u32;
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for i in 0..rate * 10 {
            let t = i as f32 / rate as f32;
            let s = ((t * 440.0 * std::f32::consts::TAU).sin() * (t * 3.0).sin() * 12000.0) as i16;
            w.write_sample(s).unwrap();
            w.write_sample(s).unwrap();
        }
        w.finalize().unwrap();
        let (fp, seconds) = fingerprint(&path).unwrap();
        assert!(fp.len() > 20, "{fp}");
        assert!((seconds - 10.0).abs() < 0.1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
