# Proposal

Planned for: **v0.4 · Your collection, cleaned up**

## Why

Some of Seba's M4A files and common formats (Opus, WavPack, HE-AAC) don't load with the current decoder set.

## What Changes

- Decode Opus, WavPack and HE-AAC, using system decoders (CoreAudio on macOS, FFmpeg on Linux) where the pure-Rust ones fall short.
- Say clearly when a file can't be decoded instead of loading silence.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `deck-transport`: more formats load; undecodable files are reported

## Out of Scope

- DRM-protected files.

## Impact

crates/engine decode.rs, platform decoder bindings, release build dependencies.
