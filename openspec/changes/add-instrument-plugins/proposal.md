# Proposal

Planned for: **v2.0**, after `add-audio-plugin-hosting`

## Why

Seba (2026-11-01): loading instrument plugins could make sense, so a DJ can play a synth, drum machine or sampler instrument live over the mix. This fits the project's aim of bringing live music and groove boxes into a DJ set.

## What Changes

- An instrument slot that loads a CLAP, VST3 or AU instrument, hosted out of process exactly like effect plugins (same helper, scanning, allow-list, crash recovery and CPU budget).
- It plays as an extra input channel next to Mic and Aux: its own fader, gain, EQ, cue and the microphone's effect unit.
- Notes come from a MIDI keyboard, the Keyboard pad page, or a controller mapping; the instrument gets the master tempo and beat position for its arpeggiators and sequencers.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `mic-aux-inputs`: an instrument input

## Out of Scope

- Multi-timbral instruments and several outputs per instrument.
- Recording or editing MIDI clips (see `add-pattern-sequencer`).

## Impact

`crates/plugin-host` (note events in, instrument audio out), engine inputs, MIDI input routing, MIC panel.
