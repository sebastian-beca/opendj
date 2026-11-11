# Proposal

Planned for: **v0.7 · Your library, everywhere** (from a user request, issue #637, 2026-11-11)

## Why

The large scrolling waveforms have one fixed height per layout. On a big screen they could be much taller, which makes beats, phrases, grid handles and live decks' lanes easier to read; on a laptop the library needs the room. Other DJ apps barely help: Serato has a two-step toggle, VirtualDJ only skins with a split panel, and rekordbox, djay Pro and Engine DJ nothing, with requests going back years.

## What Changes

- Four heights for the large waveforms: Small, Medium, Large and Extra large. Medium is today's height. Each waveform is 48, 72, 104 or 144 px in 2-Deck and 2-Deck Pro, 36, 52, 68 or 84 px in 4-Deck, and 120, 176, 240 or 320 px in Prep.
- Each layout keeps its own height, saved.
- The library keeps room for about five tracks: a height that doesn't fit shows the next smaller one that does, never below Medium unless the DJ chose less, and the chosen one comes back when the window has room. The deck rows never shrink.
- A grip on the line under the waveforms: dragging snaps to the four heights, with a tick per height and "Height: Large" in the middle while dragging; double-click goes back to Medium.
- A drop-down arrow on the right of the waveforms top-bar button, as on MIC, listing the four heights.
- A height row in a large waveform's right-click menu, View › Taller Waveforms / Shorter Waveforms / Reset Waveform Height, ⌥⌘↑ / ⌥⌘↓, and controller controls `[Skin],waveform_height` (1 to 4), `waveform_height_up`, `waveform_height_down` and `waveform_height_set_default`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `waveforms`: a choice of heights for the large scrolling waveforms

## Out of Scope

- Free heights between the four.
- Changing the deck rows' height.
- A default controller gesture on the bundled mappings.

## Difficulty

**Small: one PR.** The strip's height was a constant per layout; it becomes a table of four, and the renderer already sizes its textures from the strip.

## Performance

Nothing new per frame. The waveform textures are made again at the new size once per change, as on a window resize.
