## Getting started

The XDJ-AZ is an all-in-one system, so openDJ drives it in its **PC
control** (MIDI) mode:

1. Connect the XDJ-AZ to the computer with a USB cable and switch its decks
   to PC control (**BROWSE**, then **MIDI**, then **LOAD**, as for
   rekordbox).
2. In **Settings › Audio**, choose the XDJ-AZ as the sound card.
3. openDJ picks this mapping by itself.

Use the **4-Deck** layout to see all four decks on screen.

This mapping is openDJ's own, written from AlphaTheta's list of the
XDJ-AZ's MIDI messages and built on the [XDJ-RX3 mapping](pioneer-xdj-rx3.md),
so it works the same way where they share controls.

- **DECK SELECT** picks the deck each side plays; each deck sends on its own
  MIDI channel, so the mixer follows.
- **Pads**: HOT CUE, BEAT LOOP and BEAT JUMP open those pad pages, and
  SLIP LOOP the loop rolls. SHIFT and a pad mode button (GATE CUE, BEAT
  LOOP2, KEY SHIFT, BEAT JUMP2) opens openDJ's PAD FX, SAMPLER, KEYBOARD
  (press it again for stems) and KEY SHIFT pages. The pads play whatever
  page the deck shows on screen.
- **BEAT JUMP ◀ ▶** jump by the deck's beat jump size (SHIFT: halve and
  double it). **4 BEAT** and **8 BEAT** set a loop. **KEY SYNC** matches the
  key of the other deck and **TEMPO RESET** puts the tempo back to 0.
- **CROSSFADER ASSIGN** puts each channel on side A, THRU or B.
- **BEAT FX** runs openDJ's Beat FX: **EFFECT SELECT** picks the nearest
  openDJ effect, **BEAT ◀ ▶** halve and double its length, **TAP** taps its
  tempo (**AUTO/TAP**: back to the track's), **LEVEL/DEPTH** sets the amount
  and **CH SELECT** sends it to one channel, the decks on crossfader side A
  or B, or, on MASTER, all four.
- **COLOR FX**: SPACE, DUB ECHO, NOISE, CRUSH and FILTER pick what the COLOR
  knobs play; SWEEP plays the filter.
- **MASTER** makes the deck the one the others sync to. **JOG MODE** swaps
  between scratching and bending.
- **SEARCH ◀◀ ▶▶** jump a beat at a time while held; **TRACK SEARCH** jumps
  32 beats; **CUE/LOOP CALL** halves and doubles a loop; holding **LOOP IN**
  lets the jog move the loop start.
- The jog displays show the playhead going round and where the cue point
  sits.
- **ROTARY SELECTOR** moves through the list (SHIFT: zooms the waveforms),
  **BACK** moves between the list and the sidebar and **TAG TRACK** adds the
  track to Automix.

Not mapped yet: CUE/LOOP MEMORY and DELETE, BEAT FX TIME, X-PAD, FX
QUANTIZE, FREQUENCY, PARAMETER, MASTER CUE, LINK CUE, master and mic EQ, the
mics, TALKOVER, INPUT SELECT and TRACK FILTER. The touch screen keeps
showing the XDJ-AZ's own PC-mode screen.

## When you try it

- Unlike the XDJ-RX3, the XDJ-AZ has no MIDI message that hands its mixer
  to the computer. Check whether its mixer still mixes what openDJ sends. If
  it does, the EQs and faders act twice: tell us, and use openDJ's deck
  outputs to send each deck to its own XDJ-AZ channel instead.
- Load a track on each deck with **LOAD**, switch each side between its two
  decks, and check PLAY, CUE, the pads and the jog wheel follow.
- Move every mixer knob and fader on all four channels, and the crossfader
  assign switches.
- Try every pad mode, with and without SHIFT, on pads 1-4 and 5-8, and check
  the pad colours.
- Turn on the Beat FX, turn **EFFECT SELECT** and **CH SELECT** through
  every position.
- Watch the jog displays and the LOAD lights while a track plays.
