//! MusicBrainz, the open music database: from a recording's ISRC, the
//! pages where it's sold ("purchase for download" links), which often
//! include the artist's own Bandcamp or shop.
//!
//! API: `GET https://musicbrainz.org/ws/2/isrc/<ISRC>?inc=url-rels&fmt=json`
//! answers `{"recordings": [{"relations": [{"type": ..., "url":
//! {"resource": ...}}]}]}`. MusicBrainz asks for at most one request a
//! second and a User-Agent that says who's asking.

use std::time::{Duration, Instant};

use serde_json::Value;

use crate::{is_web_url, links, PluginError, USER_AGENT};

/// A page where a recording is sold, and the store it's on (a built-in
/// store's id, or "artist").
#[derive(Clone, Debug, PartialEq)]
pub struct SoldAt {
    pub store: &'static str,
    pub url: String,
}

pub struct MusicBrainz {
    agent: ureq::Agent,
    last: Option<Instant>,
}

impl MusicBrainz {
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(8)))
            .user_agent(USER_AGENT)
            .build()
            .into();
        Self { agent, last: None }
    }

    /// Where the recording with this ISRC is sold. Waits as needed to keep
    /// to one request a second.
    pub fn sold_at(&mut self, isrc: &str) -> Result<Vec<SoldAt>, PluginError> {
        let isrc = clean_isrc(isrc);
        if isrc.len() != 12 {
            return Ok(Vec::new());
        }
        if let Some(last) = self.last {
            let wait = Duration::from_millis(1100).saturating_sub(last.elapsed());
            std::thread::sleep(wait);
        }
        self.last = Some(Instant::now());
        let url = format!("https://musicbrainz.org/ws/2/isrc/{isrc}?inc=url-rels&fmt=json");
        let answer: Value = match self.agent.get(&url).call() {
            Ok(mut r) => r
                .body_mut()
                .read_json()
                .map_err(|e| PluginError::Format(format!("MusicBrainz: {e}")))?,
            // An ISRC MusicBrainz doesn't know.
            Err(ureq::Error::StatusCode(404)) => return Ok(Vec::new()),
            Err(e) => {
                eprintln!("musicbrainz: {e}");
                return Err(PluginError::Network("MusicBrainz didn't answer.".into()));
            }
        };
        Ok(parse(&answer))
    }
}

impl Default for MusicBrainz {
    fn default() -> Self {
        Self::new()
    }
}

/// An ISRC as MusicBrainz takes it: letters and digits, upper case.
pub fn clean_isrc(isrc: &str) -> String {
    isrc.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// The purchase pages in an ISRC lookup's answer, each once.
pub fn parse(answer: &Value) -> Vec<SoldAt> {
    let mut out: Vec<SoldAt> = Vec::new();
    let recordings = answer["recordings"].as_array().cloned().unwrap_or_default();
    for rel in recordings
        .iter()
        .flat_map(|r| r["relations"].as_array().cloned().unwrap_or_default())
    {
        if rel["type"].as_str() != Some("purchase for download") {
            continue;
        }
        let Some(url) = rel["url"]["resource"].as_str().filter(|u| is_web_url(u)) else {
            continue;
        };
        let Some(store) = links::store_of(url) else {
            continue;
        };
        if !out.iter().any(|s| s.url == url) {
            out.push(SoldAt {
                store,
                url: url.to_string(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn finds_purchase_pages() {
        let answer = json!({"isrc": "GBCFB1600016", "recordings": [
            {"id": "a", "title": "Kerala", "length": 245000, "relations": [
                {"type": "purchase for download", "target-type": "url",
                 "url": {"resource": "https://bonobomusic.bandcamp.com/track/kerala"}},
                {"type": "free streaming", "target-type": "url",
                 "url": {"resource": "https://open.spotify.com/track/1"}},
                {"type": "purchase for download", "target-type": "url",
                 "url": {"resource": "https://www.beatport.com/track/kerala/8724501"}}
            ]},
            {"id": "b", "title": "Kerala", "relations": [
                {"type": "purchase for download", "target-type": "url",
                 "url": {"resource": "https://bonobomusic.bandcamp.com/track/kerala"}}
            ]}
        ]});
        let found = parse(&answer);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].store, "bandcamp");
        assert_eq!(found[1].store, "beatport");
    }

    #[test]
    fn nothing_for_an_odd_answer() {
        assert!(parse(&json!({"error": "Not Found"})).is_empty());
    }

    #[test]
    fn cleans_isrcs() {
        assert_eq!(clean_isrc("gb-cfb-16-00016"), "GBCFB1600016");
    }
}
