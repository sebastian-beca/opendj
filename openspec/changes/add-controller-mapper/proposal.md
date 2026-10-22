# Proposal

Planned for: **v1.0 · Ready for any DJ switching over**

## Why

Every DJ app has MIDI learn. openDJ runs Mixxx's mappings unchanged but a DJ can't change one or map an unsupported controller.

## What Changes

- A mapper in Settings › Controllers: a list of every mapped control, editing, and MIDI learn (click a control, move a knob).
- Save, share and import custom mappings; Mixxx XML/JS stays the file format.
- Custom mappings override built-in ones and survive updates.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `controllers`: mapping editor with MIDI learn and custom mappings

## Out of Scope

- HID controllers (much later).

## Impact

crates/controllers mapping writer, app prefs, UI hit-testing for learn.
