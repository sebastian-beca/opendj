# Proposal

Planned for: **v0.4 · Your collection, cleaned up**

## Why

DJs check a track in the headphones before loading it. The engine has `[PreviewDeck1]` but nothing in the browser drives it.

## What Changes

- A preview button on each row (and Shift+click on the cover) that plays the track in the headphones only.
- A small waveform in the side panel to scrub the preview.
- Controllers' library preview controls drive it.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `track-browser`: preview a track in the headphones from the list

## Out of Scope

- Preview of streaming tracks.

## Impact

app browser rows, side panel, engine preview.rs unchanged.
