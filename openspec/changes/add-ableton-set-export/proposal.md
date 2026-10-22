# Proposal

Planned for: **v1.0 · Ready for any DJ switching over**

## Why

Turning a recorded set into an Ableton Live set lets a DJ edit the mix or make a release from it. The v0.2 recorder already logs every control change for this.

## What Changes

- Layer 1: a Live set with the master recording, one pre-fader audio track per deck, and the controller MIDI.
- Layer 2 (if time allows): each played track as a warped clip on its deck's track, split at cue jumps and loops, with volume, crossfader, EQ and filter as automation.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `mix-recording`: per-deck stems in recordings and .als export

## Out of Scope

- openDJ effects recreated as Live devices beyond Echo, Reverb and Auto Filter.

## Impact

engine recorder, a new .als writer. See roadmap feasibility notes.
