# Proposal

Planned for: **v0.6 · Buy it from the artist** (first step of the split `add-settings-layers`, 2026-11-05; issue #364)

## Why

Settings has grown to 11 pages, but there is no single list of the settings. `prefs.rs` reads and writes about 97
values, and each one's default is typed where it is read, so `main.rs`, `browser.rs` and `automix.rs` repeat some
of them. A DJ can't see what they changed or put one thing back. Many rows also carry a grey help line under a
name that already says what it does, against the short-labels rule (help in an ⓘ popover only when needed).

## What Changes

- **One option table**: every Settings row is one entry (key, page, tab, plain name, help, search words, default).
  Pages, search and the read sites take the default from it, so a default lives in one place.
- **Show modified**: a switch in the Settings header. On, every page (and a search) lists only the rows that differ
  from their default.
- **Reset**: a small reset arrow on each changed row puts it back at once; **Reset all** in the header asks first.
- **Search by meaning**: each row's search words are matched too ("pitch" finds tempo, "PFL" finds Split cue,
  "hamster" finds Reverse crossfader).
- **Help in ⓘ**: the grey line under a row's name goes; help that adds something moves to the ⓘ popover beside the
  name, as folder lists already do.

## Capabilities

### Modified Capabilities

- `app-layout`: Show modified and reset, search by meaning, help in an ⓘ popover

## Out of Scope

- Settings › Advanced and layer-3 rows (`add-settings-layers`, later).
- Profiles, which will save what Show modified lists (later).
- Rows that hold the DJ's content rather than a choice (folder lists, tag groups, sources, plugins, shortcuts): they
  are not in Show modified.

## Impact

`crates/app/src/prefs.rs`, `crates/app/src/settings.rs` and their read sites, `preferences.slint`, the user guide's
Settings page.
