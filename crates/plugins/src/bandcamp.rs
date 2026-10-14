//! Bandcamp: search results in the app, and the purchase page in the browser.
//!
//! Bandcamp has no public catalogue API. Results come from the JSON
//! endpoint behind the search box on bandcamp.com, which isn't a published
//! API and may change; when it fails, the DJ is offered Bandcamp's own
//! search page instead.

use std::time::Duration;

use serde_json::{json, Value};

use crate::{
    encode, is_web_url, InboxFile, InboxHint, ItemKind, Manifest, Plugin, PluginError, StoreItem,
    StoreQuery, StoreSearch, USER_AGENT,
};

const MANIFEST: Manifest = Manifest {
    id: "bandcamp",
    name: "Bandcamp",
    hosts: &["bandcamp.com"],
    downloads: false,
};

const SEARCH: &str = "https://bandcamp.com/api/bcsearch_public_api/1/autocomplete_elastic";

pub struct Bandcamp {
    agent: ureq::Agent,
}

impl Bandcamp {
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(8)))
            .user_agent(USER_AGENT)
            .build()
            .into();
        Self { agent }
    }
}

impl Default for Bandcamp {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for Bandcamp {
    fn manifest(&self) -> &Manifest {
        &MANIFEST
    }
    fn store(&mut self) -> Option<&mut dyn StoreSearch> {
        Some(self)
    }
    fn inbox(&self) -> Option<&dyn InboxHint> {
        Some(self)
    }
}

impl StoreSearch for Bandcamp {
    fn search(&mut self, query: &StoreQuery) -> Result<Vec<StoreItem>, PluginError> {
        if query.text.is_empty() {
            return Ok(Vec::new());
        }
        let body = json!({
            "search_text": query.text,
            "search_filter": "",
            "full_page": false,
            "fan_id": null,
        });
        let answer: Value = self
            .agent
            .post(SEARCH)
            .send_json(&body)
            .map_err(|e| {
                eprintln!("bandcamp search: {e}");
                PluginError::Network(
                    "Bandcamp didn't answer; its search page is linked below.".into(),
                )
            })?
            .body_mut()
            .read_json()
            .map_err(|e| PluginError::Format(format!("Bandcamp: {e}")))?;
        parse(&answer)
    }

