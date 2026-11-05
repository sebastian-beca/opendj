# Tasks

## 1. Links

- [x] 1.1 Track links per store with search fallback; tests for each URL shape
- [x] 1.2 Cross-link lookup by ISRC (MusicBrainz) with sample answers
- [x] 1.3 iTunes Search API lookup with sample answers
- [ ] 1.4 Check the MusicBrainz, iTunes and Bandcamp lookups against the live services on a machine with network access
- [ ] 1.5 Odesli as a second cross-link source

## 2. Buy from Wanted

- [x] 2.1 Buy… action and Buy sheet on Wanted rows, grouped by store, batches of 10
- [x] 2.2 Pending purchase columns on `wanted` with expiry test

## 3. Arrival

- [x] 3.1 Inbox off by default, switched on from the Buy sheet; Music app folder watched two levels deep, pending purchases only, left in place
- [x] 3.2 Match arrivals to Wanted rows (ISRC, then artist/title/length, being bought first) with tests
- [ ] 3.3 Match to Wanted… on unmatched Inbox tracks
- [ ] 3.4 A setting to add other store-app download folders (Qobuz, Amazon)

## 4. Ranking

- [x] 4.1 Search all checkable stores at once; rank by has-it, artist-direct, speed to library, lossless; tests
- [x] 4.2 Search links for Bleep, Boomkat, Qobuz, Amazon and Mirlo

## 5. Beatport (only if partner access is granted)

- [ ] 5.1 Beatport sign-in (OAuth PKCE, loopback callback), catalogue lookup by ISRC
- [ ] 5.2 Add selected tracks to the Beatport cart, open checkout
- [ ] 5.3 Pull purchased tracks from the Beatport library into the inbox

## 6. Docs

- [x] 6.1 Update the matching user guide page in website/book/src/guide

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.
