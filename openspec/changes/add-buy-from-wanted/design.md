# Design

Research note: `/mnt/project-files/notes/stores-buy-wanted-review.md` (project files, not in the repo).

## Store capabilities found

| Store | Track link | Find from app | Multi-track cart | Affiliate |
|---|---|---|---|---|
| Bandcamp | yes | unofficial search (in code) | browser cart across artists, one checkout; no add-to-cart link | none |
| Beatport / Beatsource | yes | partner API only | server cart; partner apps can add to it, checkout on the site | none public |
| Juno Download | yes | search link | browser cart | 5-10 %, `?ref=` |
| Traxsource | yes | search link | browser cart | none found |
| Apple (iTunes Store) | yes | public iTunes Search API | none, per track | to check |
| Qobuz | yes | search link | browser cart | to check |
| Amazon MP3 | yes | search link | basket | to check |

No store offers a public link or API that fills a cart with several tracks. Opening track pages for the DJ to add to the store's own cart gives one checkout per store.

## Decisions

- **Batches of up to 10 pages** per click, so stores' bot checks are not tripped and the browser isn't flooded.
- **Pending purchase** = `wanted.pending_store` + `wanted.pending_since`; cleared on match or after 7 days.
- **Matching order**: ISRC, store comment/tag (Bandcamp writes its URL into the comment), artist + title (normalised) + length within 2 s. Only pending rows are matched automatically; others need "Match to Wanted…".
- **Extra inbox folders** take only files newer than when the folder was added and never move files out of a store app's folder (copy mode), since that folder is the store app's library.
- **Cross-links**: MusicBrainz by ISRC first (open data); Odesli as a fallback with a short cache.

## Open questions

- Apply to Beatport for partner API access?
- Affiliate tags: off, or on with a Settings switch and a note?
- Store order after artist-direct.
