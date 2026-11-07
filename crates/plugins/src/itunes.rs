//! The iTunes Store: its public Search API (no key) finds the track, and
//! its page opens in the Music app, where the DJ buys it. Bought tracks
//! are DRM-free AAC and download into the Music app's media folder.
//!
//! API: <https://performance-partners.apple.com/search-api>. Answers are
//! `{"resultCount": n, "results": [...]}`, each song with `artistName`,
//! `trackName`, `collectionName`, `trackTimeMillis`, `trackViewUrl` and
//! `artworkUrl100`, and `trackPrice` with `currency` (the price is -1 or
//! missing when the track is sold only with its album). Links carry no
//! referral tags.

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
                price: price(r["trackPrice"].as_f64(), r["currency"].as_str()),
            })
        })
        .collect())
}

/// A price to show: "$1.29", "€0.99", "150 JPY"; empty when there's no
/// price (the track is sold only with its album, or it's free to stream).
pub fn price(amount: Option<f64>, currency: Option<&str>) -> String {
    let Some(amount) = amount.filter(|a| *a > 0.0) else {
        return String::new();
    };
    let currency = currency.unwrap_or("").trim().to_ascii_uppercase();
    let symbol = match currency.as_str() {
        "USD" => "$",
        "EUR" => "€",
        "GBP" => "£",
        _ => "",
    };
    // Yen and won have no cents.
    let number = if amount.fract() == 0.0 && matches!(currency.as_str(), "JPY" | "KRW") {
        format!("{amount:.0}")
    } else {
        format!("{amount:.2}")
    };
    match (symbol, currency.as_str()) {
        ("", "") => number,
        ("", c) => format!("{number} {c}"),
        (s, _) => format!("{s}{number}"),
    }
}

/// The page as a link the Music app opens in its iTunes Store (macOS),
/// where the track can be bought; the web page shows Apple Music and needs
/// another click to get there. None for a link that isn't an iTunes Store
/// album or track page.
///
/// `https://music.apple.com/us/album/kerala/1170458452?i=1170458454` →
/// `itms://itunes.apple.com/us/album/id1170458452?i=1170458454`
pub fn store_app_link(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (host, rest) = rest.split_once('/')?;
    if !matches!(host, "music.apple.com" | "itunes.apple.com") {
        return None;
    }
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    let country = parts
        .first()
        .filter(|c| c.len() == 2 && c.chars().all(|c| c.is_ascii_alphabetic()))?;
    if parts.get(1) != Some(&"album") {
        return None;
    }
    let album = parts.last()?;
    let album = album.strip_prefix("id").unwrap_or(album);
    if album.is_empty() || !album.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let track = query
        .split('&')
        .find_map(|p| p.strip_prefix("i="))
        .filter(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()));
    Some(match track {
        Some(t) => format!("itms://itunes.apple.com/{country}/album/id{album}?i={t}"),
        None => format!("itms://itunes.apple.com/{country}/album/id{album}"),
    })
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
             "trackTimeMillis": 245_000, "trackPrice": 1.29, "currency": "USD",
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
        assert_eq!(k.price, "$1.29");
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
    fn opens_in_the_itunes_store_app() {
        assert_eq!(
            store_app_link("https://music.apple.com/us/album/kerala/1170458452?i=1170458454")
                .as_deref(),
            Some("itms://itunes.apple.com/us/album/id1170458452?i=1170458454")
        );
        assert_eq!(
            store_app_link("https://itunes.apple.com/gb/album/id42").as_deref(),
            Some("itms://itunes.apple.com/gb/album/id42")
        );
        assert_eq!(
            store_app_link("https://music.apple.com/us/artist/bonobo/1"),
            None
        );
        assert_eq!(store_app_link("https://satorio.bandcamp.com/track/x"), None);
    }

    #[test]
    fn prices_read_well() {
        assert_eq!(price(Some(0.99), Some("EUR")), "€0.99");
        assert_eq!(price(Some(1.29), Some("gbp")), "£1.29");
        assert_eq!(price(Some(250.0), Some("JPY")), "250 JPY");
        assert_eq!(price(Some(1.69), Some("CAD")), "1.69 CAD");
        // Album only, or nothing said.
        assert_eq!(price(Some(-1.0), Some("USD")), "");
        assert_eq!(price(None, Some("USD")), "");
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
