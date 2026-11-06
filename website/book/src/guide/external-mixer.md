# External mixer

With an external mixer (a Pioneer DJM, Allen & Heath Xone, Rane and the
like), the hardware does the EQ, faders, crossfader and headphone cue. openDJ
only plays the decks, each into its own mixer channel. This is also how
timecode vinyl rigs are wired.

You need an audio interface with an output pair for each deck: many mixers
have one built in (USB on the mixer itself), or use a 4- or 8-output
interface.

## Set it up

1. Open **Settings › Audio**.
2. Under the outputs, press **+** and pick **Deck 1**, then **+** again for
   **Deck 2** (and Deck 3 and 4 for a four-channel mixer). Give each its own
   channels on the interface, for example Deck 1 on **1 & 2** and Deck 2 on
   **3 & 4**.
3. Remove the Master output if it would land on the same channels, or move it
   to the computer's own output.
4. Set **Mixing** to **External mixer**.
5. Wire each output pair to a line input of the mixer: Deck 1 to channel 1,
   Deck 2 to channel 2.

## What changes

- Each deck goes out at full level, after its deck effects, pads, loops,
  stems and sync. openDJ's EQ, filter, channel fader, gain knob, crossfader
  and headphone cue are left out; auto gain still levels the tracks.
- The mixer in the middle of 2-Deck, and the channel controls of each 4-Deck
  strip, give way to each deck's level meter.
- Master and booth are silent. The headphones only play the browser's preview
  player.
- Controllers keep working. EQ, fader and crossfader moves they send do
  nothing, so a deck-only controller pairs best.
- Automix and the master limiter are off: they need openDJ's mixer.
- The mic and Auxiliary inputs aren't mixed into anything (the mixer has its
  own). The sampler plays on its own output if it has one, else on Deck 1's.
- Recording and going on air take the **Auxiliary** input: plug the mixer's
  record (or booth) out into the interface's input and pick it as Auxiliary
  under the inputs.

Set **Mixing** back to **openDJ** to mix on screen again.
