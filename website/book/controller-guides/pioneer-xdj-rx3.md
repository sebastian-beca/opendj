## Getting started

The XDJ-RX3 is an all-in-one system, so openDJ drives it in its **PC
control** (MIDI) mode:

1. Connect the XDJ-RX3 to the computer with a USB cable and switch it to
   PC control (press **SOURCE**, then pick the computer, as for rekordbox).
2. In **Settings › Audio**, choose the XDJ-RX3 as the sound card, with the
   main output on channels 1-2 and the headphones on 3-4.
3. openDJ picks this mapping by itself. When it starts it switches the
   XDJ-RX3's mixer to MIDI control, so openDJ does the mixing; it switches
   it back when you quit.

Use the **4-Deck** layout to see all four decks on screen.

This mapping is openDJ's own, written from AlphaTheta's list of the
XDJ-RX3's MIDI messages and built on the [DDJ-FLX4 mapping](pioneer-ddj-flx4.md)
and the [DDJ-FLX6 mapping](pioneer-ddj-flx6.md), so it works the same way
where they share controls.

- **DECK 1/3** and **DECK 2/4** pick the deck each side plays; each deck
  sends on its own MIDI channel, so the mixer follows.
- **Pads**: HOT CUE, BEAT LOOP and BEAT JUMP open those pad pages, and
  SLIP LOOP the loop rolls. SHIFT and a pad mode button (the EXTENSION
  pages) opens PAD FX, SAMPLER, KEYBOARD (press it again for stems) and
  KEY SHIFT. The pads play whatever page the deck shows on screen.
- **BEAT FX** runs openDJ's Beat FX: **EFFECT SELECT** picks the nearest
  openDJ effect, **BEAT ◀ ▶** halve and double its length, **TAP** taps its
  tempo (SHIFT: back to the track's), **LEVEL/DEPTH** sets the amount and
  **CH SELECT** sends it to CH1 (decks 1 and 3), CH2 (decks 2 and 4) or, on
  MASTER, all four.
- **COLOR FX**: SPACE, DUB ECHO, NOISE, CRUSH and FILTER pick what the COLOR
  knobs play; SWEEP plays the filter.
- **MASTER** makes the deck the one the others sync to. **JOG MODE** swaps
  between scratching and bending.
- **TRACK SEARCH** jumps 32 beats; **CUE/LOOP CALL** halves and doubles a
  loop; holding **LOOP IN** lets the jog move the loop start.
- The jog displays show the playhead going round and where the cue point
  sits.
- **ROTARY SELECTOR** moves through the list (SHIFT: zooms the waveforms),
  **BACK** moves between the list and the sidebar and **TAG TRACK** adds the
  track to Automix.

Not mapped yet: CUE/LOOP MEMORY and DELETE, TIME MODE, BEAT FX TIME, X-PAD,
PARAMETER, MASTER CUE, LINK CUE, crossfader assign, AUX and TRACK FILTER.
The touch screen keeps showing the XDJ-RX3's own PC-mode screen.

## When you try it

- Check that the XDJ-RX3's mixer goes quiet on its own and that openDJ's
  master plays through it. If the mixer still mixes, tell us: it may need
  switching to MIDI control by hand.
- Load a track on each deck with **LOAD**, switch each side between its two
  decks, and check PLAY, CUE, the pads and the jog wheel follow.
- Move every mixer knob and fader, including on decks 3 and 4.
- Try every pad page, with and without SHIFT, and check the pad colours.
- Turn on the Beat FX, turn **EFFECT SELECT** and **CH SELECT** through
  every position.
- Watch the jog displays while a track plays.
