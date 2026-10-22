# Proposal

Planned for: **v0.4 · Your collection, cleaned up (collection maintenance PR 5)**

## Why

Many tracks lack year, label, genre or a good cover. Looking them up by audio fingerprint gives reliable identities without typing.

## What Changes

- Identify tracks by AcoustID fingerprint, then MusicBrainz for identity and Discogs (with the DJ's token) for label, catalogue number, year and styles.
- Covers from Cover Art Archive, iTunes and Discogs, preferring the largest up to about 1024 px.
- Results go into the clean-up review table; nothing is applied unasked.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library-health`: Look up online fills missing info and covers

## Out of Scope

- Paid metadata services.

## Impact

New lookup module (rusty-chromaprint, HTTP clients), app coverart.rs reuse, settings for tokens.
