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

- **One page per click** (Open, then Next: 2 of N), so the DJ buys a track at a time instead of facing a pile of tabs; clicking a track opens just that one. (Was batches of 10; changed 2026-11-06 after use: too many tabs.)
- **Pending purchase** = `wanted.pending_store` + `wanted.pending_since`; cleared on match or after 7 days.
- **Matching order**: store plugins recognise the store (Bandcamp's comment tag); the Wanted row is found by ISRC, then artist + title (normalised) + length within 2 s, rows being bought first.
- **The Music app's folder** (macOS, `~/Music/Music/Media.localized/Music`) is watched two folders deep, looking only into folders changed since the watch began; only files that match a Wanted track being bought are taken, and they're added where they are (no copy: disk space), since that folder is the Music app's library. A general "add a folder" setting for other store apps (Qobuz, Amazon) is a follow-up.
- **The purchase inbox is off by default** and switched on from the Buy sheet; switching on resets the time from which files count.
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
