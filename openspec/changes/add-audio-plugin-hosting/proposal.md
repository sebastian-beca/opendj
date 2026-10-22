# Proposal

Planned for: **v2.0**

## Why

Mixxx only hosts LV2 on Linux. Hosting VST3, CLAP and AU effects opens openDJ to the effects DJs already own.

## What Changes

- Load VST3, CLAP and (macOS) AU effects into effect slots, out of process.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `effects`: third-party effect plugins

## Out of Scope

- Instrument plugins.

## Impact

effects host, new plugin-host process.
