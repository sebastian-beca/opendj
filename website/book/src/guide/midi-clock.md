# MIDI clock

MIDI clock plays drum machines, grooveboxes, synths with arpeggiators and
other apps in time with your mix. Most hardware follows MIDI clock rather than
[Ableton Link](link.md).

## Pick where the clock goes

Open **Settings › Controllers** and find **MIDI Clock** at the bottom.

1. Press **+** beside **Add output** and pick the MIDI output your gear is on
   (a MIDI interface, or the gear itself over USB).
2. To reach another app on the same computer, pick **openDJ Clock (other
   apps)** (macOS and Linux) and choose *openDJ Clock* as the MIDI clock input
   in that app.
3. Add as many outputs as you like. × takes one away.

If the gear plays a little early or late against the music, move its
**Offset**: + when it plays early, − when it plays late, up to 50 ms either way.

## Turn it on

Open the **MASTER** button in the top bar and turn on **MIDI Clock**. CLOCK
shows on the MASTER button while it's on.

The clock follows the master deck, the same deck Link follows: the deck that's
playing, or the one started first while both play. It runs at that deck's
tempo, 24 pulses a beat, and stays on its beat.

- When the master deck starts, the clock sends **Start** on its next downbeat,
  so pattern-based gear begins on bar 1 with the music. Cue to a bar and press
  PLAY to start them together.
- If the gear loses the bar (you jumped in the track, or the other deck took
  over), press **RESTART ON BAR**: the gear starts again on the master deck's
  next downbeat, without stopping before.
- When the decks stop, the clock sends **Stop** and carries on at the last
  tempo, so the gear keeps showing it. To keep the gear playing instead, turn
  on **Keep running when the decks stop** in Settings › Controllers.

On Windows only one app at a time can open a MIDI output, so close other apps
using it first.
