# Your library

openDJ keeps everything it knows about your music in its **library**: one
database file with every track's tags, rating, colour, cues, loops and beat
grid, your playlists, smart lists and folders, and the history of what you
played. The library points at your audio files where they are, and only
moves them between your stage folders when you turn that on (see
[Stages](#stages-inbox-triage-and-collection)).

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

## Stages: Inbox, Triage and Collection

Stages take new music from wherever it lands to a prepared collection.
Set them up in **Settings › Library › Stages**:

- **Inbox folders**: any number of folders where new music arrives, such
  as Downloads, a Soulseek folder, or where purchases land.
- **Triage folder**: where tracks you are preparing are kept. Its
  subfolders are **triage crates** (for example `Yoga STG` or
  `Journey STG`), for sorting tracks by where they'll go.
- **Collection folder**: where prepared tracks are kept. Its subfolders
  (such as `Albums`) stay as they are.

Once the triage and collection folders are set, the tree starts with
**Inbox**, **Triage** and **Collection**, each with its track count and
subfolders, followed by **All Tracks**. Every track gets the stage of the
folder it is in.

Right-click tracks to move them along, or drag them onto a stage:

1. **To Triage** (or **To Triage › a crate**) for inbox tracks worth
   keeping. **Not interested** hides the rest.
2. In Triage, prepare each track: tags, cover, rating, tempo and key. Show
   the **Prep** column (right-click a column heading) for a dot each for
   tags, rating, and tempo and key; tracks in Triage are analysed in the
   background, so tempo and key fill in on their own.
3. **Add to Collection** when it's ready. A track from a triage crate also
   joins the list named after the crate (`Yoga STG` goes to **Yoga**).

**Back to Triage** and **Back to Inbox** move a track back a stage, and
the track menu's **Undo** line takes back the last change.

Files only move between the folders when **Move files** is on. Leave it
off while another app, such as VirtualDJ, still uses them: tracks change
stage in openDJ at once, their moves wait, and they are made when you turn
it on. Files going from the inbox to triage are renamed `Artist - Title
(Remix)` from their tags, unless you turn **Rename files sent to Triage**
off. A file is never moved over another one.

Search with `stage:inbox`, `stage:triage` or `stage:collection`, and
`is:prepped` for tracks with tags, rating, tempo and key.

## When tracks are analysed

Tracks in Triage are analysed in the background. Any other track is
analysed the first time it is loaded on a deck: its waveform,
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

## Library Health

**Library Health**, under All Tracks in the collection, checks the whole
library for what needs fixing. Its page has one line per check, with how
many tracks it finds; **Review** lists them, and the bar above the list
fixes them all at once. To fix only some, pick them (shift-click for a
range) and right-click.

| Check | Finds | Fixes |
|---|---|---|
| Not music | Audiobooks, podcasts and spoken word: Apple Books and Podcasts folders, `.m4b` files, genres such as Audiobook, "(Unabridged)" albums, and chapters (several tracks in one folder sharing a title) | Remove from library, It's music |
| Strays | Files outside your Inbox, Triage and Collection folders | Move to Triage, Remove from library |
| Duplicates | The same track more than once | Review for now |
| Not analyzed | No tempo or key | Analyze, in the background |
| Missing info | No artist, title, album, year or genre | Fill from file name (`Artist - Title.mp3`), written to the file too |
| No cover | No picture in the file's tags or its folder | Find one from the track's info panel |
| No rating | No stars | Rate them in the list |
| Missing files | The file isn't where the library expects it | Find files (looks for the same file name in your stage folders and music folder), Remove from library |
| Long tracks | Over 20 minutes: recorded sets and long pieces | Tag as #mix |

**Ignore for** in the right-click menu tells a check to leave a track
alone. A track that isn't music is left out of the other checks, so an
audiobook doesn't also show up as missing its cover.

**Remove from library** keeps the file where it is. The track no longer
shows, and a rescan or a VirtualDJ import doesn't bring it back; adding the
file again does, and so does **Bring back** at the bottom of the page.

Each check is also a search: `is:notmusic`, `is:stray`, `is:unanalyzed`,
`is:noinfo`, `is:nocover`, `is:unrated`, `is:missing`, `is:long` and
`is:duplicate`.

Covers are looked for in the background, and a file is looked at again
only when it changes. On a Mac, files that iCloud has taken off the disk
are skipped, so the check doesn't download your whole collection.

## Search

Search matches titles, artists, BPM and key. Rest the pointer on the search box
for a moment to see everything search understands.

## Filters

The filter bar above the track list narrows it down with chips, as djay Pro
does. Click a chip to pick values: tick a few genres, a trip stage or some
tags, type a BPM range, or turn on **Near the playing track** (BPM) or
**Mixes with the playing track** (Key) so the list follows what's playing.
Chips work together with each other and with the search, and **Clear**
empties them all. **+** adds any other field, and **Keep on the bar** pins
it there. The filter button next to search shows or hides the bar (it starts
hidden and lights up while filters are on), and
**Save** keeps the search and the chips as a smart list.

## Columns

Right-click any column heading to choose which columns the list shows, your
own fields included, or **Reset columns** to go back to the default. Drag a
divider on a heading's right edge to make that column wider or narrower (the
columns after it move along and keep their widths), and drag the heading itself
to move it. Click a heading to sort by it. openDJ remembers your columns.

## Your own fields

Name grouping and your tag fields in **Settings › Library › Your fields**,
for example Trip Stage, Mood and Instrument, and add as many tag fields as
you need. VirtualDJ's User 1 and User 2 come in as the first two. Each name
works as a search word too (`mood:uplifting`, `tripstage:"4 peak"`), and so
does each number (`tag1:uplifting`). The `#` in front of a tag is optional
when searching.
