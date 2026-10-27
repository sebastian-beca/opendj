# Proposal

Planned for: **v0.7 · Your library, everywhere**

## Why

DJs switching from Mixxx, Serato, Traktor or an Apple Music library need their cues, grids and lists to come with them. The VirtualDJ import set the pattern: one importer per source, read-only, into a field superset.

## What Changes

- Importers for the Mixxx library (`mixxxdb.sqlite`: tracks, cues, saved loops, beat grids, crates, playlists and history), Serato crates and tags, Traktor NML, and the Apple Music / iTunes library, each switched on in Settings › Library with the same wizard as VirtualDJ.
- Cross-source matching by artist, title and duration so two apps pointing at different copies don't double the library.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library-import`: Mixxx, Serato, Traktor and Apple Music importers; cross-source matching

## Out of Scope

- rekordbox and OneLibrary import: dropped for now by Seba (2026-10-19); revisit when a tester asks.
- Writing back to other apps.

## Impact

crates/plugins importers, library merge, settings.
