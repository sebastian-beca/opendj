# Proposal

Planned for: **v0.6 · Buy it from the artist** (moved from v0.5 at the 0.5.0 release)

## Why

Pads and scratching must feel right for 1.0. Jog latency, platter feel and a fast crossfader cut are what scratch DJs notice first.

## What Changes

- Measure and lower jog-to-sound latency: a latency probe test, a quicker twin of Mixxx's scratch filter that leads while the hand changes speed fast, and 1 ms timers on Windows.
- Tune platter inertia and back-spin per controller.
- Crossfader cut-in under 1 mm on the Scratch curve, with a cut-in setting (`[Mixer Profile],xFaderCutIn`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `deck-transport`: scratch feel tuned and measured
- `mixer`: Scratch curve cut-in setting

## Out of Scope

- DVS (add-dvs, v0.6), which builds on this scratch path.

## Impact

engine scratch filter, controller host jog, mixer settings.
