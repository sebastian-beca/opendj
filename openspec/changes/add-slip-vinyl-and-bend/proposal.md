# Proposal

Planned for: **v0.5 · Stems, performance and switching over**

## Why

Slip mode only brought the track back when slip was switched off, not when each trick ended, so it worked like a hold button that nobody expects (and the deck-transport spec already said otherwise). Nothing on screen reached slip, the pitch bend buttons or censor, and the DDJ-FLX4's jog top always scratched, with no CDJ mode. rekordbox, djay Pro, VirtualDJ, Serato and Traktor all have a SLIP button with a ghost playhead, a VINYL switch and bend buttons. From the deck performance review (Seba, 2026-11-01).

## What Changes

- Slip mode returns when each trick ends: letting go of the jog, ending a loop, letting go of a hot cue (which plays only while held), turning reverse off, or pressing play after a pause. Only a playing deck slips.
- A dashed lilac ghost playhead on both waveforms while slipping (`slip_position`).
- SLIP as a word in the top right corner of each deck's jog (lilac when on, light grey when off, 2-Deck only); vinyl mode as a switch in Settings › Decks.
- `vinyl_mode` per deck (on by default): the DDJ-FLX4 and DDJ-FLX6 scratch on jog touch only while it's on; off, the top bends like the rim.
- « and » pitch bend buttons in the jog's lower corners (2%, 4% with Shift), and ← → bend the deck started last.
- Shift-click and hold PLAY on a playing deck for censor.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `deck-transport`: slip per trick, ghost playhead, on-screen bend and censor
- `controllers`: vinyl mode follows Settings › Decks › Vinyl mode

## Out of Scope

- Slip for beat jumps, pad FX and the slicer (stays in add-slip-everywhere).
- Brake, backspin and soft start as settings; scratch latency (improve-scratching).

## Impact

engine deck slip state, waveform shader, deck.slint, main.slint keys, FLX4 and FLX6 scripts, user guide.
