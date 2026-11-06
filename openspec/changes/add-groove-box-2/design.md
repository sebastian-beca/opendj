# Design

Builds on `add-groove-box`. The research on other apps is in that change's design.md and the project notes (`notes/pattern-player-review.md`). This file keeps what is new for 2.0.

## Principles kept from 1.0

- One press still plays: a kit loads with a groove, PLAY starts on the next bar, no tempo or clock to set.
- The face stays small. New detail (accent, chance, nudge, rolls, per-row length) lives in hold and right-click menus, and the timeline does nothing until something is dragged onto it.
- A Groove Box is still a deck: its channel, FX, cue and meters are untouched.

## The waveform strip becomes the timeline

A track deck draws its scrolling waveform across the deck. A Groove Box has no file to analyse, so the strip is free. It shows:

```
 bar  1        2        3        4        5        6        7        8
     ┌──────── A ────────┬─ A ──┬─ B ──┬──────── B ────────┬─ FILL ─┐
 KCK |▮   ▮   ▮   ▮   ▮  |▮   ▮ |▮   ▮ |▮   ▮   ▮   ▮   ▮  |▮ ▮▮▮▮ |
 HAT |  ▮   ▮   ▮   ▮    |  ▮   |▮▮▮▮▮▮|▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮|▮▮▮▮▮▮ |
     └───────────────────┴──────┴──────┴───────────────────┴────────┘
                    ▲ playhead, same position and zoom as the waveforms
```

- Same zoom and same playhead line as the other decks' waveforms, so a bar on the groove sits under the same bar of the leader: the groove's bars are the leader's bars.
- Blocks are grooves; ticks are hits (one thin lane per row, accent as height). The leader's phrase markers are drawn faintly behind, so "B for 4 bars, then fill" can be lined up with the next drop by eye.
- Empty, it shows the selected groove repeating; dragging A to H onto it builds a chain, click a block to jump there on the next bar, right-click to repeat, remove or set a fill.
- The 4-Deck strips have no room for it; they show the current groove letter and bars left in the chain.

Alternatives considered for the strip: more rows (the grid area grows instead, see below) and the per-sound knobs (they fit in the right column). The timeline is the one thing the strip's width and its alignment with the other waveforms is uniquely good for.

## Grid and rows

- 8 rows on screen in 2-Deck when the window is tall enough, else two 4-row views with a 1-4 / 5-8 switch (the same shifter idea as the pads).
- Each row: name, steps, M, S, and its length shown as the last lit step; dragging the row's end handle sets its length (1-64), so a 3-step hat runs against a 16-step kick.
- Step size per row (1/16, 1/8, 1/16T) from the row menu.
- 64 steps = 4 pages of 16; the page follows the playhead unless pinned.

## Step detail

| Detail | Values | How |
|---|---|---|
| Accent | soft, medium, hard | Hold a step and turn LEVEL, or right-click; drawn as fill brightness |
| Chance | 100, 75, 50, 25 % | Right-click |
| Nudge | −50 % to +50 % of a step | Hold and turn SWING, or right-click |
| Roll | 1, 2, 3, 4 hits | Right-click; drawn as split cell |

Engine: a step is a small packed value (on, accent 2 bits, chance 2 bits, nudge as i8, roll 2 bits), so a 16-row × 64-step groove stays a fixed-size value sent whole on each edit, as in 1.0.

## Sound knobs

LEVEL, PITCH, DECAY plus one named knob per row from `pack.json` (`"knob": {"name": "TONE", "param": "tone"}`). For the synthesised kits the parameter goes into the voice (kick sweep depth, snare noise mix "SNAPPY", hat band-pass "COLOR"); for sample rows it is a per-row low-pass. Knobs act on the selected row, as in 1.0; a controller with 16 encoders (Twister) gets one knob per row and parameter through `[GrooveN],rowR_knobK`.

## Fills and variation

- A groove can have a fill (a second step set). FILL plays it once on the last bar before the next phrase of the leading deck, or on the next bar when there are no phrases. Hold FILL for a manual fill that ends when released.
- Variation: a groove can hold A/B versions; the switch alternates them every bar or every 2 bars.
- Built-in grooves gain fills, so FILL works out of the box.

## Melodic row

- One optional row of kind `melodic`: per step a note (scale degree and octave), length and slide.
- Notes are scale degrees, played in the key of the leading deck (from key detection), following its key changes on the bar. A fixed key can be set instead.
- Sound: a small mono synth in the engine (saw/square, filter, envelope, two knobs on the face: CUTOFF and RESONANCE as the row's knobs) or a hosted instrument plugin from `add-instrument-plugins`, which gets the notes with sample offsets.
- Editing: click a step to toggle, drag up and down for the degree; no piano roll.

## Record from pads

- REC arms recording into the selected groove; pad hits on the sampler or the Steps page land on the nearest step (or with nudge kept, an option), with accent from pad velocity.
- One bar count-in when the groove is stopped. Undo removes the last pass.

## Sounds from the mix

- Drop a file, a sampler pad, or a region selected on a track deck's waveform onto a row: it becomes the row's sound, trimmed to its first transient and saved as an override of the kit.
- With stems loaded, the row menu offers "Kick from deck 1" (and snare, hat): the engine finds the cleanest hit in the drum stem (onset with least overlap, by energy in the row's band) and cuts it.

## Replace drums

From the DDJ-GRV6's Groove Circuit idea (task 5.6 of `add-groove-box`):

- On a track deck with stems, "Replace drums" opens a Groove Box over the deck instead of in place of it. The drum stem is muted (and faded back on release), the groove plays in the track's grid on the track's channel.
- Drum roll and stutter act on the groove; releasing returns to the track's own drums on the next beat.
- This needs a deck that has both a track and a groove; the engine mixes the groove into the deck's buffer after the stems and before EQ.

## Kits and items

`pack.json` grows: up to 16 `rows`, per row `knob` and `kind` (`drum` or `melodic`), grooves with `fill` and `variation`, and `chains`. Users' grooves and chains are saved in the kit's overrides and can be saved as items shown in the Instruments source, dragged onto a Groove Box, and exported with the kit.

## Controllers

New `[GrooveN]` controls: `rowR_accentS`, `rowR_length`, `rowR_knobK`, `fill`, `variation`, `rec`, `chain_position`, `page` (0-3). The Steps pad page gets accent by velocity, rows 1-8 on Shift, and a two-row mode on 16-pad controllers (two rows of 8, or one row of 16).

## Order of work

1. Timeline in the waveform strip and chains (uses the free space, no engine changes beyond chain playback).
2. Step detail and per-row length (engine step format, menus).
3. More rows, sound knobs, kit format.
4. Fills and variation.
5. Record from pads.
6. Melodic row (after `add-instrument-plugins` for the plugin route; the built-in synth can come first).
7. Sounds from the mix and replace drums (need stems on a deck alongside a groove).

## Open questions

- Is the timeline the right use for the waveform strip, or should the grid itself grow into it?
- Melodic row: key-following by default, or a fixed key unless chosen?
- Eight rows on screen, or keep four with a row switch for beginners and eight in an "Advanced" view?
- Should 2.0 wait for v2.0, or ship in pieces (timeline and step detail could land in v0.6 or v0.7)?
