# Your library

openDJ keeps everything it knows about your music in its **library**: one
database file with every track's tags, rating, colour, cues, loops and beat
grid, your playlists, smart lists and folders, and the history of what you
played. The library points at your audio files where they are; it never
moves, copies or changes them.

The **Collection** is the list of every track in the library. It is the top
of the tree on the left, and the first source in the rail.

## How tracks come in

Tracks come into the library in three ways:

- **Your music folder.** openDJ looks through it each time it starts, and
  when you press **Rescan** in **Settings › Library**. New files are added,
  and changed files have their tags read again. Choose the folder there
  (`~/Music` until you do), or start openDJ with the `OPENDJ_MUSIC`
  environment variable set to it.
- **Files.** The **Files** source in the rail browses any folder on your
  computer. Load or drag a track from there to add it.
- **Another DJ app.** openDJ can bring in your VirtualDJ library (see below).

Bringing a track in only reads its tags: title, artist, album, genre, year,
key, BPM, rating, comment and cover art. It is quick, and nothing is played
or analysed yet.

## When tracks are analysed

A track is analysed the first time it is loaded on a deck: its waveform,
tempo and beat grid, and its key when the tags don't have one. It plays
while that happens, and the waveform fills in as it goes. The result is
saved, so the track loads at once next time; **Clear** in **Settings ›
Library** throws those results away.

Grids, keys and cues brought in from VirtualDJ are used as they are, so
those tracks don't wait for analysis.

## Bringing in a VirtualDJ library

Press **Set up…** next to **Import from VirtualDJ** in **Settings ›
Library**. Three steps follow:

1. **Find your library.** openDJ looks for `database.xml` in
   `~/Library/Application Support/VirtualDJ` (or `~/.vdj` on Linux) and
   shows what it holds. Choose another file if yours is elsewhere.
2. **Choose which lists come across:** playlists, virtual folders, filter
   folders (as smart lists) and history.
3. **Choose whose tags win** when a track's title, rating or colour differ:
   VirtualDJ's, while you still use it, or openDJ's.

Press **Import**. Each track keeps VirtualDJ's tags, stars, colour, comment,
play count, dates, BPM, beat grid, key, hot cues with their names and
colours, saved loops, gain, and its remix and automix points. Anything
VirtualDJ leaves empty is filled in from the file's own tags. The lists go
into a **VirtualDJ** folder, and history into the History tab.

openDJ only ever reads VirtualDJ's files. If you keep using VirtualDJ,
openDJ checks its files each time it starts and brings in what changed.
Tracks are matched by their file, so nothing is added twice. Grids and cues
you edited in openDJ are kept. The VirtualDJ folder is made again each time,
so change those lists in VirtualDJ, or copy one out of the folder to keep
editing it in openDJ.

**Stop** leaves what came in but stops updating it. **Remove…** takes out
the VirtualDJ folder and history, VirtualDJ's cues, grids and markers, and
the tracks only VirtualDJ had. Tracks in your own lists or music folder
stay.

## Keeping it tidy

- Keep your files in one music folder, and point openDJ at it.
- Move or rename files with openDJ closed, then let it rescan. The
  **Missing files** smart list shows tracks whose file has gone.
- **Duplicates** shows tracks that appear more than once.
- Load a new track once before a set, so it is analysed and ready.

## Search

Search matches titles, artists, BPM and key. The **?** next to the search box
lists everything search understands.

## Filters

The filter bar above the track list narrows it down with chips, as djay Pro
does. Click a chip to pick values: tick a few genres, a trip stage or some
tags, type a BPM range, or turn on **Near the playing track** (BPM) or
**Mixes with the playing track** (Key) so the list follows what's playing.
Chips work together with each other and with the search, and **Clear**
empties them all. **+** adds any other field, and **Keep on the bar** pins
it there. The filter button next to search shows or hides the bar, and
**Save** keeps the search and the chips as a smart list.

## Columns

Right-click any column heading to choose which columns the list shows, your
own fields included, or **Reset columns** to go back to the default. Drag a
heading's right edge to make it wider or narrower, and drag the heading itself
to move it. Click a heading to sort by it. openDJ remembers your columns.

## Your own fields

Name grouping and your tag fields in **Settings › Library › Your fields**,
for example Trip Stage, Mood and Instrument, and add as many tag fields as
you need. VirtualDJ's User 1 and User 2 come in as the first two. Each name
works as a search word too (`mood:uplifting`, `tripstage:"4 peak"`), and so
does each number (`tag1:uplifting`). The `#` in front of a tag is optional
when searching.
