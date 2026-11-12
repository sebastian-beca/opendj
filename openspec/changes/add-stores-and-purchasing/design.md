# Design

Research: `/mnt/project-files/notes/stores-buy-wanted-review.md` (project files), and the design of `add-buy-from-wanted`, whose ranking and matching this change keeps as they are.

## Ground rules

- openDJ never takes payment; every purchase uses the store's own checkout.
- No affiliate or referral tags, so the store's full share goes to the label and artist.
- No fixed store order. Per track: stores that have it, then artist-direct, then fastest to the library, then lossless. A pinned store rises only among stores that have the track.
- Nothing in Downloads or a store-app folder moves until the DJ turns the inbox on; only files newer than that moment count.

## Order of work

Each step is its own PR and useful alone.

| Step | Depends on | Why this order |
|---|---|---|
| 1 Stores on | nothing | Built code goes live first; checks the unofficial Bandcamp search on a real network |
| 2 Buy… everywhere | 1 | Reuses the Buy sheet; biggest reach for the least code |
| 3 Track links (Odesli, prices) | 2 | Better matches for tracks found from streaming ids |
| 4 Arrival (Match to Wanted…, folders, Just bought) | 1 | Closes the loop for purchases from any store |
| 5 Bandcamp sync | 4 | Lands in Just bought; the fastest path to the library for the main artist-direct store |
| 6 More artist-direct stores | 2 | Each one an add-on checked against its terms |
| 7 Beatport | partner access | Built only if Beatport grants it |

## Store sources

- `STORES_ENABLED` becomes a per-store switch in Settings › Sources (Bandcamp, Beatport, Shopping), all hidden on a fresh install as now, so a DJ who never buys sees nothing new.
- Beatport without partner access shows search links only ("Search on Beatport"), never scraped results.

## Buy… on any track without a file

- The action appears on rows that have a streaming id, ISRC or artist and title but no collection match. It builds the same Buy sheet as Wanted.
- Starting a purchase from outside Wanted adds the track to Wanted as being bought (pending store and time). If nothing arrives within the 7-day pending window, the row stays on Wanted, so the DJ can see what was not finished. A wanted-only "Remove when it arrives" is the existing behaviour.
- Lookups per track: ISRC from the streaming service when known, then MusicBrainz relations, then Odesli, then the iTunes Search API and Bandcamp search. Results are cached per track for a day.

## Arrival

- **Just bought** is a list inside the Inbox stage, not a new stage: tracks whose arrival came through the purchase inbox or Bandcamp sync (a `purchase_store` on the Inbox row). Moving a track on from Inbox removes it from Just bought.
- **Match to Wanted…** on an unmatched Inbox track lists Wanted rows by artist and title similarity; picking one ticks it off and copies its ISRC and store into the track.
- **Other folders**: Settings › Library › Purchase inbox gets a folder list (+/-), each with Take (move, copy or leave in place). The Music app folder keeps its rule: only pending purchases, left in place.

## Bandcamp sync

- Bandcamp has no public API for a fan's collection. Sync uses the same endpoints the Bandcamp site uses for a signed-in fan: the collection list and each item's download page. The session cookie comes from a sign-in window (embedded web view or the system browser with a pasted cookie as a fallback) and is kept in the system keychain.
- It runs on demand ("Sync now") and optionally once a day; downloads one at a time, spaced, to stay polite. Each purchase is downloaded once (Bandcamp item id stored), unpacked by the inbox and tagged with the Bandcamp comment that the inbox already recognises.
- Built into openDJ beside the Bandcamp search (same plugin), since the purchase inbox is core and both use the same unofficial endpoints.

## More artist-direct stores

- Mirlo has an open API; Faircamp sites are static and publish feeds; Ampwall to be checked. Each is checked for terms before it is built, as an add-on with a `StoreSearch` that marks its results artist-direct.

## Beatport

- Partner API v4: OAuth PKCE with a loopback callback, catalogue lookup by ISRC, cart add (up to 100), checkout opens beatport.com, and library downloads into the inbox. Built only once access is granted; until then step 7 stays open.

## Open questions

- Bandcamp sync format default: FLAC (smaller lossless, tags) or the DJ's choice of FLAC, AIFF or WAV? Proposed: FLAC, with AIFF as an option for players that need it.
- Bandcamp sync built in beside Bandcamp search (proposed, since both use the same endpoints and the inbox is core) or as a separate add-on as first planned?
- Should Buy… from a streaming list add the track to Wanted visibly, or keep a hidden pending row? Proposed: visibly, so unfinished purchases are not lost.
