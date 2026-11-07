# Proposal

Planned for: **v0.6 · Buy it from the artist** (2026-11-07, from a user report of stray notes playing the instrument; issue #470)

## Why

Every MIDI port with no mapping running is opened as a keyboard, and all its channel messages, on every channel, go to the Instrument input. Nothing shows which devices those are. A keyboard's DAW or Mackie Control port sends its transport buttons as notes, the MIDI side of an HID controller (the Maschine Mikro MK3's) sends pad notes, and either can play the instrument on another deck without the DJ knowing why. Instrument devices also can't be tied to the deck they play: a pad controller or keyboard should play the deck holding its instrument, and move with it when the instrument is dragged to another deck.

## What Changes

- **DJ controllers never play instruments.** A device running a mapping never does; neither do DAW and Mackie Control ports, the computer's own ports, or the MIDI side of an HID controller running its HID mapping, unless the DJ picks it.
- **A role for each device** in Settings › Controllers: the mapping menu gains **Play instruments** (no mapping, its notes play the instrument), beside Automatic, the mappings and Off.
- **A Deck menu for each device:** Auto, **As mapped** (the decks its mapping names), **Follow instrument** (the deck holding the instrument plugin, else the Groove Box; it moves when the instrument moves) and **Deck 1** to **Deck 4**. Auto follows for devices that play instruments and is as mapped for the rest. A keyboard pinned to a deck only plays the instrument while it is on that deck.
- **What plays it:** the instrument deck says which keyboard played it last, and a notice says so the first time a keyboard plays it.

## Capabilities

### Modified Capabilities

- `controllers`: devices that play instruments, and the deck each device plays

## Out of Scope

- A per-instrument-deck "MIDI from" device and channel pick: with the Instruments rework (`rework-instruments`, issue #465), once more than one instrument deck exists.
- Passing a mapped device's unmapped notes to the instrument ("Both").
