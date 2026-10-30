# Decks and mixer

Drag a track from the browser onto a deck to load it, or select it and
press a controller's **LOAD** button. You can also drag an audio file
straight from Finder, Explorer or your file manager onto a deck: it loads
there and joins your library. (On Linux this needs an X11 session; Wayland
doesn't pass file drops to openDJ yet.)

## Decks

In 2-Deck each deck runs, from the outer edge in:

- **The tempo column**: **SYNC** on top, the tempo fader, and **−** and **+**
  under it for small steps. The triangle beside the fader puts the tempo
  back to the track's own, and the padlock above it is
  [key lock](#keys).
- **The pads**, with their tabs on top (see [Pads](pads.md)).
- **The LOOP column** (see [Looping](looping.md)).
- **The jog wheel**, with **CUE** and **PLAY** under it, next to the mixer.
  Drag the jog to scratch or nudge.

Above them, the deck's header shows the cover, title and artist, the time
left and the track's tempo and key, with the overview waveform of the whole
track. Click the overview to jump; dragging it moves without playing what it
passes.

**PLAY**, **CUE** and **SYNC** show an outline in the deck's colour while
they're on. **Shift-click** PLAY to go back to the start of the track.

The decks' effects are in the [FX bar](fx.md#the-fx-bar) under them.

## Sync and quantize

**SYNC** sits over the tempo fader. It stays on while it's lit: the deck
follows the **master** deck's tempo and keeps its beats in step, at half or
double tempo when that's closer. Press it again to let go. **Shift-click**
SYNC to match the tempo and beat once, without staying synced, as SYNC does
in VirtualDJ.

The deck that synced decks follow shows a small **MASTER** under its key.
openDJ picks it by itself, the deck that has been playing longest, and hands
over when it stops. Click **MASTER** on a deck to keep that deck the master
(it fills in), and click it again to let openDJ pick.

**Right-click SYNC** for **quantize**. While it's on, cues and loops are set
on the beat, and play, CUE, hot cues and jumps land in step with the other
playing deck. The same menu sets the step: **1/16**, **1/8**, **1/4**,
**1/2** or **1** beat, or **4** to land on the same beat of the bar. Cues and
loops snap to a beat at most. **Settings › Decks › Quantize** sets whether
quantize starts on for every deck.

## Keys

The padlock over the triangle beside the tempo fader is **key lock**: while
it's lit, moving the tempo fader keeps the track's key. The **+** and **−**
beside the deck's key shift it a semitone at a time, and clicking the key
lists every shift with the key it gives.

The track list shows in lilac the keys that mix with the track playing now.
**Settings › Decks › Key mixing** picks which ones count:

- **Harmonic** (4 keys): the same Camelot number, or the next number up or
  down with the same letter. 8A mixes with 8A, 8B, 7A and 9A.
- **Fuzzy** (6 keys): the letter doesn't matter, so 7B and 9B mix with 8A
  too. This is [fuzzy keymixing](https://www.digitaldjtips.com/fuzzy-keymixing-the-new-way-to-dj/).

A key one semitone from mixing gets a small **+1** or **−1** in a dimmer
lilac. With **Match key on load** (Off, ±1 or ±2 semitones, ±1 by default), a
track loaded while another deck plays is shifted by that much, with key lock
on, so it mixes. At ±2, keys two semitones away are marked **+2** or **−2**
in grey.

## Auto reset on load

Loading a track puts its deck back to normal, so the last track's settings
don't carry over. In **Settings › Decks**, under **Auto reset on load**, each
of these can be switched off (all are on to start):

- **Effects**: the deck's effects switch off. Their beats and dry/wet stay as
  you set them. An effect that also plays on the main mix or on another
  playing deck, such as a shared Beat FX, keeps going.
- **EQ and filter**: EQ, kills, filter and sculpt go back to the middle.
- **Gain**: the gain knob goes back to 0 dB (or the track's own gain, if its
  info sets one). Auto gain still levels each track.

## Mixer

Between the decks, each channel has four knobs down its outer side, its
gain, and a channel fader beside the main level meter. Under the faders are
the headphone **CUE** buttons and the crossfader.

The menu over the meters picks what the four knobs do:

- **EQ**: high, mid and low, and the [Color FX](fx.md#color-fx) filter knob,
  with a small **▾** beside it to pick the Color FX.
- **STEMS**: the level of each [stem](stems.md).
- **SCULPT**: a low-pass, a sculpt band with its frequency and cut or boost,
  and a high-pass, after PlayDifferently's Model 1.

A controller's EQ knobs follow the same choice.

**The crossfader** can be switched off, so it no longer fades either side:
right-click it, or use **Settings › Mixer**.

## Master, phones and mic

The **MASTER** button in the top bar opens the output levels: **Master**,
**Phones**, **Booth**, and **Cue / Mix**, which blends cue and main in the
headphones. Its **Ableton Link** switch is described in
[Ableton Link](link.md). **Audio Settings…** opens the outputs in Settings.
Levels are kept between runs.

The **MIC** button opens the microphone and the auxiliary input, each with
**ON** and a level. The microphone has effects of its own under **MIC FX**,
which the decks' effects never touch, and **AUX** sends the auxiliary input
through them too. Choose the inputs in **Settings › Audio**. The auxiliary
input starts off.
