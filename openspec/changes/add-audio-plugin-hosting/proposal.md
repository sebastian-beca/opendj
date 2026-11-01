# Proposal

Planned for: **v2.0**

## Why

Mixxx only hosts LV2 on Linux. Hosting VST3, CLAP and AU effects opens openDJ to the effects DJs already own. Other DJ apps and DAWs crash or start slowly with plugins: they scan every installed plugin at startup and run plugins inside the app, so one bad plugin stops the music. openDJ must allow plugins without either.

## What Changes

- Load CLAP, then VST3, then (macOS) AU effects into effect slots.
- Every plugin runs in an `opendj-plugin-host` helper process. A block the helper can't deliver in time plays dry.
- Plugins are scanned only when the DJ asks, each in a throwaway process with a timeout, and cached.
- Only plugins the DJ switches on appear in the FX chooser, and each loads only when first used.
- Crash and hang recovery, per-plugin CPU budget, latency compensation, editors in their own windows.

Research and reasoning: `notes/plugin-hosting-review.md` in the project files (2026-11-01).

Licences: CLAP is MIT; the VST3 SDK is MIT since 3.8 (the VST logo needs Steinberg's separate logo licence); AU is Apple's API. All fit GPL-2.0-or-later.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `effects`: third-party effect plugins

## Out of Scope

- Instrument plugins.
- Sandboxing plugins' file access (process isolation stops crashes, not hostile code).

## Impact

New crate `crates/plugin-host` (library for the engine, `opendj-plugin-host` helper binary); effects engine; Settings › Plugins; release packaging and macOS entitlements for the helper.
