# Tasks

## 1. Now Playing feed

- [x] 1.1 Audible-deck rule on the UI thread from the meter values (post-fader, post-crossfader level; hold time; nothing playing), with tests for blends, cuts and headphone-only decks
- [x] 1.2 `now-playing.txt` (template) and `now-playing.json` (current + last ten) written on a worker thread, write-then-rename, only on change
- [x] 1.3 `crates/remote`: read-only `/now-playing`, `/now-playing.json` and `/now-playing/cover` routes without pairing, bound to 127.0.0.1 unless network access is allowed. Shipped without `/now-playing/ws`: the page asks for the JSON once a second, which OBS handles well and needs no push. The cover is the track's own art, encoded as JPEG.
- [x] 1.4 Overlay page in three styles (lower third, corner card, ticker), clear background, fade between tracks; a style change shows without reloading the page
- [x] 1.5 Settings › Broadcast (beside the radio title, not Mixer): Now Playing switch, overlay address with Open, style, folder, template, hold time, network switch
- [x] 1.6 Update the user guide (streaming overlays: OBS Text source and Browser Source)

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.
