# Tasks

## 1. Find

- [ ] 1.1 A Find source that sends one query to every download source and merges their snapshots, keeping each row's source; tests with two stub sources
- [ ] 1.2 Find in the rail above the add-on sources, with the search bar and cog; screenshot
- [ ] 1.3 Any and per-source chips under the search bar, kept for the session; test that a chip narrows the sources asked

## 2. Ranking

- [ ] 2.1 Quality tiers read from `format` and `quality` ("FLAC", "16/44.1", "320 kbps", "~128 kbps"); tests
- [ ] 2.2 Rank by tier, then artist/title/length match, then source order; tests
- [ ] 2.3 Source chip on each row; Preview and Download through the row's source; a combined Downloads row under Find

## 3. Source order

- [ ] 3.1 Download sources in Settings › Sources in a draggable order, saved; test

## 4. Wanted

- [ ] 4.1 Get on Wanted rows searches Find and downloads the best result that meets Prefer; tests with stub sources
- [ ] 4.2 Wanted's auto-download uses Find; a row nothing meets says so and stays
- [ ] 4.3 Add to Downloads on Spotify, TIDAL and Apple Music rows and playlists goes through Find, not one source

## 5. Lossy

- [ ] 5.1 Lossy flag on downloads below 256 kbps or from a source that says lossy; Health lists them; test
- [ ] 5.2 Guide page in website/book/src/guide
