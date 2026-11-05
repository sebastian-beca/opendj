# Proposal

Planned for: **v0.6 · Buy it from the artist** (second step of the split `add-settings-layers`, 2026-11-05; issue #365; follows
`add-settings-table`)

## Why

Settings › Decks is one list of about 12 rows. Splitting it by what the DJ is doing makes it easy to scan, and three
choices picked from the settings review belong there: what a hot cue does on a playing deck, a warning before a
track ends, and a backup of the collection.

## What Changes

- Settings › Decks gets sections **Loading**, **Playing** and **Tempo and key**; the existing rows move under them.
- **Hot cue on a playing deck**: Jump and keep playing (default) / Stutter. Also on a hot cue pad's right-click menu.
- **Track end warning**: 30 s (default) / 20 / 60 / Off, under Playing. The warning itself is built in its own change;
  this step adds and wires the row.
- **Back up the collection**: Off / Daily (default) / Weekly and a folder, on Settings › Collection › General. The
  backup itself is built in its own change; this step adds and wires the rows.
- Each new row goes into the option table from `add-settings-table`.

## Capabilities

### Modified Capabilities

- `app-layout`: Decks page sections, deck choices on the control
- `deck-transport`: hot cue on a playing deck

## Out of Scope

- The track end warning and collection backup features (own changes, own threads).
- Tempo on load, Load at, Match tempo on load, instant doubles and the other choices in `add-settings-layers`.

## Impact

`prefs.rs` Decks and Collection pages, engine hot cue handling, hot cue pad menu in `deck.slint`, user guide.