    fn search_url(&self, query: &StoreQuery) -> Option<String> {
        Some(format!(
            "https://bandcamp.com/search?q={}",
            encode(&query.text)
        ))
    }
}

/// Reads the search box's answer: `{"auto": {"results": [...]}}`, each
/// result with a `type` (t track, a album, b artist, l label), its `name`,
/// `band_name`, `album_name`, page URL and artwork.
pub fn parse(answer: &Value) -> Result<Vec<StoreItem>, PluginError> {
    let Some(results) = answer["auto"]["results"].as_array() else {
        return Err(PluginError::Format("Bandcamp: no results list".into()));
    };
    let text = |r: &Value, key: &str| r[key].as_str().unwrap_or_default().trim().to_string();
    let mut items: Vec<StoreItem> = results
        .iter()
        .filter_map(|r| {
            let kind = match r["type"].as_str()? {
                "t" => ItemKind::Track,
                "a" => ItemKind::Release,
                "b" => ItemKind::Artist,
                "l" => ItemKind::Label,
                _ => return None,
            };
            let url = ["item_url_path", "item_url_root", "url"]
                .iter()
                .filter_map(|k| r[*k].as_str())
                .find(|u| is_web_url(u))?
                .to_string();
            let name = text(r, "name");
            let (artist, title) = match kind {
                ItemKind::Artist | ItemKind::Label => (name, String::new()),
                _ => (text(r, "band_name"), name),
            };
            Some(StoreItem {
                kind,
                store: MANIFEST.name.into(),
                artist,
                title,
                release: match kind {
                    ItemKind::Track => text(r, "album_name"),
                    ItemKind::Release => "Album".into(),
                    ItemKind::Artist => "Artist".into(),
                    _ => "Label".into(),
                },
                duration: 0.0,
                artwork_url: r["img"]
                    .as_str()
                    .filter(|u| is_web_url(u))
                    .map(str::to_string),
                page_url: Some(url),
                download: None,
            })
        })
        .collect();
    // Tracks first, then releases, artists and labels, keeping Bandcamp's
    // order within each.
    items.sort_by_key(|i| match i.kind {
        ItemKind::Track => 0,
        ItemKind::Release => 1,
        ItemKind::Artist => 2,
        _ => 3,
    });
    Ok(items)
}

impl InboxHint for Bandcamp {
    /// Bandcamp writes "Visit https://<artist>.bandcamp.com" into the
    /// comment tag of everything it sells, and names files
    /// "Artist - Album - 01 Title" inside "Artist - Album.zip".
    fn recognise(&self, file: &InboxFile) -> f32 {
        if file.comment.to_lowercase().contains("bandcamp.com") {
            return 0.95;
        }
        let name = file.file_name();
        let numbered = name.split(" - ").nth(2).is_some_and(|rest| {
            rest.len() > 3 && rest.as_bytes()[..2].iter().all(u8::is_ascii_digit)
        });
        if file.archive.is_some() && numbered {
            0.6
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        json!({"auto": {"results": [
            {"type": "b", "name": "Bonobo", "item_url_root": "https://bonobomusic.bandcamp.com",
             "img": "https://f4.bcbits.com/img/1.jpg"},
            {"type": "t", "name": "Kerala", "band_name": "Bonobo", "album_name": "Migration",
             "item_url_path": "https://bonobomusic.bandcamp.com/track/kerala",
             "img": "https://f4.bcbits.com/img/a2.jpg"},
            {"type": "a", "name": "Migration", "band_name": "Bonobo",
             "item_url_path": "https://bonobomusic.bandcamp.com/album/migration"},
            {"type": "t", "name": "Bad link", "band_name": "X", "item_url_path": "javascript:x"},
            {"type": "f", "name": "A fan"}
        ]}})
    }

    #[test]
    fn parses_results_tracks_first() {
        let items = parse(&sample()).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].kind, ItemKind::Track);
        assert_eq!(items[0].artist, "Bonobo");
        assert_eq!(items[0].title, "Kerala");
        assert_eq!(items[0].release, "Migration");
        assert_eq!(
            items[0].page_url.as_deref(),
            Some("https://bonobomusic.bandcamp.com/track/kerala")
        );
        assert_eq!(items[1].kind, ItemKind::Release);
        assert_eq!(items[2].kind, ItemKind::Artist);
        assert_eq!(items[2].artist, "Bonobo");
    }

    #[test]
    fn a_changed_answer_is_a_format_error() {
        assert!(matches!(
            parse(&json!({"results": []})),
            Err(PluginError::Format(_))
        ));
    }

    #[test]
    fn search_page_fallback() {
        let b = Bandcamp::new();
        assert_eq!(
            b.search_url(&StoreQuery::text("bonobo kerala")).unwrap(),
            "https://bandcamp.com/search?q=bonobo+kerala"
        );
    }

    #[test]
    fn recognises_bandcamp_files() {
        let b = Bandcamp::new();
        let tagged = InboxFile {
            path: "/in/Kerala.flac".into(),
            comment: "Visit https://bonobomusic.bandcamp.com".into(),
            ..Default::default()
        };
        assert!(b.recognise(&tagged) > 0.9);
        let named = InboxFile {
            path: "/in/Bonobo - Migration - 03 Kerala.flac".into(),
            archive: Some("Bonobo - Migration.zip".into()),
            ..Default::default()
        };
        assert!(b.recognise(&named) >= 0.5);
        let other = InboxFile {
            path: "/in/Kerala.mp3".into(),
            ..Default::default()
        };
        assert_eq!(b.recognise(&other), 0.0);
    }
}
