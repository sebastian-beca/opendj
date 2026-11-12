# Proposal

Planned for: **v0.7 · Your library, everywhere**

## Why

A recorded set is only a sound file today, with a cue sheet beside it. openDJ already logs every control change, track load and jump next to each recording (`<name>.controls.jsonl`), so it can show the set as it was played: which tracks, where each transition happened, how long it lasted and what was moved. Mix editors such as DJ.Studio show mixes this way but cannot open a set played live. This is the read-only first step; editing, re-rendering and exports to Ableton follow in v0.8 (add-set-view, add-ableton-set-export).

## What Changes

- **Mixed Set Editor**: right-click a recording in Recordings (or a History set that has one) › Open in Mixed Set Editor. It shows the whole set as a timeline: a row per deck with track blocks and waveforms, transitions shaded with their length in bars and the style found (bass swap, filter, echo out, smooth, cut), loops as brackets with a repeat count, scratches and other hard moves hatched, and folded lanes for volume, bass, filter, crossfader and FX. An overview of the whole set scrolls it; zoom goes from the whole set to 8 bars.
- **Playback**: the recording plays with a playhead across the timeline; clicking anywhere jumps there.
- **Transitions list**: a side panel listing every transition (out → in, time, length, style); a click jumps to 4 bars before it.
- Recordings without a control log open with the recording's waveform and the cue sheet's tracklist only.

Mockups (board 1; board 2 is v0.8): https://claude.ai/artifact/76fLT6FF4ULjWNcXRSQCzv

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `mix-recording`: open a recording as a timeline with its transitions

## Out of Scope

- Changing anything in the set, rendering a new mix, Keep or Bypass for scratches, frame-accurate capture (add-set-view, v0.8).
- Ableton export (add-ableton-set-export, v0.8).
- Recordings made by other apps (no control log).
- Learning Automix points or pair mixes from a set: Automix points are set in Prep (add-prep-layout).

## Impact

A control-log reader and timeline model in a new module (shared with v0.8), the Mixed Set Editor screen and its menu entries.
