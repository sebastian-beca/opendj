# Design

Research note: `/mnt/project-files/notes/stores-buy-wanted-review.md` (project files, not in the repo).

## Store capabilities found

| Store | Track link | Find from app | Multi-track cart | Strong on |
|---|---|---|---|---|
| Bandcamp | yes | unofficial search (in code) | browser cart across artists, one checkout; no add-to-cart link | independent, ambient, experimental |
| Beatport / Beatsource | yes | partner API only | server cart; partner apps can add to it, checkout on the site | club genres |
| Juno Download | yes | search link | browser cart | club, broad back catalogue |
| Traxsource | yes | search link | browser cart | house, soulful, deep |
| Apple (iTunes Store) | yes | public iTunes Search API | none, per track | broad mainstream + many indies |
| Qobuz | yes | search link | browser cart | ambient, jazz, classical, hi-res |
| Bleep | yes | search link | browser cart | electronic, ambient, Warp-type labels |
| Boomkat | yes | search link | browser cart | ambient, experimental, leftfield |
| Mirlo | yes | search (add-on) | per artist | artist-direct, independent |
| Amazon MP3 | yes | search link | basket | mainstream |

No store offers a public link or API that fills a cart with several tracks. Opening track pages for the DJ to add to the store's own cart gives one checkout per store.

## Decisions

- **Batches of up to 10 pages** per click, so stores' bot checks are not tripped and the browser isn't flooded.
- **Pending purchase** = `wanted.pending_store` + `wanted.pending_since`; cleared on match or after 7 days.
- **Matching order**: ISRC, store comment/tag (Bandcamp writes its URL into the comment), artist + title (normalised) + length within 2 s. Only pending rows are matched automatically; others need "Match to Wanted…".
- **Extra inbox folders** take only files newer than when the folder was added and never move files out of a store app's folder (copy mode), since that folder is the store app's library.
- **Cross-links**: MusicBrainz by ISRC first (open data); Odesli as a fallback with a short cache.

## Ranking

No fixed store order. For each track:

1. **Has it**: only stores where the track was found (ISRC, then artist + title + length) are offered first; stores that can only be searched follow as "Try on …".
2. **Artist-direct first** among those that have it (artist site, Bandcamp, Mirlo).
3. **Fastest to the library** next:
   - pulled automatically: Bandcamp account sync, Beatport library (with partner access), Apple Music app automatic downloads;
   - browser download picked up by the inbox: everyone else.
4. Ties: lossless formats first (FLAC/WAV/AIFF over MP3/AAC).

The DJ can pin a preferred store; pinned stores rise within step 2 or 3, never above a store that has the track when the pinned one doesn't.

## Decisions (2026-11-05)

- Beatport partner access: requested; the cart and library pull are built only if it is granted.
- No affiliate or referral tags, so the full price goes to the store and the artist's share isn't reduced by a referral fee.
- Coverage and speed rank stores, not a fixed list.
