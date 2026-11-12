# Design

## Timeline model

`SetTimeline` is built from `<name>.controls.jsonl` (snapshot, control lines, `load` lines, `pos` lines):

- **Clips**: a stretch of playback on one deck, from a load or jump to the next jump, stop or unload. Each has the track, its start in the recording and its start in the track, and its tempo (from rate and grid).
- **Lanes**: per deck volume, EQ low/mid/high, filter, gain; crossfader; FX unit enable and parameters. Each lane is a list of (time, value) points straight from the log.
- **Transitions**: where two decks are audible at once. Audible = deck playing and its gain after fader, crossfader curve and EQ is above -30 dB of its own level. A transition runs from the moment the second deck becomes audible to the moment the first stops being audible. Shorter than 2 bars counts as a cut.
- **Style found**: bass swap (low EQ of one deck crosses the other's inside the transition), filter (filter moved more than a third of its range), echo out (an echo or delay unit on the outgoing deck at the end), smooth (volume only), cut.

Loops (loop on to loop off, with the bounds and the number of times round) and hard moves (stretches where a deck's place does not follow play, rate and jumps: scratches, backspins, brakes) are marked on the clips. The editing in add-set-view and the Ableton export (add-ableton-set-export step 3) read the same model, so the log is parsed in one place.

## Pair mixes

For each transition: out point = where the incoming track became audible, given in the outgoing track's time, snapped to the nearest beat; in point = where the incoming track was when it became audible; length in bars and style found from the timeline model.

`pair_mixes(out_track, in_track, out_seconds, in_seconds, bars, style, source, created)`, unique on (out_track, in_track); saving a newer mix of the same pair replaces the row. Automix's plan (`app/src/automix_phrases.rs`) looks it up first; then the tracks' own points (`markers` of kind `automix`, `fadeStart` / `realStart`); then phrases, as today. Learning never writes `markers`.

The transitions list has a tick per row (on for transitions of 4 bars or more, off for cuts) and a "Save pair mixes" button; a row whose pair already has a saved mix shows it, and saving replaces it.

## Recordings without a usable log

Recordings made before the control log, or whose log is missing, open with the recording's waveform as a single row and the tracklist from the `.cue` sheet; there are no deck rows, lanes or transitions, and no pair mixes to learn.

## Check my mix

Each finding has a time, a short text and a severity; clicking plays from 2 bars before.

- Clipping: master sample peak at 0 dBFS, or limiter gain reduction over 3 dB for more than 1 s (from the recording itself).
- Silence: below -50 dBFS for more than 2 s, away from the start and end.
- Level jump: a track's short-term loudness (after the transition) differs from the previous one's by more than 3 LU.
- Drift: during a transition, the two decks' beat phases (from their places and grids in the log) differ by more than 20 ms for more than 2 beats.
- FX left on: an FX unit still on a deck 4 bars after a phrase change it was switched on before.
- Jump mid-phrase: a hot cue or seek that lands more than 1 beat away from a phrase start while the deck is audible.

The log-based checks need a control log; without one only clipping, silence and level jumps run.

## Quick fixes

The recording is never changed. Fixes are a list of edits saved beside it (`<name>.edits.json`) and Save renders a new file `<name> (edited).wav|flac` by reading the recording, applying the edits in order and writing the result:

- Trim: new start and end, snapped to a bar when there is a grid, with a fade in and out (default 2 s and 5 s).
- Track level: a gain in dB over a track's audible stretch, eased over one beat at each end. Since the recording is the whole mix, this also moves the other track inside its transitions; the ease sits at the transition's middle, and the panel says so.
- Cut: from bar to bar, a 20 ms equal-power crossfade at the join; the transitions list, tracklist and chapters after it move earlier.
- Loudness: measure the edited audio (BS.1770, as Analyze does), apply one gain to reach the preset, then the true-peak limiter at -1 dBTP; the panel shows before and after and warns when more than 3 dB of limiting would be needed.

## Layout

- Header: name, date, length, number of tracks and transitions; Play/Pause; Show decks switch (off by default); Save (writes the edited file when there are fixes); Export menu (Mixcloud tracklist, YouTube chapters, .cue, Export for Mixcloud).
- Whole set overview: the recording's waveform with the track blocks above it and a box for what the timeline shows; drag the box to scroll.
- Simple view (default): the recording's waveform as one row with a block per track above it and the transitions shaded.
- Timeline with Show decks on: a row per deck that played (2 or 4), track blocks with their waveform from analysis, transitions shaded with length in bars and style, loops as brackets with a repeat count, hard moves hatched. Lanes below (volume, bass, filter, crossfader, FX), folded by default. Zoom from the whole set to 8 bars.
- Side panel tabs: Transitions, Check my mix, Fixes. Transitions: the transitions list (number, out track → in track, time in the set, length, style). Clicking a row scrolls the timeline to it and plays from 4 bars before; the selected transition's details show above the list.
- Fixes tab: Trim, Track level (for the selected track), Cut (select a range on the waveform), Loudness preset; each fix listed with undo.
- The mix itself cannot be changed (editing comes in add-set-view).
