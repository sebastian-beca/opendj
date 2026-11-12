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

## Hard moves

A hard move is a stretch where a deck's place in its track does not follow play, rate and jumps: the jog in scratch mode (`scratch2_enable`), a backspin or a brake. The recorder (step 2) logs the deck's place every audio block while one lasts. The timeline shows it as one block on the clip, from the first to the last block that did not follow normal play, padded to the nearest beat on each side.

- **Keep from recording** (default): the render copies that stretch of the recording's audio, with a 10 ms crossfade at each edge, in place of rendering it. The copy holds the whole mix at that moment, so if the DJ edited another deck's lanes or a transition overlapping the block, the render plays the logged positions back through the engine instead (exact for openDJ's own processing) and says so on the block.
- **Bypass**: the deck plays straight through at its rate from where it was when the move began. The rest of the clip follows from there, so later cue jumps and loops keep their places in the track; the Set view shows the clip shifted where the place differs.

The Ableton export treats a kept hard move the same way: a rendered audio clip on the deck's track in place of warped clips.

## Frame-accurate log

Control changes made from the UI or a controller are applied by the audio thread at a buffer boundary. The audio thread writes (frame, control id, value) into a lock-free ring the logger drains, in place of the logger's 5 ms poll. Plugin parameter changes and instrument notes go through the same ring, and so do the dense place lines during a hard move. Mic and Aux files are written beside the recording with the same frame base.
