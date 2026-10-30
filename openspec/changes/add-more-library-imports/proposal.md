# Proposal

Planned for: **v0.5 · Stems, performance and switching over** (moved from v0.7 by Seba, 2026-10-29)

## Why

DJs switching from rekordbox or Mixxx need their cues, grids and lists to come with them, and VirtualDJ users (Seba included) need the built-in VirtualDJ import to bring everything across. The VirtualDJ import set the pattern: one importer per source, read-only, into a field superset.

## What Changes

- **rekordbox** (brought back from "dropped", Seba 2026-10-29; asked for in issue #75): read `master.db` with rbox (already used for Pioneer USB export), with the rekordbox XML export as fallback. Tracks, hot and memory cues with colours, saved loops, beat grids (including tempo changes), phrases, rating, colour, comments, play count, playlists, intelligent playlists as smart lists, and history.
- **Mixxx**: read `mixxxdb.sqlite`: tracks, hot cues, saved loops, beat grids, rating, play count, crates, playlists and history.
- **VirtualDJ, updated**: pick which lists to import (today every playlist, virtual folder and history comes across), plus whatever the field superset still drops (checked against Seba's live database).
- Each one switched on in Settings › Collection › Import, where rekordbox and Mixxx have "coming soon" rows today, with the same three-step wizard as VirtualDJ.
- Cross-source matching by artist, title and duration so two apps pointing at different copies don't double the library.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library-import`: rekordbox and Mixxx importers, VirtualDJ list picking, cross-source matching

## Out of Scope

- Serato, Traktor, Apple Music and OneLibrary importers: later, as their own change.
- Writing back to other apps.

## Impact

crates/plugins importers (rbox for rekordbox), library merge, Settings › Collection › Import.
