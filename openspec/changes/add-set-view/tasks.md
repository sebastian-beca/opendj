# Tasks

## 1. Automix points from a set

- [ ] 1.1 Set-log reader and timeline model: clips, lanes, transitions, style found (unit tests on recorded logs)
- [ ] 1.2 `pair_mixes` table and Automix plan lookup: pair mix, then track points, then phrases
- [ ] 1.3 "Learn Automix points" screen from a recording: transitions with out/in points, existing points beside them, save ticks
- [ ] 1.4 "Play it again as Automix" from a recording or History set

## 2. Capture

- [ ] 2.1 Control changes stamped with their audio frame through a ring from the audio thread
- [ ] 2.2 Plugin parameter changes and instrument / Groove Box notes in the log
- [ ] 2.3 Setting to record Mic and Aux to their own files beside the recording
- [ ] 2.4 Deck place every audio block while a deck is scratched, backspun or braked

## 3. Set view

- [ ] 3.1 Open in Set view from Mixed Sets and History; deck rows, transitions, lanes, overview, zoom
- [ ] 3.2 Playback of the recording with a playhead; click to jump
- [ ] 3.3 Things to check: FX left on past a phrase change, jumps mid-phrase, limiter gain reduction over 3 dB

## 4. Edit and render

- [ ] 4.1 Move and stretch transitions with beat and phrase snapping; styles; undo
- [ ] 4.2 Transition editor: both tracks on one beatgrid, editable curves, as-played ghost, A/B
- [ ] 4.3 Offline render through the engine to a new file; null test against an unedited recording
- [ ] 4.4 Loops as brackets with a repeat count; drag to change length or count, or remove
- [ ] 4.5 Hard moves as one block with Keep from recording (copied audio, the default) or Bypass
- [ ] 4.6 Mixcloud export and tracklist from the rendered file

## 5. Docs

- [ ] 5.1 User guide page for Set view and learned Automix points in website/book/src/guide

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.
