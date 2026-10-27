# Proposal

Planned for: **v0.6 · Buy it from the artist**

## Why

Bandcamp search, store links, the wanted list and the purchase inbox are built (PR #27) but switched off in release builds, so every store source says "Coming soon". The artist-direct release is where they go live.

## What Changes

- Switch on Bandcamp and Shopping in the rail.
- Bandcamp search results in the list, opening the purchase page in the browser; other stores as link-outs.
- The purchase inbox: watch a chosen folder, unpack, recognise, file into the music folder, add to the Inbox stage.
- The wanted list: tracks the DJ means to buy.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library-sources`: Bandcamp, Shopping and the purchase inbox go live

## Out of Scope

- Taking payment in the app.
- Streaming playback.

## Impact

app browser SERVICES_ENABLED split per source, stores.rs, inbox.rs, settings.
