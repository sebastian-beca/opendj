//! The iTunes Store: its public Search API (no key) finds the track, and
//! its page opens in the Music app, where the DJ buys it. Bought tracks
//! are DRM-free AAC and download into the Music app's media folder.
//!
//! API: <https://performance-partners.apple.com/search-api>. Answers are
//! `{"resultCount": n, "results": [...]}`, each song with `artistName`,
//! `trackName`, `collectionName`, `trackTimeMillis`, `trackViewUrl` and
//! `artworkUrl100`. Links carry no referral tags.

use std::time::Duration;

use serde_json::Value;

use crate::{
    encode, is_web_url, ItemKind, Manifest, Plugin, PluginError, StoreItem, StoreQuery,
    StoreSearch, USER_AGENT,
};

const MANIFEST: Manifest = Manifest {
    id: "itunes",
    name: "iTunes Store",
    hosts: &["itunes.apple.com"],
    downloads: false,
};

pub struct Itunes {
    agent: ureq::Agent,
    /// The storefront's two-letter country, which sets prices and what's
    /// for sale.
    country: String,
}

impl Itunes {
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(8)))
            .user_agent(USER_AGENT)
            .build()
            .into();
        Self {
            agent,
            country: country(),
        }
    }

    fn url(&self, query: &StoreQuery) -> String {
        format!(
            "https://itunes.apple.com/search?term={}&media=music&entity=song&limit=10&country={}",
            encode(&query.text),
            self.country
        )
    }
}

impl Default for Itunes {
    fn default() -> Self {
        Self::new()
    }
}

/// The country from the system locale ("en_GB.UTF-8" → "gb"); "us" when
/// it doesn't say.
fn country() -> String {
    let lang = std::env::var("LC_ALL")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("LANG").ok())
        .unwrap_or_default();
    country_of(&lang)
}

fn country_of(locale: &str) -> String {
    let region = locale
        .split(['.', '@'])
        .next()
        .and_then(|l| l.split(['_', '-']).nth(1))
        .unwrap_or("");
    if region.len() == 2 && region.chars().all(|c| c.is_ascii_alphabetic()) {
        region.to_ascii_lowercase()
    } else {
        "us".into()
    }
}

impl Plugin for Itunes {
    fn manifest(&self) -> &Manifest {
        &MANIFEST
    }
    fn store(&mut self) -> Option<&mut dyn StoreSearch> {
        Some(self)
    }
}

impl StoreSearch for Itunes {
    fn search(&mut self, query: &StoreQuery) -> Result<Vec<StoreItem>, PluginError> {
        if query.text.is_empty() {
            return Ok(Vec::new());
        }
        let answer: Value = self
            .agent
            .get(&self.url(query))
            .call()
            .map_err(|e| {
                eprintln!("itunes search: {e}");
                PluginError::Network("The iTunes Store didn't answer.".into())
            })?
            .body_mut()
            .read_json()
            .map_err(|e| PluginError::Format(format!("iTunes: {e}")))?;
        parse(&answer)
    }

    fn search_url(&self, query: &StoreQuery) -> Option<String> {
        Some(format!(
            "https://music.apple.com/{}/search?term={}",
            self.country,
            encode(&query.text)
        ))
    }
}

/// Reads a Search API answer: its songs, as tracks.
pub fn parse(answer: &Value) -> Result<Vec<StoreItem>, PluginError> {
    let Some(results) = answer["results"].as_array() else {
        return Err(PluginError::Format("iTunes: no results list".into()));
    };
    let text = |r: &Value, key: &str| r[key].as_str().unwrap_or_default().trim().to_string();
    Ok(results
        .iter()
        .filter(|r| r["kind"].as_str() == Some("song"))
        .filter_map(|r| {
            let page = r["trackViewUrl"].as_str().filter(|u| is_web_url(u))?;
            Some(StoreItem {
                kind: ItemKind::Track,
                store: MANIFEST.name.into(),
                artist: text(r, "artistName"),
                title: text(r, "trackName"),
                release: text(r, "collectionName"),
                duration: r["trackTimeMillis"].as_f64().unwrap_or(0.0) / 1000.0,
                artwork_url: r["artworkUrl100"]
                    .as_str()
                    .filter(|u| is_web_url(u))
                    .map(str::to_string),
                page_url: Some(strip_tracking(page)),
                download: None,
            })
        })
        .collect())
}

/// The page without the `uo` and `at`/`ct` tracking or referral fields
/// the API adds, so a link carries nobody's referral.
fn strip_tracking(url: &str) -> String {
    let Some((base, query)) = url.split_once('?') else {
        return url.to_string();
    };
    let kept: Vec<&str> = query
        .split('&')
        .filter(|p| {
            let key = p.split('=').next().unwrap_or("");
            !matches!(key, "uo" | "at" | "ct" | "pt" | "mt" | "app")
        })
        .collect();
    if kept.is_empty() {
        base.to_string()
    } else {
        format!("{base}?{}", kept.join("&"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Value {
        json!({"resultCount": 3, "results": [
            {"wrapperType": "track", "kind": "song", "artistName": "Bonobo",
             "trackName": "Kerala", "collectionName": "Migration",
             "trackTimeMillis": 245_000,
             "trackViewUrl": "https://music.apple.com/us/album/kerala/1170458452?i=1170458454&uo=4",
             "artworkUrl100": "https://is1-ssl.mzstatic.com/image/thumb/a/100x100bb.jpg"},
            {"wrapperType": "track", "kind": "music-video", "artistName": "Bonobo",
             "trackName": "Kerala (Video)", "trackViewUrl": "https://music.apple.com/us/music-video/x/1"},
            {"wrapperType": "track", "kind": "song", "artistName": "X",
             "trackName": "Bad", "trackViewUrl": "javascript:alert(1)"}
        ]})
    }

    #[test]
    fn parses_songs_only() {
        let items = parse(&sample()).unwrap();
        assert_eq!(items.len(), 1);
        let k = &items[0];
        assert_eq!((k.artist.as_str(), k.title.as_str()), ("Bonobo", "Kerala"));
        assert_eq!(k.release, "Migration");
        assert_eq!(k.duration, 245.0);
        assert_eq!(
            k.page_url.as_deref(),
            Some("https://music.apple.com/us/album/kerala/1170458452?i=1170458454")
        );
    }

    #[test]
    fn a_changed_answer_is_a_format_error() {
        assert!(matches!(parse(&json!({})), Err(PluginError::Format(_))));
    }

    #[test]
    fn links_carry_no_referral() {
        assert_eq!(
            strip_tracking("https://music.apple.com/us/album/x/1?i=2&at=1000l&ct=y&uo=4"),
            "https://music.apple.com/us/album/x/1?i=2"
        );
        assert_eq!(
            strip_tracking("https://music.apple.com/us/album/x/1?uo=4"),
            "https://music.apple.com/us/album/x/1"
        );
    }

    #[test]
    fn storefront_from_the_locale() {
        assert_eq!(country_of("en_GB.UTF-8"), "gb");
        assert_eq!(country_of("de-DE"), "de");
        assert_eq!(country_of("C"), "us");
        assert_eq!(country_of(""), "us");
    }
}
