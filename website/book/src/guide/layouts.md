# Layouts

The menu at the top left of the window switches layouts:

- **Player** works like a music player and mixes for you. One bar shows the
  track you hear: its album art, title, artist, BPM and key (coloured by its
  place on the key wheel, so keys that mix have neighbouring colours). The
  track's overview runs across the middle, with the time played on its left
  and the track's length on its right. Click or drag it to move through the
  track; the shaded part at the end is where the next mix starts. Under it
  are **Order**, **Previous**, **Play/Pause**, **Next** and **Repeat**. With
  Repeat on, a played track goes back to the end of Up Next instead of
  leaving it, so the list keeps going round. On the right are what Automix
  does next, the master volume, and the queue button, which shows or hides
  **Up Next** (the Automix list).
  The search box in the top bar (**⌘F**) is Player's only search box: it
  filters the track list below and shows the best matching tracks as you
  type. Point at one, or move with **↑** and **↓**, and pick **Play next**,
  **Add to queue** (the end of Up Next) or **Play now**, which mixes into it
  straight away. The keys are **↵**, **⇧↵** and **⌘↵**; **Esc** clears the
  box and the list's search.
  Underneath, decks 1 and 2 take turns and [Automix](automix.md) mixes from
  one to the other. **Play** switches Automix on, **Next** mixes into the next
  track straight away, **Previous** goes back to the start of the track, and
  **Order** puts Up Next in order (shuffle, BPM, harmonic and so on). The
  large waveforms and the FX bar are hidden in Player.
- **2-Deck** shows the full decks, with pads and loops, and the mixer between
  them (see [Decks and mixer](decks.md)).
- **4-Deck** shows four decks side by side, in the order 3, 1, 2, 4, as djay Pro
  does. Each deck is a strip with its own EQ, filter, gain and channel fader,
  PLAY and CUE, loop, pads and effects. A bar under the strips holds each
  deck's crossfader side, **SYNC** and tempo, either side of the crossfader;
  click a deck's tempo there for key lock, quantize and a tempo reset. With
  **WAVEFORMS** lit in the top bar, the four large waveforms are stacked above
  the decks in the same order.
  A [Groove Box](groove-box.md) deck shows its step grid only in 2-Deck.

The button at the bottom left of the window hides the decks for a compact
view in 2-Deck and 4-Deck.

<!-- The Prep layout is hidden until v0.6 (PREP_OFFERED in crates/app/src/main.rs).
   Put this section and its line in the list above back with it.

## Prep

Prep shows deck 1 alone, with no mixer, crossfader or effects, and opens the
browser on **In Prep** (see
[Stages](music.md#stages-inbox-in-prep-and-play-ready)). The In Prep list has
its own columns, to see what each track still needs: **Grid** (✓ locked,
✓ edited, ? check when the analysis wasn't sure, the number of tempo
sections, or detected), **Cues** (how many hot cues), **Tags** (✓ with a
genre and at least one of your tags), **Cover**, **Ready** (checklist lines
ticked) and **Score**. Pick other columns from the header's menu as
in any list; In Prep keeps its own set. Load a track and work through it:

- The large waveform shows 8 bars; **−** and **+** above the hot cues show
  more or fewer. While the grid is unlocked, dragging the waveform slides the
  audio under the grid, so you can line the beat lines up with the kicks by
  hand; one drag is one **Undo**. With the grid locked, dragging moves through
  the track as on the other layouts.
- **Beat grid** has the grid tools from the deck's grid menu: **Bar 1 here**,
  **Beat here**, **Tap**, **Click** (a metronome on every beat), move the grid
  earlier or later, tempo **−** and **+**, **½** and **×2**, tempo sections,
  **Undo** and **Detected** (back to the detected grid). **Lock** keeps the
  grid from being changed.
- **CUE** and play, jumps of 1, 4 and 8 bars, a loop, and the deck's
  headphone cue and level.
- Eight hot cue pads with their names and times: click an empty pad to set
  it, a set one to jump there, Shift-click to clear it.
- The side panel's **Cues** tab lists the track's hot cues, memory cues and
  saved loops in track order. Type a name, click the dot for the next colour,
  click a time to jump there, or click the number to move the cue to another
  pad (the cue on that pad takes its place) or make it a memory cue.
  **+ Memory cue** (or **C**) adds one at the playhead; while a loop plays it
  saves the loop instead. Outside Prep the tab follows deck 1 or deck 2.
- The side panel's **Checklist** tab lists what makes the track ready. The
  app ticks its own lines when the track has them: **Grid checked** (locked
  or edited), **4 hot cues**, **Genre**, **Rating** and **Cover**; tick one
  by hand when the track is fine without it. Add your own lines, such as
  "Vocals checked", in Settings › Collection › Stages (or **Edit checklist
  lines…** at the bottom of the tab), where you can also turn the app's
  lines off. Tick a line with a click or **Option+1**–**9**. Below the lines,
  give the track your own score from 1 to 10; it shows in the Score column
  and doesn't count towards ready.
- **Play Ready, next** lights up once every line is ticked (the count is
  beside it). It works before that too, for a track that is fine as it is.
  It locks the grid, moves the track to Play Ready and loads
  the next track In Prep. Cmd-Z (Ctrl-Z) takes the stage change back.

A strip under the waveform lists the quick review keys, with how long this
track has taken and your average per track sent to Play Ready from Prep.

Keys in Prep: **Enter** Play Ready, next; **Tab** and **Shift-Tab** next and
previous phrase (8 bars at a time when the track has no phrases); **←** and
**→** four bars; **Option+1**–**9** tick a checklist line; **1**–**8** set or jump to a hot cue
(Shift clears it); **B** Bar 1 here; **T** tap; **M** the click; **[** and **]**
move the grid; **−** and **=** tempo; **L** lock; **C** memory cue. Space plays and pauses, as
everywhere.
-->

## Crossfader assignment

In 4-Deck, each deck says which side of the crossfader it is on: **LEFT**,
**RIGHT** or **THRU** (not affected by the crossfader). Decks 1 and 3 start on
the left, 2 and 4 on the right. Click the label to change it.

## Decks 3 and 4 on a two-deck controller

A controller with two decks, such as the DDJ-FLX4, plays decks 1 and 2. In
4-Deck, click deck 3's number to move the controller's left side to deck 3,
and deck 1's number to move it back; deck 4's and deck 2's numbers do the same
for the right side. The number of each deck the controller plays is lit.
Switching to another layout moves both sides back to decks 1 and 2.

Automix always mixes between decks 1 and 2.
