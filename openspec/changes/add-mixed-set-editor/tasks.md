# Tasks

## 1. Reading a set

- [ ] 1.1 Control-log reader and timeline model: clips, lanes, transitions with style found, loops, hard moves (unit tests on recorded logs)
- [ ] 1.2 Fallback for recordings without a log: waveform and cue-sheet tracklist

## 2. Mixed Set Editor

- [ ] 2.1 Open in Mixed Set Editor from Recordings and from History sets that have a recording
- [ ] 2.2 Simple view by default; Show decks: deck rows with track blocks, transitions, loops and hard moves; folded lanes; whole-set overview; zoom
- [ ] 2.3 Playback of the recording with a playhead; click to jump
- [ ] 2.4 Transitions list with details of the selected one; click jumps to 4 bars before

## 3. Check my mix

- [ ] 3.1 Clipping, silence and level-jump checks from the recording
- [ ] 3.2 Drift, FX left on and mid-phrase jump checks from the control log
- [ ] 3.3 Check my mix tab; click plays from 2 bars before

## 4. Quick fixes

- [ ] 4.1 Edit list beside the recording and a renderer that writes a new file
- [ ] 4.2 Trim with fades; track level with eased ends; cut with crossfade
- [ ] 4.3 Loudness presets through the meter and true-peak limiter, with before/after
- [ ] 4.4 Fixes tab with undo; Save

## 5. Export

- [ ] 5.1 Mixcloud tracklist, YouTube chapters (checked against YouTube's rules) and .cue, following cuts and trims
- [ ] 5.2 Export for Mixcloud from the edited file

## 6. Pair mixes

- [ ] 6.1 `pair_mixes` table and Automix plan lookup: pair mix, then track points, then phrases
- [ ] 6.2 Save pair mixes from the transitions list: tick per transition, existing pair mix shown, newer replaces it

## 7. Docs

- [ ] 7.1 User guide page for the Mixed Set Editor (review, Check my mix, quick fixes, export) and pair mixes in website/book/src/guide

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.
