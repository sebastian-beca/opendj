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
- **The LOOP column** (see [Looping](looping.md)), with **SLIP** and
  **VINYL** at the bottom (see [Slip, bend and censor](#slip-bend-and-censor)).
- **The jog wheel**, with **CUE** and **PLAY** under it, next to the mixer.
  Drag round its black rim to bend the tempo, scroll over it to nudge, and
  hold **«** or **»** in its lower corners to bend slower or faster.

Above them, the deck's header shows the cover, title and artist, the time
left and the track's tempo and key, with the overview waveform of the whole
track. Click the overview to jump; dragging it moves without playing what it
passes.

### Beat counter

Beside the title, four squares light up one beat at a time, so you can see
where the bar is: the first beat of each bar lights white, the others in the
deck's color. The thin bars under them fill up through each group of four
bars, and all four turn lilac in the last bar before a new phrase starts,
so you know the change is one bar away.

Next to the squares, the count reads **phrase.bar.beat** (for example
**3.2.1**: phrase 3, bar 2, beat 1), and under it **↓ 6 › DROP** says the
next phrase, a drop, starts in 6 bars. The phrases come from the track's
[phrase analysis](music.md#phrases); a track without phrases is counted in phrases of
eight bars from its first bar. If beat 1 doesn't land on the downbeat, the
beat grid is off: click the tempo on the jog and choose **Bar 1 here** on a
downbeat (see [Beat grids](music.md)).

Choose **Off**, **Beats** (squares only) or **Beats and phrases** under
**Settings › Decks › Beat counter**. The 4-Deck strips show the squares
only.

**PLAY**, **CUE** and **SYNC** show an outline in the deck's color while
they're on. **Shift-click** and hold PLAY while the deck plays for
[censor](#slip-bend-and-censor).

The decks' effects are in the [FX bar](fx.md#the-fx-bar) under them.

### Instruments on a deck

A deck can play something live instead of a track. Click **Instruments** in
the rail at the far left (the keyboard icon) and drag one onto a deck:

- A **Groove Box** kit: a drum machine in time with the mix (see
  [Groove Box](groove-box.md)).
- An **instrument plugin** you ticked in **Settings › Plugins**, played with
  a MIDI keyboard or the **KEYBOARD** pads. **EDIT** on the deck opens its
  own window for its sounds and presets (see
  [Instruments](fx.md#instruments)).
- The **Microphone** or the **Aux input**, from the inputs picked in
  **Settings › Audio**: turntables, a drum machine or a singer.

It plays through that deck's channel, so the EQ, filter, effects, fader and
crossfader all work on it. A track on the deck is stopped and ejected
first. An input plays on one deck at a time: dropping it on another deck
moves it there, and while a deck has it, its own channel in the MIC panel
stays quiet. The Instruments page shows which deck each one is on.

To go back to a track deck, load a track onto it, or press EJECT while it is
stopped. openDJ remembers what each deck plays for next time.

## Slip, bend and censor

**SLIP** keeps the track in time while you play with it. While it's lit,
scratching, a loop, a hot cue held down, reverse or a pause takes the sound
away, but a silent playhead carries on, drawn as a dashed lilac line on the
waveforms. When the trick ends (you let go of the jog or the pad, end the
loop, turn reverse off or press PLAY) the track carries on from that line,
on the beat and the phrase, as if it had played on. In slip mode a hot cue
plays only while you hold it. SLIP blinks while the track is held elsewhere;
switching it off then also brings the track back. Slip only works on a deck
that's playing: on a paused deck, scratching and hot cues find a spot as
usual.

**VINYL** decides what touching the top of a controller's jog does. Lit (the
default), it grabs the record so you can scratch. Off, the top bends the
tempo like the rim does, so brushing the jog doesn't stop the track, as CDJ
mode does on a Pioneer player. The DDJ-FLX4 and DDJ-FLX6 follow it.

**Pitch bend** nudges a deck's beat into line by ear: it plays a little
faster or slower while you bend and springs back when you stop. Turn a
controller's jog rim, drag round the on-screen jog's rim, or hold **«** or
**»** beside the jog (2%, or 4% with Shift). <kbd>←</kbd> and <kbd>→</kbd>
bend the deck you started last, the one you're mixing in.

**Censor** plays the deck backwards while you hold it, then carries on as if
it had played forwards, to bleep a word. Shift-click and hold PLAY while the
deck plays, or hold SHIFT and PLAY on the DDJ-FLX4.

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
[Ableton Link](link.md). **Audio Settings…** opens the
outputs in Settings. Levels are kept between runs.

The arrow on the **MIC** button opens the microphone, with **ON**, a level
and one **MIC FX** slot of its own, which the decks' effects never touch.
Auxiliary inputs play on a deck from the **Instruments** source. Choose
the inputs in **Settings › Audio**: **+** under the inputs adds a second
and third auxiliary input, and each auxiliary input takes a pair of
channels (stereo) or one (mono).
