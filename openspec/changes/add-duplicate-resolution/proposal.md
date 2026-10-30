# Proposal

Planned for: **v0.4 · Your collection, cleaned up (collection maintenance PR 3)**

## Why

Collection Health already counts duplicates but can only list them. Seba's VirtualDJ scan found about 320 duplicate groups; he wants them grouped by case and fixed in batches, with the safe, lossless choices one click away.

## What Changes

- Group duplicates by case: same file twice, same audio in another format or bitrate, same track in another folder or stage, and likely matches by artist, title and duration.
- Pick a keeper per group by rules (lossless over lossy, higher bitrate, in Collection over Triage over Inbox, more cues and plays).
- Merge: the keeper takes the others' cues, loops, grid edits, tags, ratings, play counts and list slots; the others go to a quarantine folder, never deleted.
- One-click fix for groups where the rules agree and nothing would be lost.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library-health`: Duplicates gets grouping, keepers, merge and quarantine

## Out of Scope

- Audio fingerprint matching (see add-super-library).
- Deleting files: quarantine only.

## Impact

crates/library (health, merge), crates/app browser/health.rs, a quarantine folder setting.
