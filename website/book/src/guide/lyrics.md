# Lyrics

openDJ shows the lyrics of the track playing in the **Lyrics** tab of the
side panel (next to Info). When the lyrics are timed, the line being sung
is lit and kept in view, and clicking a line plays the deck from there.

## Where lyrics come from

openDJ reads lyrics you already have. It looks for them in this order:

1. **An `.lrc` file beside the track**, with the same name: `Song.lrc` (or
   `Song.mp3.lrc`) next to `Song.mp3`. Tools such as LRCGET write these.
   Plain LRC gives a time for each line; enhanced LRC, with a time before
   each word (`<00:10.40>the`), gives a time for each word.
2. **The track's tags**: timed lyrics in an MP3's `SYLT` frame, and lyrics
   in `USLT` (MP3), `LYRICS` (FLAC and Ogg) and `©lyr` (M4A). Lyrics in
   these that are written as LRC are read as timed.

openDJ never fetches or sends lyrics on its own.

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
