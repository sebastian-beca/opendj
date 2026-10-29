# Your library

openDJ keeps everything it knows about your music in its **library**: one
database file with every track's tags, rating, colour, cues, loops and beat
grid, your playlists, smart lists and folders, and the history of what you
played. The library points at your audio files where they are, and only
moves them between your stage folders when you turn that on (see
[Stages](#stages-inbox-triage-and-collection)).

The **Collection** is the list of every track in the library. It is the top
of the tree on the left, and the first source in the rail.

## Sources

The rail at the far left picks where the tree and the track list come from:

- **Collection**: your library, with its stages, crates, sets, playlists
  and history.
- **Files**: any folder on your computer, as it is on disk.
- **Spotify**, **TIDAL**, **Apple Music**, **Bandcamp**, **Beatport**,
  **OneLibrary** and **SoundCloud**: coming soon. Streaming services will be
  for browsing only.

## How tracks come in

Tracks come into the library in three ways:

- **Your music folders.** openDJ looks through them each time it starts,
  and when you press **Rescan** in **Settings › Library**. New files are
  added, and changed files have their tags read again. With a big collection
  you can turn off **Update library on start** there, and press **Rescan**
  when you add music. Add as many folders as you like there with
  **Add…**, and take one out with **Remove**, or start openDJ with the
  `OPENDJ_MUSIC` environment variable set to one. There's no default folder:
  `~/Music` is where Ableton, GarageBand and other apps keep their samples,
  so openDJ only looks there if you add it. When a folder is no longer looked through,
  its tracks leave the library, except ones you rated, played, staged or put
  in a list. The files stay where they are.
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

To analyse tracks yourself, right-click them and choose **Analyze Track**
(or **Analyze 12 Tracks** for a shift-click selection), or right-click a
list, folder or stage and choose **Analyze All**. They are analysed again
from scratch in the background, one at a time: tempo, beat grid, key,
loudness, mini waveform and phrases. A grid you edited and a key you typed
in or got from the tags stay as they are. While it runs, each waiting
track has a gray dot by its title (lilac and pulsing while it is being
analysed), and the footer shows how far along it is, with a ✕ to stop.

A track with no cover also gets one: openDJ takes the first cover it finds
online, if it is at least 600 pixels, and saves it in the file's tags. When
the first one is smaller the track is left without, so you can pick one
yourself from the cover dialog in **Info**.

## Phrases

Once a track has a beat grid, openDJ splits it into phrases of whole
bars: intro, build (up), drop (peak), chorus (main groove), break and
outro. Each deck's overview shows them as a bar under the minute dots:
deep blue for the intro and outro, violet for a build, rose for a drop,
green for a chorus and tangerine for a break. Phrases are 8 to 32 bars
long, so a short fill never gets one of its own. They are saved in the
library with the track, and move with the grid when you edit it.

Rest the pointer on a deck's overview to see the phrase under it: its
kind, how many bars it runs and where it starts and ends.

Right-click a deck's overview and choose **Hide phrases** (or **Show
phrases**) to turn the bars off or on for every deck.

## Bringing in a VirtualDJ library

Press **Set up…** next to **Import from VirtualDJ** in **Settings ›
Library › Import**. That tab is always there and lists every DJ app openDJ
can bring a library in from; Mixxx, Serato DJ and Traktor are marked coming
soon. Three steps follow:

1. **Find your library.** openDJ looks for `database.xml` in
   `~/Library/Application Support/VirtualDJ` (or `~/.vdj` on Linux) and
   shows what it holds. Choose another file if yours is elsewhere.
2. **Choose which lists come across.** Every playlist, virtual folder,
   filter folder (as a smart list) and history session is in a tree with a
   tick box. Ticking a folder ticks everything in it, and **All lists** at
   the top does the lot. History is grouped by year. Everything starts
   ticked except filter folders, whose searches may not mean quite the same
   here. Lists you add in VirtualDJ later come in ticked.
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

Once it's on, VirtualDJ's section of the Import tab opens up with these
choices and the two buttons below. **Lists › Choose…** opens the tree again
to change which lists come in. **Stop** leaves what came in but
stops updating it. **Remove…** takes out
the VirtualDJ folder and history, VirtualDJ's cues, grids and markers, and
the tracks only VirtualDJ had. Tracks in your own lists or music folder
stay. Either one closes the section again, back to **Set up…**.

### Lists of online tracks

Streamed tracks (VirtualDJ's online catalogues, a streaming service in
rekordbox) never come in, as there is no file to play. A list made only of
them starts unticked, marked **Online only**, and stays out even if you tick
it, rather than coming in empty.

## Bringing in a rekordbox library

**Import from rekordbox** works the same way. openDJ reads rekordbox 6 and
7's own database, `master.db`, which it finds where rekordbox keeps it
(`~/Library/Pioneer/rekordbox` on a Mac, `AppData\Roaming\Pioneer\rekordbox`
on Windows). For rekordbox 5, or if the database can't be opened, export
your collection from rekordbox (**File › Export Collection in xml format**)
and choose that file in the first step.

From `master.db` each track keeps its tags, stars, colour, comment, play
count, date added, key, My Tags (as tags like `#warmup`), hot cues A to H
with their names and colours, memory cues, saved loops, its beat grid with
any tempo changes, and rekordbox's phrases (Intro, Verse, Chorus and so on,
shown on the waveform as openDJ's own phrases are). Playlists come in their
folders, intelligent playlists as smart lists where openDJ's search can say
the same thing, and history into the History tab. An XML export holds
tracks, grids, cues and playlists, but no phrases, history or intelligent
playlists.

### The same song from two apps

When a second app's library has a different file of a song you already have
(the same artist and title, ignoring case and punctuation, and a length
within two seconds), openDJ doesn't add it twice. The copy already in your
library stays, and the second app only fills in what it left empty. Its
lists point at that copy. Its grid is only used when the track has none from
another app, and its hot cues come in as memory cues when the track already
has hot cues, so nothing is lost. Removing the second import takes out what
it added and leaves the track as it was.

## Crates and sets

Your lists come in two kinds. A **crate** is a list you keep: Yoga,
Journeys, Dance, a style. A track can be in as many crates as you like,
and the **Crates** column (right-click a heading) shows which ones it is
in. Search with `crate:yoga` for tracks in a crate with "yoga" in its
name, `crate:=yoga` for that crate only, and `crate:""` for tracks in no
crate yet.

A **set** is a lilac folder made for one gig. **+ › New set…** makes one
with a list for each step of the template: **Staging** (what you'd like
to play), **Set** (what you plan to play, about twice what you'll need)
and **Live set** (what you actually played). Change the template in
**Settings › Library › Stages** under Sets, for example to add Openers,
Closers or Transitions. Selecting a set shows all its tracks with their
total length, and how many are still in Triage without their basic prep.

After the gig, open the **History** tab, pick what you played (or the
whole session) and right-click: **Add to Set List** lists your sets,
newest first, and puts the tracks in that set's Live set.

## Library Health

[Library Health](library-health.md), under All Tracks in the collection,
finds what needs fixing in your library: duplicates, junk in names, missing
tags and covers, files that went missing, and more.

## Search and filters

Search, the filter bar and smart lists have a page of their own:
[Filtering tracks](filtering.md).

## Columns

Right-click any column heading to choose which columns the list shows, your
own fields included, or **Reset columns** to go back to the default. Drag a
divider on a heading's right edge to make that column wider or narrower (the
columns after it move along and keep their widths), and drag the heading itself
to move it. Click a heading to sort by it. openDJ remembers your columns.

## Your own fields

Name grouping and your tag fields in **Settings › Library › Fields**,
for example Trip Stage, Mood and Instrument, and add as many tag fields as
you need. VirtualDJ's User 1 and User 2 come in as the first two. Each name
works as a search word too (`mood:uplifting`, `tripstage:"4 peak"`), and so
does each number (`tag1:uplifting`). The `#` in front of a tag is optional
when searching.
