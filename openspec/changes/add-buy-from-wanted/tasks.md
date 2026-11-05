# Tasks

## 1. Links

- [ ] 1.1 Track links per store with search fallback; tests for each URL shape
- [ ] 1.2 Cross-link lookup by ISRC (MusicBrainz, then Odesli) with recorded responses
- [ ] 1.3 iTunes Search API lookup with recorded responses

## 2. Buy from Wanted

- [ ] 2.1 Buy… action and Buy sheet on Wanted rows, grouped by store, batches of 10
- [ ] 2.2 Pending purchase columns on `wanted` (schema bump) with expiry test

## 3. Arrival

- [ ] 3.1 Inbox watches several folders; store-app folders copy, new files only
- [ ] 3.2 Match arrivals to pending Wanted rows (ISRC, tags, artist/title/length) with tests
- [ ] 3.3 Match to Wanted… on unmatched Inbox tracks

## 4. Ranking

- [ ] 4.1 Search all checkable stores at once; rank by has-it, artist-direct, speed to library, lossless; tests
- [ ] 4.2 Search links for Bleep, Boomkat, Qobuz, Amazon; Mirlo via `add-artist-direct-stores`

## 5. Beatport (only if partner access is granted)

- [ ] 5.1 Beatport sign-in (OAuth PKCE, loopback callback), catalogue lookup by ISRC
- [ ] 5.2 Add selected tracks to the Beatport cart, open checkout
- [ ] 5.3 Pull purchased tracks from the Beatport library into the inbox

## 6. Docs

- [ ] 6.1 Update the matching user guide page in website/book/src/guide

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.
