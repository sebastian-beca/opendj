# Lyrics

openDJ shows the lyrics of the track playing in the **Lyrics** tab of the
side panel (next to Info). When the lyrics are timed, the line being sung
is lit and kept in view, and clicking a line plays the deck from there.

![The Lyrics tab following deck A](../images/lyrics-tab.png)

## Where lyrics come from

openDJ reads lyrics you already have. It looks for them in this order:

1. **An `.lrc` file beside the track**, with the same name: `Song.lrc` (or
   `Song.mp3.lrc`) next to `Song.mp3`. Tools such as LRCGET write these.
   Plain LRC gives a time for each line; enhanced LRC, with a time before
   each word (`<00:10.40>the`), gives a time for each word.
2. **The track's tags**: timed lyrics in an MP3's `SYLT` frame, and lyrics
   in `USLT` (MP3), `LYRICS` (FLAC and Ogg) and `©lyr` (M4A). Lyrics in
   these that are written as LRC are read as timed.

openDJ never fetches or sends lyrics on its own. To look them up online,
see [Find lyrics online](#find-lyrics-online).

## What the tab follows

The menu beside **Lyrics** picks the track:

- **Playing track** (the default): the deck playing, staying with it while
  it plays. When nothing plays, the deck played last, or the first deck
  with a track.
- **Selected track**: the track selected in the browser. Its line is lit
  when it is also on a deck.
- **Deck A** or **Deck B**.

Under the track's name the tab says how the lyrics are timed (by word, by
line, or not timed) and where they came from.

## In Player mode

The speech bubble button on the Player bar shows the lyrics of the track
playing large, in place of the track list. The line being sung is big and
white, and when the lyrics are timed by word, the words already sung turn
lilac. Lines gone by fade, and a ♪ marks a break in the singing. Click a
line to play from it. Press the button again to go back to the list.

![Lyrics in Player mode](../images/player-lyrics.png)

## Find lyrics online

openDJ can look a track up on [LRCLIB](https://lrclib.net), a free
community database of synced lyrics. It is off until you turn on
**Find lyrics online** in Settings › Collection › Health.

Then press **Find…** in the Lyrics tab. openDJ asks LRCLIB for the track by
its artist, title, album and length, and lists what it has, best first:
synced lyrics of the same length at the top. **Other length** marks lyrics
for another version of the track (a radio edit, say), whose timing won't
fit. **Use** saves the lyrics as an `.lrc` file beside the track, where
other apps find them too. If the track's folder can't be written, openDJ
keeps them in its own settings folder instead.

![Find lyrics](../images/find-lyrics.png)

Only the track's names and length are sent to LRCLIB, and only when you
press Find.
