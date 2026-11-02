# Tasks

## 1. Now Playing feed

- [ ] 1.1 Audible-deck rule on the UI thread from the meter values (post-fader, post-crossfader level; hold time; nothing playing), with tests for blends, cuts and headphone-only decks
- [ ] 1.2 `now-playing.txt` (template) and `now-playing.json` (current + last ten) written on a worker thread, write-then-rename, only on change
- [ ] 1.3 `crates/remote`: read-only `/now-playing`, `/now-playing.json` and `/now-playing/ws` routes without pairing, bound to 127.0.0.1 unless network access is allowed; cover served from the cover cache
- [ ] 1.4 Overlay page in three styles (lower third, corner card, ticker), clear background, fade between tracks
- [ ] 1.5 Settings › Mixer: Now Playing switch, folder, template, hold time, style, copy-address button, network tick
- [ ] 1.6 Update the user guide (streaming overlays: OBS Text source and Browser Source)

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.
