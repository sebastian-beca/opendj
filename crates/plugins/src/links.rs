//! Stores without an open search API: the app offers their own search
//! page, filled in with what the DJ searched for.

use crate::{encode, Manifest, Plugin, PluginError, StoreItem, StoreQuery, StoreSearch};

pub struct LinkStoreInfo {
    pub manifest: Manifest,
    /// The search page, with `{q}` where the search goes.
    pub template: &'static str,
}

pub const STORES: [LinkStoreInfo; 4] = [
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
];

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
}
