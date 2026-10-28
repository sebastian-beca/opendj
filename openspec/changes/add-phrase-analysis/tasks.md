# Tasks

## 1. Phrases

- [x] 1.1 Per-bar band levels from the analysis waveform (dynamic grids included)
- [ ] 1.1b Chroma novelty, for changes at the same loudness
- [x] 1.2 Dynamic-programming segmentation with an 8/16-bar length prior (8-32 bars); labels; cuts snap to bass entries; outro only after the last strong section
- [ ] 1.2b Energy 1-10 per phrase
- [ ] 1.3 Unit test on `synth.rs` tracks; benchmark against rekordbox phrases and VDJ automix points on the owner's Mac
- [ ] 1.4 Store in `markers` (kind `phrase`) and keep imported phrases (detected phrases are found again from the grid and waveform whenever a track loads or its grid changes)
- [x] 1.5 Phrase bar under the minute dots on the deck overviews, with a right-click Show/Hide phrases; screenshot
- [x] 1.5a Hover tooltip on the overview names the phrase, its bars and times
- [ ] 1.5b Deck countdown ("Drop · 8 bars left")
- [ ] 1.6 `has:intro>=16` search

## 2. Automatic cues

- [ ] 2.1 Settings › Analysis Auto cues (Off / Empty pads only / Replace auto cues), cues with source `auto`
- [ ] 2.2 Tests: DJ and imported cues never change

## 3. Docs

- [ ] 3.1 Update the matching user guide pages in website/book/src/guide

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.
