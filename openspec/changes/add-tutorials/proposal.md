# Proposal

Planned for: **v0.5 · Stems, performance and switching over** (Seba, 2026-10-29)

## Why

openDJ's own ideas (stages, albums, mixed sets) and its deeper features are easy to miss. After the first-run tour, DJs should be able to learn each one inside the app when they're ready.

## What Changes

- Advanced tutorials in Help › Tutorials, each a short hands-on walk on the same tour engine as the first-run tour (`add-first-run-setup`), each marked done once finished:
  - **Clean Collection**: stages (Inbox, Triage, Collection), Library Health, duplicates, name clean-up, online lookup, albums and mixed sets, crates and sets.
  - **Looping**: auto loops, loop in/out, halve/double, saved loops, loop roll.
  - **Using FX**: the FX bar, Beat FX, deck FX, Color FX on the filter knob.
  - **Filtering tracks**: search, filter chips, key and BPM match, smart lists.
  - **Automix**: building a queue, transitions, handing back to the DJ.
- Each tutorial links to its user guide page.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `app-layout`: Help › Tutorials

## Out of Scope

- Video tutorials.

## Impact

app UI (tour engine from `add-first-run-setup`), website/book guide links. Depends on `add-first-run-setup`.
