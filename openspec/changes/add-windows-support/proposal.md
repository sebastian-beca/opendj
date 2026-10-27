# Proposal

Planned for: **v2.0**

## Why

Windows is the largest DJ platform; deferred to 2.0 by decision. The app already compiles on windows-latest.

## What Changes

- Windows builds and installer; Windows paths for config and cache; file reveal and pickers; Demucs and add-on .exe handling; Ctrl labels; ASIO and WASAPI audio.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `platforms`: Windows 10/11 support

## Out of Scope

- Windows on ARM.

## Impact

CI, platform modules. See the Windows estimate artifact (about 7 PRs).
