# Proposal

Planned for: **v0.8 · From the booth to the studio**

## Why

A recorded set is only a sound file today. DJ.Studio and similar mix editors let a DJ fine-tune transitions and FX on a timeline and export the result, but they have no live mode and cannot open a set played elsewhere: they build mixes from playlists. openDJ already logs every control change, track load and jump beside each recording (`<name>.controls.jsonl`), so it can show a set it recorded as a timeline, let the DJ fix it, and render it again with its own engine. No DJ app does this, and no DJ app learns Automix points from the way the DJ actually mixed live (VirtualDJ, Mixxx, Traktor and djay only store points set by hand or by analysis).

## What Changes

- Step 1, Automix points from a set (the simple version): read a recording's control log, find each transition (the stretch where two decks are heard), and offer to save it as Automix points: a mix-out point on the outgoing track, a mix-in point on the incoming one (the same `fadeStart` / `realStart` markers imported from VirtualDJ, djay and Traktor), plus a pair mix for that ordered pair of tracks (in and out points, length in bars, style found), as VirtualDJ's Automix editor keeps per pair. Automix uses a pair mix first, then the track's points, then phrases. "Play it again as Automix" puts the set's tracks in an Automix list.
- Step 2, capture fixes: control changes stamped with the audio frame they took effect on (not the logger's 5 ms poll), plugin parameter changes and instrument / Groove Box notes in the log, an option to record Mic and Aux inputs to their own files, and the deck's exact place in its track every audio block while it is scratched (jog in scratch mode, backspin, brake).
- Step 3, Set view (read only): right-click a recording in Mixed Sets or a History set with a recording, "Open in Set view". One row per deck with track blocks and waveforms, transitions shaded with their length and style, lanes for volume, bass, filter, crossfader and FX, an overview of the whole set, playback of the recording with a playhead, and a short list of things to check (FX left on past a drop, a jump mid-phrase, the limiter working hard).
- Step 4, edit and render: drag a transition earlier or later (snapping to beats and phrases), stretch it, pick a style, open a transition editor with both tracks on one beatgrid and editable curves (VirtualDJ's Automix editor layout), loops shown as brackets with a repeat count and editable length and count, A/B against the recording, then render the edited set offline through openDJ's engine to a new file beside the recording. The recording is never changed.
- Hard moves (scratches, backspins, brakes and anything else the render cannot rebuild from mixer controls) show as one block on the timeline with two choices, not editable stroke by stroke: **Keep from recording** (the default: that stretch of audio is copied from the recording into the render) or **Bypass** (the deck plays straight through as if the move never happened). DJs fine-tune fader, EQ, filter and FX lines; they keep or drop a scratch whole.
- Step 5, exports from the same timeline: Mixcloud export of the rendered mix, and the timeline model reused by add-ableton-set-export step 3.

Mockups: https://claude.ai/artifact/76fLT6FF4ULjWNcXRSQCzv

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `mix-recording`: frame-accurate log, Set view, render an edited set
- `automix`: points and pair mixes learned from a recorded set

## Out of Scope

- Building a mix from a playlist without playing it (DJ.Studio's main use); it can reuse step 4 later.
- Video export, stems placed on the timeline, re-ordering a whole set.
- Recreating third-party plugin state that was not logged.
- Learning from recordings made by other apps (no control log).

## Impact

engine recorder (frame stamps, plugin and note lines, input files), a set-log reader and timeline model shared with the Ableton export, a new `pair_mixes` table in the library, Automix plan lookup, a Set view screen and transition editor in the app, an offline render path that drives the engine from a log.
