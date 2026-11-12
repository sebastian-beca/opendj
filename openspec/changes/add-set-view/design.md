# Design

## Timeline model

`SetTimeline` is built from `<name>.controls.jsonl` (snapshot, control lines, `load` lines, `pos` lines):

- **Clips**: a stretch of playback on one deck, from a load or jump to the next jump, stop or unload. Each has the track, its start in the recording and its start in the track, and its tempo (from rate and grid).
- **Lanes**: per deck volume, EQ low/mid/high, filter, gain; crossfader; FX unit enable and parameters. Each lane is a list of (time, value) points straight from the log.
- **Transitions**: where two decks are audible at once. Audible = deck playing and its gain after fader, crossfader curve and EQ is above -30 dB of its own level. A transition runs from the moment the second deck becomes audible to the moment the first stops being audible. Shorter than 2 bars counts as a cut.
- **Style found**: bass swap (low EQ of one deck crosses the other's inside the transition), filter (filter moved more than a third of its range), echo out (an echo or delay unit on the outgoing deck at the end), smooth (volume only), cut.

The Ableton export (add-ableton-set-export step 3) reads the same model, so the log is parsed in one place.

## Automix points from a set

For each transition: out point = where the incoming track became audible, given in the outgoing track's time, snapped to the nearest beat; in point = where the incoming track was when it became audible. Saved as `markers` rows of kind `automix`, names `fadeStart` / `realStart`, source `set`. Existing points from imports are shown beside them and kept unless the DJ ticks to replace.

A pair mix row: `pair_mixes(out_track, in_track, out_seconds, in_seconds, bars, style, source, created)`, unique on (out_track, in_track). Automix's plan (`app/src/automix_phrases.rs`) looks it up first; then the track points; then phrases, as today.

## Render

The engine already runs from controls. An offline render creates an engine with no audio device, loads the set's tracks, replays the (edited) event list at each event's frame, and pulls buffers as fast as it can into a WAV or FLAC writer. Two checks decide whether a render is trustworthy:

1. **Null test**: render an unedited log and subtract it from the recording. Target: residual below -60 dBFS outside plugin effects. Needs frame-accurate stamps (step 2) and deterministic DSP (no random seeds per run, key lock with fixed block sizes).
2. **Missing sources**: a track file that moved or a plugin that is not installed makes the render stop before it starts and says which.

Parts the DJ did not edit could later be copied from the recording instead of rendered; not needed for a first version.

## Frame-accurate log

Control changes made from the UI or a controller are applied by the audio thread at a buffer boundary. The audio thread writes (frame, control id, value) into a lock-free ring the logger drains, in place of the logger's 5 ms poll. Plugin parameter changes and instrument notes go through the same ring. Mic and Aux files are written beside the recording with the same frame base.
