# Proposal

Planned for: **v0.6 · Stems and performance**

## Why

Seba uses stems a lot. Separation needs Demucs installed by hand and is slow; quality and speed decide whether stems are usable live.

## What Changes

- Ship a separation model with openDJ instead of needing Demucs installed.
- Faster separation (GPU where available), and separate tracks ahead of time in the background (Triage, a list, a set).
- Show which tracks have stems ready.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `stems`: bundled, faster, ahead-of-time separation

## Out of Scope

- Real-time separation while loading (later, if speed allows).

## Impact

crates/engine stems.rs, model files in the release, ONNX or similar runtime. See notes/stems-review.md.
