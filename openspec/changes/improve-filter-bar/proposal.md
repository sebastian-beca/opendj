# Proposal

Planned for: **v0.6** (issue #468; design mockups chose option A with a finder)

The tasks still open moved to **v0.8 · From the booth to the studio** on 2026-11-12, so v0.7 ships with what is built.

## Why

The filter bar (chips per field, BPM and Key matching the playing deck) works, but it slows down as the library grows. Each chip counts its values over the whole list and ignores the other chips, so a combination can leave nothing. Long tag groups are sorted A–Z, so the tag on 40 tracks sits below one on a single track. Chips can't be moved, removing one means opening it and switching off "Keep on the bar", and every field shows up flat in "+" and in the column header menu.

Apps that handle facets well, such as shop facets, Lightroom's Library Filter, Linear and Gmail, count every value against the other filters, order values by count, show the top few with "Show all", and let you find a value without knowing its field first.

## What Changes

- **Linked filters.** A chip's values are counted with every other filter applied: other chips, the typed search and deck-following BPM/Key. Its own picks don't count, so you can add a second value of the same field.
- **Value order.** Picked values come first, then the most tracks in the current list, then values picked recently, then the field's own order (leading numbers as numbers, then A–Z). Values with no tracks sit behind "Show N more". Long lists show the top 8, then "Show all". A tag group can be set to Own order (for a sequence like Trip Stage).
- **Chips.** Drag a chip to reorder it, or drag it off the bar to remove it. "Remove from bar" sits in every popup's footer. × on a chip clears its value only.
- **Per-chip options.** Key: Exact / Harmonic / Fuzzy, ±1/±2 semitone, compare with Master or a deck. BPM: tolerance, half/double time. Tag groups: Any / All / Not. A chip's choice overrides the app setting and stays until changed.
- **Core and More.** "+" and the column header menu show core fields and the DJ's tag groups, with the rest under "More ▸", grouped. The default bar is Genre, BPM, Key and the tag groups marked "Show on bar". Grouping leaves the default bar. The same bar is used for every list.
- **Finder.** Typing in the search bar suggests values from every field ("Mood · Dark 41"). Return searches the words; ↓ then Return adds the value as a chip. The "+" menu has the same finder.

## Capabilities

### Modified Capabilities

- `track-browser`: filter bar

## Out of Scope

- A tag drawer with one-click pills per group (a later option).
- Saved bar setups.
- Column panes.
