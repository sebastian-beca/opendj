# Proposal

Planned for: **v0.7 · Your library, everywhere** (from a user request, issue #637, 2026-11-11)

## Why

The large scrolling waveforms have one fixed height per layout. On a big screen they could be much taller, which makes beats, phrases, grid handles and live decks' lanes easier to read; on a laptop the library needs the room. Other DJ apps barely help: Serato has a two-step toggle, VirtualDJ only skins with a split panel, and rekordbox, djay Pro and Engine DJ nothing, with requests going back years.

## What Changes

- Ten heights for the large waveforms, 1 to 10, with 3 today's height (first shipped as four: Small, Medium, Large, Extra large; ten after trying it). Each waveform goes from 48 to 144 px in 2-Deck and 2-Deck Pro, 36 to 84 px in 4-Deck, and 120 to 320 px in Prep.
- Each layout keeps its own height, saved.
- The library keeps room for about five tracks: a height that doesn't fit shows the next smaller one that does, never below 3 unless the DJ chose less, and the chosen one comes back when the window has room. The deck rows never shrink.
- A grip on the line under the waveforms: the waveforms stretch smoothly with the pointer and settle on the nearest height when it lets go, with the height's number in the middle; double-click goes back to 3.
- A drop-down arrow on the right of the waveforms top-bar button, as on MIC, with the ten numbers.
- − / + for the height in a large waveform's right-click menu, View › Taller Waveforms / Shorter Waveforms / Reset Waveform Height, ⌥⌘↑ / ⌥⌘↓, and controller controls `[Skin],waveform_height` (1 to 10), `waveform_height_up`, `waveform_height_down` and `waveform_height_set_default`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `waveforms`: a choice of heights for the large scrolling waveforms

## Out of Scope

- Changing the deck rows' height.
- A default controller gesture on the bundled mappings.

## Difficulty

**Small: one PR.** The strip's height was a constant per layout; it becomes a table of four, and the renderer already sizes its textures from the strip.

## Performance

Nothing new per frame. The waveform textures are made again at the new size once per change, as on a window resize.
