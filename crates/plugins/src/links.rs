//! Stores without an open search API: the app offers their own search
//! page, filled in with what the DJ searched for.

use crate::{encode, Manifest, Plugin, PluginError, StoreItem, StoreQuery, StoreSearch};

pub struct LinkStoreInfo {
    pub manifest: Manifest,
    /// The search page, with `{q}` where the search goes.
    pub template: &'static str,
}

pub const STORES: [LinkStoreInfo; 8] = [
    LinkStoreInfo {
        manifest: Manifest {
            id: "beatport",
            name: "Beatport",
            hosts: &[],
            downloads: false,
        },
        template: "https://www.beatport.com/search?q={q}",
    },
    LinkStoreInfo {
        manifest: Manifest {
            id: "juno",
            name: "Juno Download",
            hosts: &[],
            downloads: false,
        },
        template: "https://www.junodownload.com/search/?q%5Ball%5D%5B%5D={q}",
    },
    LinkStoreInfo {
        manifest: Manifest {
            id: "traxsource",
            name: "Traxsource",
            hosts: &[],
            downloads: false,
        },
        template: "https://www.traxsource.com/search?term={q}",
    },
    LinkStoreInfo {
        manifest: Manifest {
            id: "qobuz",
            name: "Qobuz",
            hosts: &[],
            downloads: false,
        },
        template: "https://www.qobuz.com/us-en/search?q={q}",
    },
    LinkStoreInfo {
        manifest: Manifest {
            id: "bleep",
            name: "Bleep",
            hosts: &[],
            downloads: false,
        },
        template: "https://bleep.com/search/query?q={q}",
    },
    LinkStoreInfo {
        manifest: Manifest {
            id: "boomkat",
            name: "Boomkat",
            hosts: &[],
            downloads: false,
        },
        template: "https://boomkat.com/products?q%5Bkeywords%5D={q}",
    },
    LinkStoreInfo {
        manifest: Manifest {
            id: "mirlo",
            name: "Mirlo",
            hosts: &[],
            downloads: false,
        },
        template: "https://mirlo.space/search?search={q}",
    },
    LinkStoreInfo {
        manifest: Manifest {
            id: "amazon",
            name: "Amazon Music",
            hosts: &[],
            downloads: false,
        },
        template: "https://www.amazon.com/s?k={q}&i=digital-music",
    },
];

/// The store a page belongs to, by its host: the id of a built-in store
/// (or "artist" for none we know, which is likely the artist's own site).
pub fn store_of(url: &str) -> Option<&'static str> {
    if !crate::is_web_url(url) {
        return None;
    }
    let host = url
        .split("://")
        .nth(1)?
        .split(['/', '?', '#'])
        .next()?
        .rsplit('@')
        .next()?
        .to_ascii_lowercase();
    let on = |domain: &str| host == domain || host.ends_with(&format!(".{domain}"));
    let known = [
        ("bandcamp.com", "bandcamp"),
        ("beatport.com", "beatport"),
        ("junodownload.com", "juno"),
        ("traxsource.com", "traxsource"),
        ("qobuz.com", "qobuz"),
        ("bleep.com", "bleep"),
        ("boomkat.com", "boomkat"),
        ("mirlo.space", "mirlo"),
        ("music.apple.com", "itunes"),
        ("itunes.apple.com", "itunes"),
    ];
    if let Some((_, id)) = known.iter().find(|(d, _)| on(d)) {
        return Some(id);
    }
    if host.starts_with("amazon.") || host.contains(".amazon.") {
        return Some("amazon");
    }
    // Streaming services and catalogues sell nothing themselves.
    let not_shops = [
        "spotify.com",
        "tidal.com",
        "soundcloud.com",
        "youtube.com",
        "deezer.com",
        "discogs.com",
        "wikipedia.org",
        "wikidata.org",
        "musicbrainz.org",
    ];
    if not_shops.iter().any(|d| on(d)) {
        return None;
    }
    Some("artist")
}

pub struct LinkStore(pub &'static LinkStoreInfo);

impl Plugin for LinkStore {
    fn manifest(&self) -> &Manifest {
        &self.0.manifest
    }
    fn store(&mut self) -> Option<&mut dyn StoreSearch> {
        Some(self)
    }
}

impl StoreSearch for LinkStore {
    fn search(&mut self, _query: &StoreQuery) -> Result<Vec<StoreItem>, PluginError> {
        Err(PluginError::NotSupported)
    }

    fn search_url(&self, query: &StoreQuery) -> Option<String> {
        Some(self.0.template.replace("{q}", &encode(&query.text)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_in_the_search() {
        let mut s = LinkStore(&STORES[0]);
        let q = StoreQuery::track("Bicep", "Glue");
        assert_eq!(
            s.search_url(&q).unwrap(),
            "https://www.beatport.com/search?q=Bicep+Glue"
        );
        assert_eq!(s.search(&q), Err(PluginError::NotSupported));
    }

    #[test]
    fn knows_stores_by_their_pages() {
        assert_eq!(store_of("https://a.bandcamp.com/track/b"), Some("bandcamp"));
        assert_eq!(
            store_of("https://www.beatport.com/track/x/1"),
            Some("beatport")
        );
        assert_eq!(
            store_of("https://music.apple.com/us/album/x/1?i=2"),
            Some("itunes")
        );
        assert_eq!(store_of("https://www.amazon.co.uk/dp/B01"), Some("amazon"));
        assert_eq!(store_of("https://open.spotify.com/track/1"), None);
        assert_eq!(store_of("https://fakebandcamp.com/x"), Some("artist"));
        assert_eq!(store_of("https://someartist.net/shop"), Some("artist"));
        assert_eq!(store_of("file:///etc/passwd"), None);
    }
}
