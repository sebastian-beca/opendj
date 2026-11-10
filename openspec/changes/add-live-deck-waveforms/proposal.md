# Proposal

Planned for: **v0.6**, with the Instruments rework (`rework-instruments`)

## Why

A deck can play a live source instead of a track: the mic or an aux input, an instrument plugin, or the Groove Box. Such a deck has no track, so its scrolling waveform is empty, and nothing on screen shows what it plays or how it sits against the mix. Tracks get a waveform with a beat grid that lines up with the other decks; a live part should get the same.

## What Changes

- **Live trace.** On a deck playing an input, an instrument or the Groove Box, the scrolling waveform draws the deck's own audio as it plays, in the usual band colours, at the play line, scrolling away to the left. Nothing is drawn to the right of the play line. The audio is read after the deck's trim, before its EQ.
- **Leading deck's grid.** The live lane draws the beat grid and bar numbers of the leading deck (the sync leader, else the first playing deck with a tempo), at that deck's playing tempo, so the live part lines up with the track it plays against.
- **Note roll** on instrument decks: the notes played as bars on a piano roll, fitted to the range of notes in view, rows of the current scale tinted, held notes lit at the play line; a faint trace under it.
- **Step lanes** on Groove Box decks: eight lanes with the hits played behind the play line and the coming hits ahead of it, fills marked, and the bar where the next groove starts. Track names only where the lane has room (2-Deck).
- **Macro lanes** as a second view on instrument decks: the knobs as automation lines at full height, a colour each, only the knobs that moved drawn, behind a small switch on the waveform; 2-Deck only. Groove Box decks keep their step lanes.

## Out of scope

- A spectrogram view.
- Keeping the last bars of a live deck to drop on another deck as a loop.
- Phrases on the waveforms (its own change).

## Capabilities

### Modified Capabilities

- `waveforms`: the scrolling waveform of a deck playing a live source.
