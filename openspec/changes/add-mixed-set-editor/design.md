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

## Layout

- Header: name, date, length, number of tracks and transitions; Play/Pause; Export menu with what exists today (Export for Mixcloud, tracklist).
- Whole set overview: the recording's waveform with the track blocks above it and a box for what the timeline shows; drag the box to scroll.
- Timeline: a row per deck that played (2 or 4), track blocks with their waveform from analysis, transitions shaded with length in bars and style, loops as brackets with a repeat count, hard moves hatched. Lanes below (volume, bass, filter, crossfader, FX), folded by default. Zoom from the whole set to 8 bars.
- Side panel: the transitions list (number, out track → in track, time in the set, length, style). Clicking a row scrolls the timeline to it and plays from 4 bars before; the selected transition's details show above the list.
- Nothing in the set can be changed (editing comes in add-set-view); the only thing saved is pair mixes.
