## Getting started

openDJ's own mapping for one Xone:K2 playing two decks. It picks itself when
the K2 is plugged in; Mixxx's K2 mapping (**Allen&Heath Xone K1/K2/K3**) is
still in **Settings › Controllers** for anyone who wants it.

The K2 needs its factory settings, which most are already on:

- **Latching Layers off.** openDJ lights the buttons and does the SHIFT
  layer itself. To check: hold the bottom right encoder (SETUP) while
  plugging the K2 in, press the second matrix button (LATCHING LAYERS), turn
  the encoder until no button is lit, and press the encoder to store.
- **MIDI channel 15**, the factory channel.

The bottom right button is **SHIFT**. The left half is deck 1 and the right
half deck 2, mirrored:

| Where | Does | With SHIFT |
|---|---|---|
| Top encoders, outer | Filter (press: back to the middle) | Headphones level (left), master level (right) |
| Top encoders, inner | Gain; while a loop plays, its size. Press: 4-beat loop on or off | Move the loop; press: reloop |
| Outer knobs | The deck's FX 1, 2, 3; ON under each | Next effect |
| Inner knobs | Treble, mid, bass; kills under them | – |
| Faders | Pitch (outer), volume (inner) | – |
| Bottom buttons, rows 1-2 | Hot cues 1 2 / 3 4 (red when set) | Delete the hot cue |
| Bottom buttons, row 3 | Beat jump back, forward 32 beats (amber) | 16 beats |
| Bottom buttons, row 4 | SYNC outer (red when on), PLAY inner | Tempo reset; back to the start |
| Bottom encoders | Browse; press to load the side's deck, hold to open a folder | Turn: moves the side's deck through the track, silently (about 0.5% of the track a click); press: next pane (left), expand the browser (right) |

PLAY is green while the deck plays and blinks green while a track is loaded
and paused. The kill buttons and the light under the gain encoder are the
deck's level meter, bottom up: green above 35% and 50%, amber above 75%, red
above 95%. A kill that's on stays red.

Every button, encoder and light is an openDJscript line, so any of them can
be changed in the Controller Editor: **Mapping › Duplicate** makes your own
copy to edit.

## When you try it

- Check Latching Layers is off: the bottom buttons should stay dark until a
  track is loaded.
- Load a track on each deck with the bottom encoders and play both.
- Turn every encoder both ways, with and without SHIFT, and check nothing
  jumps.
- Set, play and delete hot cues; jump back and forward.
- Move every fader and knob, and turn each FX on and off.
