# Proposal

Planned for: **v0.6 · Buy it from the artist**

Status: proposed, waiting for a direction (store links only, or also a Beatport partner cart; affiliate links on or off).

## Why

The Wanted list holds tracks the DJ means to own. Getting from there to a file in the Inbox stage takes many manual steps per track: find the store, find the track, buy, download, unzip, import. Selecting several Wanted tracks should open them for purchase in as few checkouts as the stores allow, and the bought files should arrive in Inbox on their own.

## What Changes

- **Buy… on Wanted rows** (one or many): a Buy sheet lists the best store match per track, artist-direct first, grouped by store.
- **Open by store**: track pages open in the browser (or the Music app for Apple) in small batches, so the store's own cart collects them for one checkout. openDJ never takes payment.
- **Track links, not just searches**, where a store id can be found: Bandcamp search, the public iTunes Search API, cross-links by ISRC (MusicBrainz, Odesli). Other stores fall back to their search page.
- **Pending purchases**: the Wanted rows sent to a store are remembered for a while so arriving files can be matched back to them.
- **More inbox folders**: besides Downloads, the inbox can watch store-app download folders (Music app, Qobuz, Amazon), taking only new files.
- **Match on arrival**: ISRC, then store tags, then artist + title + length; a match files the track into Inbox and ticks off its Wanted row. Unmatched files still land in Inbox with "Match to Wanted…".
- Optional, pending a decision: Beatport partner cart ("Add N to cart", then Checkout on beatport.com); affiliate tags on store links behind a Settings switch.

Supersedes the Wanted part of `add-find-and-buy`; builds on the purchase inbox in `enable-stores-and-purchase-inbox`. Bandcamp account downloads stay in `add-bandcamp-collection-sync`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library-sources`: Buy from Wanted, pending purchases, more inbox folders, match on arrival

## Out of Scope

- Taking payment or storing payment details in the app.
- Automatic purchasing without the DJ checking out on the store.
- DRM-protected downloads or any DRM workaround.
- Store sign-in scraping beyond what an add-on's own change covers.

## Impact

`crates/plugins` (links.rs track links, a cross-link lookup, inbox.rs multiple folders and matching), `crates/app` (browser/wanted.rs Buy action and sheet, inbox.rs, settings), library schema (pending purchase rows on `wanted`).
