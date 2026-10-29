# Effects

openDJ has four kinds of effects:

- **Deck effects**: three on each deck, in the FX bar.
- **Beat FX**: one effect for the main mix or a deck, under the mixer.
- **Color FX**: one on each channel's filter knob.
- **Pad FX**: played from the pads while you hold them.

All of them follow the tempo of the track, so echoes and gates stay on the
beat.

## The FX bar

The **FX** button in the top bar, beside **WAVEFORMS**, shows or hides the
FX bar. It runs the width of the window, under the decks and above the
library: each deck's three effects across that deck's width, and the Beat
FX in the middle, under the mixer.

Each effect has:

- **Its name.** Click it for the list of effects, or step through them with
  the arrows either side.
- **ON** to switch it on and off.
- **Its length in beats**, with ◀ and ▶ to halve and double it. Effects
  that have no length show a slider instead, for their main setting:
  reverb's decay, the filter's resonance, the bitcrusher's bit depth, the
  noise's sweep.
- **D/W**, how much of the effect you hear over the dry track.

A deck's effects start as **Reverb**, **Echo** and **Gate**. They play one
after another, so you can stack them.

In 4-Deck each deck's three effects are stacked in three short rows under
that deck.

### The effects

| Effect | What it does |
|---|---|
| Echo | Repeats the sound on the beat, fading out |
| Reverb | Puts the sound in a room or a hall |
| Filter | Low-pass and high-pass, with resonance |
| Flanger | A sweeping jet sound |
| Bitcrusher | Lo-fi: fewer bits and a lower sample rate |
| Phaser | A softer sweep than the flanger |
| Gate | Cuts the sound in and out on the beat |
| Roll | Repeats a short slice, for a stutter |
| Echo Out | Lets the track go, leaving its echo |
| Brake | Slows the track to a stop, like a turntable |
| Noise | A white noise sweep, for build-ups |
| Spiral | An echo that rises in pitch |

Switching an effect off fades it out quickly. On a DDJ-FLX4,
**SHIFT** + **BEAT FX ON/OFF** releases the Beat FX instead: it stops
taking in the track, and its echo or reverb rings out for a few beats.

## Beat FX

The **BEAT FX** cell under the mixer is one effect on its own. Pick it by
name, choose where it plays with **1**, **2** or **1&2** (deck 1, deck 2 or
both), then use **ON**, its beats and **D/W** as on a deck effect. On a
DDJ-FLX4 the BEAT FX section of the controller plays it.

## Color FX

Each channel's filter knob plays a **Color FX**. The small **▾** beside the
knob picks which. Each deck keeps its own. The knob does nothing in the
middle and more the further you turn it either way.

- **Filter**: a low-pass to the left, a high-pass to the right.
- **Space** and **Dub Echo**: the same filter, gentler, sent into a reverb
  or an echo on the beat. The tail rings on for a few seconds after the knob
  is back in the middle.
- **Noise**: adds filtered white noise.
- **Crush**: lowers the bit depth and sample rate.

On a DDJ-FLX4, **SHIFT** + **SMART CFX** steps through the Color FX.

## Pad FX

The **PAD FX 1** and **PAD FX 2** pad pages play an effect while you hold
the pad, and stop it when you let go: quick throws without touching the FX
bar. See [Pads](pads.md) for how to open them.

| PAD FX 1 | PAD FX 2 |
|---|---|
| Echo | Roll |
| Reverb | Gate |
| Low pass | Echo Out |
| High pass | Brake |
| Flanger | Noise |
| Phaser | Spiral |
| Crush | Gate Fast |
| Echo Max | Reverb Max |

## When you load a track

Loading a track switches its deck's effects off, so the last track's echo
doesn't carry over. Their beats and dry/wet stay as you set them. Turn this
off in **Settings › Decks** under **Auto reset on load** (see
[Decks and mixer](decks.md#auto-reset-on-load)).
