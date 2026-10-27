# Proposal

Planned for: **v0.4 · Your collection, cleaned up**

## Why

Sync now follows a leader and keeps phase (PR #24), but the screen still lacks the controls the sync review proposed: a per-deck quantize button, a quantize resolution, and a sign of which deck leads.

## What Changes

- A Q button per deck for `quantize`, replacing the global-only switch (the setting becomes the default).
- Quantize resolution: beat, half beat, bar.
- A leader badge on the deck that synced decks follow, and a way to pin the leader.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `sync`: per-deck Q, resolution, leader badge and pinning

## Out of Scope

- Sync to Link peers as leader (Link already follows the master deck).

## Impact

engine sync leader picking, deck.slint and fourdeck.slint, controls list.
