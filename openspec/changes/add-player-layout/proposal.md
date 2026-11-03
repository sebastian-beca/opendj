# Proposal

Planned for: **v0.5 · Stems, performance and switching over** (step 1 of 3; from a user request, 2026-11-03)

## Why

The Simple layout is still a DJ mixer, only smaller: two deck headers, small jogs and a 2 × 2 EQ in a 108 px row. Someone who just wants music playing (a party, a shop, a dinner) has no screen that works like a music player. Other DJ apps don't fill this gap either. djay Pro's One Deck view loads the next song with no mix, VirtualDJ's Starter and Essentials layouts and Serato's Practice mode are still decks and a mixer, and Mixxx's long-standing "Mixxx-Mini" request was stuck because one deck alone cannot crossfade. The phone remote already shows the right model: one now-playing track over Automix, with a queue.

## What Changes

- **Player** replaces Simple as layout 1 in the layout menu. It shows one bar: cover, title, artist, BPM and key; Order, Previous, Play/Pause and Next over the track overview (click or drag to seek, Automix's mix-out zone shaded); what Automix does next; the master level; and a queue button.
- Underneath, decks 1 and 2 keep alternating and Automix mixes between them. The bar always shows the deck you hear. Play switches Automix on, Next mixes into the next track now, Previous goes back to the start of the track, and Order opens the Automix Order menu.
- The queue button shows or hides Up Next (the side panel's Automix list).
- WAVEFORMS, the FX bar and their top-bar buttons are hidden in Player.
- The compact row (deck headers and a small mixer) stays for the expanded library in 2-Deck and 4-Deck.

Later steps: a search box in the centre of the top bar with Play next, Add to queue and Play now (step 2), and a full-window Now Playing view with large cover art and Up Next restyled with covers (step 3).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `app-layout`: Simple becomes Player.

## Out of Scope

- A mini player in its own window.
- Previous going back to the last played track from History.

## Impact

`ui/player.slint` (new `PlayerBar`), `ui/main.slint` (layout 1, top bar), `src/automix.rs` (queued count, live deck and the deck being mixed into for the bar), user guide `guide/layouts.md`. The stored `layout` value 1 keeps meaning layout 1, so a saved Simple opens as Player.
