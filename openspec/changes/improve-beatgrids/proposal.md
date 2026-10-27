# Proposal

Planned for: **v1.0 · Ready for any DJ switching over**

## Why

Grids drive sync, loops and FX. Detection should find bar 1 and handle live drums reliably, and fixing a grid should take seconds on the waveform.

## What Changes

- Better downbeat detection and drift handling; an "unsure" warning when a grid drifts.
- Grid edit mode on the zoomed waveform: drag a beat marker, tap along to set tempo, set the downbeat with one click.
- Lock a grid so analysis never overwrites it.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `beatgrids`: grid edit mode, lock and unsure warning
- `track-analysis`: better downbeat detection

## Out of Scope

- Grid editing on controllers.

## Impact

engine analysis and beats, waveform interaction.
