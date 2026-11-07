# Proposal

Planned for: **v0.6 · Buy it from the artist** (2026-11-07)

## Why

The Groove Box plays and looks right, but it is still a fixed loop: it can only follow the mix's tempo, its patterns never change unless edited by hand, it has no fills, its ninth track doesn't fit 8- and 16-pad controllers, and its kick is in no key. DJs want a beat that sits under a track, breathes, and builds into the next phrase, from the screen or from a pad controller.

## What Changes

- **Own tempo**: follows the mix by default; `[GrooveN],follow` off keeps its own `bpm`; `tempo_scale` plays at half or double the mix's tempo. The top bar's tempo button opens Follow the mix, ±1 and ±0.1 BPM, Half time / Normal / Double and SWING.
- **Kick in key**: `[GrooveN],kick_key` (Off, Root, 5th) tunes the kick to the nearest root or fifth of the leading deck's key, at most half an octave from its natural pitch (measured from each kit's kick); `kick_note` shows the note.
- **Eight tracks**: crash and ride share track 8, Cymbal, picked per groove (no library groove uses both); saved Groove Boxes move track 9 into track 8.
- **Dice** per track or for the groove: New (a groove from the genre's step-to-step habits), Mutate (moves a hit or two, keeps the kick on 1 and the snare's backbeat), Busier and Calmer (one hit more or fewer, by step weight: downbeats first, as in Grids, and how often the genre hits the step), Even out (Euclid), Shift later/earlier; track locks and undo; all start on the next bar. `[GrooveN],dice`, `dice_row`, `dice_pending`, `dice_undo`, `can_undo`, `rowR_lock`.
- **Living grooves**: steps that play half the time (right-click cycle, `?` in groove files) and a VARY knob (weak hits drop out, ghost notes where the genre puts them, loudness wavers; the backbone always plays); Living house and Rolling breaks in Basics.
- **Fills**: Auto 1 (last beat of every 4th bar, last two beats of every 8th, the whole 16th) and Auto 2 (the same at 2, 4 and 8 bars), on the leading deck's phrases; a Remix strip whose position picks a fill level (hat rolls, rolls over two or three sounds and swapped tracks, 16th rolls over all), from the mouse or a touch strip; letting go returns on the next step.
- **4x4 view**: the Sequencer as a 4x4 pad grid in the same order as a 16-pad controller (step 1 bottom left).

## Capabilities

### New Capabilities

- `groove-box-play`: tempo, key, variation and fills for the Groove Box

### Modified Capabilities

None.

## Notes

Takes over from add-groove-box-2: tasks 2.2 (chance part), 2.5 Euclid, 2.6 Dice and Remix, 4.1 FILL and Auto fill, and replaces 3.1 (nine rows) with eight tracks. The fill and Remix behaviour follows Sugar Bytes DrumComputer's Auto Fill and Remix as its manual describes them; nothing is copied from its code.
