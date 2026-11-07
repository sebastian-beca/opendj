# Groove Box

The Groove Box is a small drum machine that takes the place of a track on a
deck. It plays in time with the mix, so you can add a beat under a track,
keep a groove going between two tracks, or practise mixing without a second
track.

## Start a groove

1. Click **Instruments** in the rail at the far left (the keyboard icon)
   and drag a kit from **Groove Box** onto a deck. A track on that deck is
   stopped and ejected first.
2. Press **PLAY** in the Groove Box. It starts on the next downbeat of the
   playing deck, so it is in time from the first hit.
3. Pick a genre (Basics, Techno, House, Drum & Bass and more) and then a
   groove from the menu beside it. Each groove has a line saying what it is.

To go back to a normal deck, load a track onto the deck, or press EJECT
on the stopped Groove Box.

## Sequencer, Synth and Mixer

The Groove Box fills the whole deck. Along the top are PLAY, the kit menu,
the genre, the groove, **Chain ▾**, **A** to **D**, the tempo and three
tabs.

**SEQUENCER** shows eight tracks, each in its own color, with 16 squares
for the 16 steps of a bar: kick, snare, closed hat, open hat, clap, rim,
cowbell and cymbal. The cymbal track plays the crash or the ride, whichever
the groove you load uses. The icon on the left shows the track's sound; click it
to hear the sound, and hover over it to see its name. A lit square plays the
sound on that step. Click a square to add or remove a hit; when the Groove
Box is stopped, the click also plays the sound. Right-click a lit square to
make the hit accented (brighter, a little louder), then soft (paler, half as
loud), then normal again. A track with no hits stays
dimmed, and a track's icon lights up as it plays. The brighter column is the
step that's playing. **M** mutes a track, **S** solos it.

**SYNTH** shows each track's sound. Click the sound's name to pick another
from the kit. **PITCH** tunes it up or down by up to an octave, **DECAY**
makes it shorter, and **TONE** darkens it (turned left) or thins it (turned
right).

Under the kick, **KEY** tunes the kick to the playing track's key. Click it
to go from **OFF** to **ROOT** (the key's root note) to **5TH** (its fifth)
and back. The kick moves to the nearest such note, never more than half an
octave, and the button shows the note it plays. This suits long, boomy
kicks most (Boom, Analog); a short, punchy kick has little pitch to tune.

**MIXER** has a strip for each track: **PAN** places it left or right, the
fader sets its level beside a meter, and **M** and **S** mute and solo it.

## Kits and grooves

The kit menu at the top left has six kits, all synthesised, so they are
free to use anywhere:

- **Analog** and **Synthetic**: kick, snare, clap and hats from the drum
  models of Mutable Instruments Plaits, with two toms each.
- **Boom**: a deep, long kick and soft analogue drums, 808-style.
- **Punch**: a short, punchy kick and bright hats, 909-style, for house and
  techno.
- **Tin**: small, ticking rhythm-box drums, 606-style.
- **Dub**: a soft kick, a rimshot and a chord stab, for dub techno.

Every kit plays the same eight tracks, so any groove sounds right on any kit.
Kits with more sounds (toms, shaker, chord) offer them in each track's sound
menu in **SYNTH**.

**A**, **B**, **C** and **D** are four grooves you can switch between. A new
one starts at the next bar, so the switch is always in time. Picking a
groove loads it into the one you picked.

The grooves come in genres. **Basics** has openDJ's own six; the other
genres (techno, house, drum & bass, breakbeat, ambient, dubstep and more, 400
grooves in all) are drum patterns from
[mpump](https://github.com/gdamdam/mpump), used under its licence (AGPL-3.0).

**Chain ▾** picks a second groove from the same genre. It goes into the
groove after the one you picked (A's partner is B, C's is D), and the two
take turns, a bar each, until you set Chain to Off.

Your grooves, kit, sounds and mixer settings are kept when you quit.

## Timing

The Groove Box follows, in this order:

1. The deck that SYNC follows, or else the first playing deck with a tempo.
2. Ableton Link, when it is on (see [Ableton Link](link.md)).
3. Its own tempo, 120 BPM, when nothing else is playing.

The tempo button in the top bar shows the BPM, outlined while it follows the
mix. Click it for:

- **Follow the mix**: on by default. Turn it off to keep the Groove Box at
  its own tempo whatever plays.
- **−1**, **−.1**, **+.1** and **+1**: change the BPM. This turns following
  off, so the tempo stays where you put it.
- **Half time**, **Normal** and **Double**: follow the mix at half or twice
  its tempo (and turn following back on).
- **SWING**: delays every other step for a looser feel.

If
the leading deck jumps (a hot cue or beat jump), the Groove Box moves to the
new beat at its next bar rather than mid-bar.

The Groove Box plays through its deck's channel, so the EQ, filter, effects,
channel fader and crossfader work on it as they do on a track.

## In 4-Deck

In the 4-Deck layout a Groove Box deck shows its name, kit and tempo, and its
PLAY works, but the step grid is only in the 2-Deck layout for now.
