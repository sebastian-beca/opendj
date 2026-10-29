# Design

## How a tutorial runs

- Built on the `add-first-run-setup` tour engine: a dimmed window with the real control highlighted, a tip bubble beside it, and **Skip step** / **Close** in the bubble.
- Each step names the control (by its control key or UI element) and the condition that ends it, read from the same control values the engine and controllers use, so doing it on screen or on a controller both count.
- On start a tutorial remembers the layout, decks and FX, switches to 2-Deck, and loads the CC0 demo tracks (two at about the same BPM, keys that mix). **Use my tracks** at the start skips the loading and asks the DJ to load tracks instead. Closing or finishing puts back the layout; demo tracks stay loaded only if the DJ finished.
- Done marks are kept in settings. Help › Tutorials shows each tutorial with its length, a tick when done, and **Open guide** for its page.
- Guide pages (stable paths agreed with the website docs update): `guide/looping.md`, `guide/fx.md`, `guide/filtering.md`, `guide/automix.md`, `guide/controllers/ddj-flx4.md`.

## Steps

### Looping (about 3 min, deck 1 playing)

1. Press a beat loop size (4) in the loop column. Ends when `loop_enabled`.
2. Halve it with the left arrow of the loop size, then double it back. Ends after `loop_halve` then `loop_double`.
3. Press the loop size again to let go. Ends when not `loop_enabled`.
4. Manual loop: press **IN** and then **OUT** where you want it to end. Ends when `loop_enabled` after `loop_in`.
5. Save it: switch the pads to HOT CUE and press an empty pad while the loop plays. Ends when a `hotcue_N_type` becomes a loop.
6. Exit the loop, then press that pad again to jump back into the saved loop. Ends when `loop_enabled` from the pad.
7. Loop roll: switch pads to LOOP ROLL and hold a pad; let go and the track carries on where it would have been. Ends after a roll is held and released.

### Using FX (about 3 min, both decks playing, deck 2 low)

1. Open the FX bar with the **FX** button in the top bar. Ends when `fx_bar_visible`.
2. Deck FX: turn on Echo on deck 1 (ON), change its beats, turn D/W up. Ends after the unit is on and its beats changed.
3. Pick another effect from the name dropdown. Ends when the effect changes.
4. Beat FX: pick an effect in the centre, choose where it plays, press ON. Ends when the Beat FX is on.
5. Color FX: pick **Space** from Color FX ▾ beside deck 1's filter knob, and turn the knob both ways. Ends after the knob passes both sides of centre.
6. Turn everything off. Ends when all FX on deck 1 and Beat FX are off.

### Filtering tracks (about 3 min, deck 1 playing)

1. Search: type an artist or title in the search box. Ends when the list narrows.
2. Add a filter chip: **+** › Genre, pick one. Ends when a Genre chip is active.
3. BPM chip: set a range around the playing track. Ends when a BPM chip is active.
4. Key chip: **Mixes with the playing track**; note the lilac keys in the list. Ends when key match is on.
5. Save the search as a smart list and name it. Ends when a smart list is made.
6. **Clear** the bar. Ends when no chip is active.

### Automix (about 3 min)

1. Open the Automix panel on the right. Ends when the panel shows.
2. Queue three demo tracks: drag them in or right-click › **Add to Automix**. Ends at three or more tracks.
3. Pick a length (16 bars) and a style (Bass swap). Ends after both change.
4. Press **AUTOMIX**. Ends when Automix is on and a track plays.
5. Press **Mix now** to start the next transition early. Ends when a transition starts.
6. Take over: move a channel fader or the crossfader, or press AUTOMIX again. Ends when Automix is off.

### DDJ-FLX4 (about 5 min, only with a DDJ-FLX4 connected)

Every step waits for the controller's own control; the tip shows where it is on the controller picture from the Controller Editor.

1. Turn the browse knob and press **LOAD** on deck 1, then on deck 2.
2. **PLAY/PAUSE** deck 1; **CUE** to go back to the cue point; play again.
3. Jog wheel: nudge with the outer ring while playing, scratch on the top.
4. **BEAT SYNC** on deck 2, then play it; watch the tempo follow deck 1.
5. Headphones: the channel **CUE** button on deck 2, then the headphones **MIXING** knob.
6. Bring deck 2 in: channel fader up, swap the bass with the **LOW** EQs, then turn the **CFX** filter knob on deck 1.
7. Pads: **HOT CUE** set and press a cue; **BEAT LOOP** a 4-beat loop; **SAMPLER** a sound.
8. Beat FX: **BEAT FX SELECT**, **CH SELECT**, **ON/OFF** and the **LEVEL/DEPTH** knob.

Steps that rely on parts of the FLX4 mapping not yet checked on hardware (master/phones level, some LEDs) are kept out until Seba has tried them.
