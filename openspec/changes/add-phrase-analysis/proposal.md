# Proposal

Planned for: **v0.7 · Your library, everywhere**

## Why

Phrases (intro, verse, chorus, breakdown, outro) feed smart cues, Automix and lighting later. rekordbox, VirtualDJ and Traktor 4 have them.

## What Changes

- Detect phrase sections on the beat grid during analysis.
- Show them as coloured bands on the overview waveform.
- Search by phrase (`has:intro>=16`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `track-analysis`: phrase sections from analysis
- `waveforms`: phrase bands on the overview

## Out of Scope

- Lighting from phrases (much later).

## Impact

engine analysis, library schema, waveform shader.
