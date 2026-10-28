# Decks and mixer

Drag a track from the library onto a deck to load it.

## Decks

Each deck has:

- **PLAY** and **CUE**
- **SYNC** and a tempo fader, with key lock and key shift
- **Hot cues**, **loops**, **beat jumps** and the **sampler** on the pads
- Three **effect** slots, beat-synced, with echo, reverb, filter, flanger and more

## Sync and quantize

**SYNC** sits over the tempo fader. It stays on while it's lit: the deck
follows the **master** deck's tempo and keeps its beats in step, at half or
double tempo when that's closer. Press it again to let go. **Shift-click**
SYNC to match the tempo and beat once, without staying synced, as SYNC does
in VirtualDJ.

The deck that synced decks follow shows a lit **MASTER** in its header.
openDJ picks it by itself, the deck that has been playing longest, and hands
over when it stops. Click **MASTER** on a deck to keep that deck the master
(it fills in), and click it again to let openDJ pick.

The **Q** button under SYNC is **quantize**. While it's lit, cues and loops
are set on the beat, and play, CUE, hot cues and jumps land in step with the
other playing deck. The arrow beside it sets the step: **1/16**, **1/8**,
**1/4**, **1/2** or **1** beat, or **4** to land on the same beat of the bar.
Cues and loops snap to a beat at most. **Settings → Decks → Quantize** sets
whether Q starts on for every deck.

## Sampler

The **SAMPLER** pad tab plays short sounds over the mix: drops, horns,
rewinds, name tags. There are 16 pads, eight at a time: the **1** and **2**
on the outside of the pads switch between them. On a DDJ-FLX4 the pads in
SAMPLER mode play the same eight the screen shows.

- **Sample packs**: **SAMPLE PACK** under the pads picks a pack. openDJ comes
  with the **Default DJ Sample Pack**; **+** makes a new, empty one.
- **Your own sounds**: drop an audio file (WAV, AIFF, FLAC, MP3, M4A or OGG,
  up to a minute) from Finder or the track list on a pad, or right-click a
  pad and choose **Load File…**. **Choose Sample…** borrows a sound from any
  pack, with a play button to hear it in the headphones first.
- The **pencil** shows a level knob and a **•••** menu on each pad. The menu
  (or a right-click) renames the pad, sets how it plays (**One shot**,
  **Hold**, **On/Off** or **Loop**), puts back the pack's own sound, or
  clears it. Drag a pad onto another to swap them. Shift-click a pad to stop
  it.

What you change on a pad is kept for that pack, and the pack's own files are
left alone.

## Keys

The padlock over the triangle beside the tempo fader is **key lock**: while
it's lit, moving the tempo fader keeps the track's key. The **+** and **−**
beside the deck's key shift it a semitone at a time.

The library shows in lilac the keys that mix with the track playing now.
**Settings → Decks → Key mixing** picks which ones count:

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
don't carry over. In **Settings > Decks**, under **Auto reset on load**, each
of these can be switched off (all are on to start):

- **Effects**: the deck's effects switch off. Their beats and dry/wet stay as
  you set them. An effect that also plays on the main mix or on another
  playing deck, such as a shared Beat FX, keeps going.
- **EQ and filter**: EQ, kills, filter and sculpt go back to the middle.
- **Gain**: the gain knob goes back to 0 dB (or the track's own gain, if its
  info sets one). Auto gain still levels each track.

## Mixer

Between the decks, each channel has gain, a 3-band EQ, a filter and a channel
fader. Below them are the crossfader and the headphone cue buttons.
