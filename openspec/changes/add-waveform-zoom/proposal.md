# Proposal

Planned for: **v0.7 · Your library, everywhere** (from a user request, issue #566, 2026-11-10)

## Why

The large scrolling waveforms always show ten seconds across. Mixing a long blend wants more of the track in view; setting a beat grid or lining up a kick wants much less. Prep has its own −/+ zoom in bars, so moving between layouts changes what you see. Other DJ apps zoom with the mouse wheel; one with many tiny steps feels loose, while a few fixed steps let a DJ learn the zoom they like ("70%").

## What Changes

- One zoom for every large scrolling waveform in every layout (2-Deck, 2-Deck Pro, 4-Deck and Prep), so beats sit the same distance apart on every deck and the zoom stays when the layout changes. Prep's bar zoom goes; its −/+ step the shared zoom.
- Ten steps, shown as 10% to 100%: each step is 1.5 times tighter, from about 34 s across to under a second; 40% is today's ten seconds and the start.
- The zoom glides to each step over 150 ms, the waveform stretching as it goes; the shader fades between its detail levels so the outline doesn't jump.
- Inputs: the mouse wheel or trackpad over a large waveform (a wheel notch is a step; a trackpad swipe builds up to one), a slim 10-notch bar at the waveform's left edge while the pointer is over it, ⌘= / ⌘− / ⌘0, View › Zoom Waveforms In/Out and Reset Waveform Zoom, the waveform's right-click menu, and Mixxx's `waveform_zoom`, `waveform_zoom_up`, `waveform_zoom_down` and `waveform_zoom_set_default` on each deck group for controllers.
- The step shows for a moment in the waveform's corner after each change, and is saved.
- Deck overviews and mini waveforms keep showing the whole track. Live decks (inputs, instruments, Groove Box) keep ten seconds across: their traces keep about that much.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `waveforms`: shared zoom for the large scrolling waveforms

## Out of Scope

- A zoom per deck.
- Zooming live decks' traces, note rolls and step lanes.
- A default controller gesture for zoom on the bundled mappings.

## Difficulty

**Small: one PR.** The renderer already takes the seconds across as a parameter each frame; the zoom replaces a constant, Prep's bar zoom goes, and the shader gains a cross-fade between detail levels.

## Performance

Nothing is uploaded again on a zoom change. While gliding the waveforms redraw every frame for 150 ms, as when a deck plays. The level cross-fade samples a second level only in the last quarter before a level change.
