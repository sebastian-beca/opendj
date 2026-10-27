# Proposal

Planned for: **v1.0 · Ready for any DJ switching over** (the view-only editor ships first as `add-controller-viewer`)

## Why

Every DJ app lets you change what a control does. openDJ runs Mixxx's mappings but a DJ can't change one or map an unsupported controller.

## What Changes

- Builds on the Controller Editor from `add-controller-viewer`.
- Each controller has its **Original Mapping** (Mixxx's, read only) and any number of the DJ's own: **Duplicate** and **Rename**; one mapping per controller is in use at a time; **Reload Original** is always there.
- In a mapping of your own: change a row's function from a categorised, searchable menu (target chosen once: Deck 1-4, left/right side, FX unit, Main, Library), add or remove rows, and set advanced options: control type, button behaviour (hold, toggle, trigger), encoder mode, invert, takeover (jump, pick-up, value scaling), range, 14-bit pairing, comment. LEDs are edited in the Lights tab.
- Script-driven rows can be replaced by a plain binding; the script keeps running for the rest.
- Map mode, as in Ableton: the main window tints, click an on-screen control then move a hardware control; Learn stays armed, Enter or ⌘Z undoes the last one; Replace or Add when a control is already used.
- Mapping options (Mixxx's `<settings>` plus openDJ's own) can be changed per mapping.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `controllers`: own mappings, editing, Map mode

## Out of Scope

- Saving back to Mixxx's format (Seba, 2026-10-27: Mixxx mappings are read in; openDJ mappings are their own format so they can do more).
- Editing JavaScript.
- HID controllers (much later).

## Impact

crates/controllers (openDJ mapping format, writer, learn), app editor and main window hit-testing for Map mode.
