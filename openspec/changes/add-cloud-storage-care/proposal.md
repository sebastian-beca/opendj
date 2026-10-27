# Proposal

Planned for: **v0.4 · Your collection, cleaned up (collection maintenance PR 7)**

## Why

Seba's collection (about 53 GB) stays in iCloud Drive with only 18 GB free locally. macOS can offload files at any time, which is dangerous before a gig.

## What Changes

- Keep downloaded: pin the Collection (or chosen lists) so macOS never offloads them.
- A pre-gig check that warns about offloaded tracks in the lists about to be played, with Download now.
- Show offloaded tracks with a cloud mark in lists.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `music-library`: iCloud-aware storage: keep downloaded, offload warnings

## Out of Scope

- Moving the collection off iCloud (Seba keeps it there).
- Other cloud drives beyond detection.

## Impact

macOS-only file APIs (NSURL ubiquitous item keys, `brctl`), app browser.
