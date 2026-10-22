# Proposal

Planned for: **v0.4 · Your collection, cleaned up (collection maintenance PR 6)**

## Why

Seba organises music as crates (Yoga, Journeys, Dance) and per-gig set folders (Staging, Set, Live set, sometimes Openers, Closers, Transitions). Lists today are one kind, and building a Live set from History is manual.

## What Changes

- Two kinds of list folder, crate and set, shown in different colours.
- New set… makes a set folder from a template (Staging, Set, Live set, plus optional Openers, Closers, Transitions).
- History rows get Add to Set List…, which asks which set's Live set to fill.
- A track can be in any number of crates, and lists may hold Triage tracks.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `music-library`: crates and sets as two kinds of list folder, New set template

## Out of Scope

- Automatic Spotify playlists (a later idea).
- Exporting lists (see add-super-library).

## Impact

crates/library schema (list kind), app tree and menus.
