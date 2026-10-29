# Proposal

Planned for: **v0.5 · Stems, performance and switching over** (Seba, 2026-10-29)

## Why

openDJ's deeper features are easy to miss. After the first-run tour, DJs should be able to learn each one inside the app, with real music playing, when they're ready.

## What Changes

- Hands-on tutorials in **Help › Tutorials**, on the same tour engine as the first-run tour (`add-first-run-setup`): a highlight on the real control, a short tip, and the step moves on only once the DJ does it (on screen or on the controller). **Skip step** and **Close** are always there. Each is marked done once finished and can be run again.
- They play the CC0 demo tracks that ship with openDJ for the first-run tour, or the DJ's own tracks if they pick **Use my tracks**.
- Five tutorials (steps in `design.md`):
  - **Looping**: auto loops, halve/double, loop in/out, saving a loop on a hot cue pad, loop roll.
  - **Using FX**: the FX bar, deck FX, Beat FX, Color FX on the filter knob.
  - **Filtering tracks**: search, filter chips, key and BPM match, saving a search as a smart list.
  - **Automix**: building a queue, length and style, Mix now and Skip, taking over.
  - **DDJ-FLX4**: the same basics on the controller: load, play/cue, jog, tempo and sync, EQ and filter, pad modes, Beat FX. The first controller-specific tutorial; others can follow the same pattern.
- Each tutorial links to its page in the user guide (`website/book/src/guide`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `app-layout`: Help › Tutorials

## Out of Scope

- A Clean Collection tutorial (stages, health, duplicates, clean-up, lookup, albums, mixed sets, crates and sets): skipped for now (Seba, 2026-10-29).
- "Learn this" links on panels: Help › Tutorials only for now.
- Tutorials for other controllers.
- Video tutorials.

## Impact

app UI only, on the tour engine from `add-first-run-setup` (overlay, highlight, wait-for-action steps, done marks); the demo tracks it ships; guide links. Depends on `add-first-run-setup`.
