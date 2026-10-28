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

- **Your music folders.** openDJ looks through them each time it starts,
  and when you press **Rescan** in **Settings › Library**. New files are
  added, and changed files have their tags read again. With a big collection
  you can turn off **Update library on start** there, and press **Rescan**
  when you add music. Add as many folders as you like there with
  **Add…**, and take one out with **Remove** (`~/Music` until you choose
  your own), or start openDJ with the `OPENDJ_MUSIC` environment variable
  set to one.
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
Library › General**. Three steps follow:

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

Once it's on, VirtualDJ gets its own tab in **Settings › Library**, with
these choices and the two buttons below. **Stop** leaves what came in but
stops updating it. **Remove…** takes out
the VirtualDJ folder and history, VirtualDJ's cues, grids and markers, and
the tracks only VirtualDJ had. Tracks in your own lists or music folder
stay. Either one takes the VirtualDJ tab away again.

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
| Outside folders | Files outside your music folders and stage folders, such as ones that came in with a VirtualDJ import | Move to Triage, Remove from library |
| Duplicates | The same track more than once: same artist, title and remix | Fix all easy, Keep this copy, Link as versions (see [Duplicates](#duplicates)) |
| Not analyzed | No tempo or key | Analyze, in the background |
| Missing info | No artist, title, album, year or genre | Fill from file name (`Artist - Title.mp3`), written to the file too; Look up online (see [Online lookup](#online-lookup)) |
| Junk in names | Track numbers, web sites, "(Official Video)", underscores, or a remix or guest in the title | Clean up… (see [Clean-up](#clean-up)), Look up online |
| No cover | No picture in the file's tags or its folder | Find covers online, or one by one from the track's info panel |
| No rating | No stars | Rate them in the list |
| Missing files | The file isn't where the library expects it | Find files (looks for the same file name in your stage folders and music folders), Remove from library |
| Long tracks | Over 20 minutes: recorded sets and long pieces | Tag as #mix |

**Ignore for** in the right-click menu tells a check to leave a track
alone. A track that isn't music is left out of the other checks, so an
audiobook doesn't also show up as missing its cover.

**Remove from library** keeps the file where it is. The track no longer
shows, and a rescan or a VirtualDJ import doesn't bring it back; adding the
file again does, and so does **Bring back** at the bottom of the page.

Each check is also a search: `is:notmusic`, `is:stray`, `is:outside`, `is:unanalyzed`,
`is:noinfo`, `is:junk`, `is:nocover`, `is:unrated`, `is:missing`, `is:long` and
`is:duplicate`.

Covers are looked for in the background, and a file is looked at again
only when it changes. On a Mac, files that iCloud has taken off the disk
are skipped, so the check doesn't download your whole collection.

**Find covers** looks online (Cover Art Archive, iTunes, and Discogs with
a Discogs token in Settings › Library) for each track, one at a time in the
background, and embeds the largest cover found, scaled down to 1024 pixels
when it is bigger. Tracks it finds nothing for stay in the list: search
those from the track's info panel.

### Clean-up

**Clean up…** on Junk in names (or a right-click on some of its tracks)
shows every change before anything happens: the track, the field, what it
says now and what it becomes. Untick a change to leave it, then **Apply**.
The changes go to the library and to the files' tags. **Undo** in the bar
or the track menu takes the whole batch back.

| Rule | Example |
|---|---|
| Track numbers | `03 - Road` becomes `Road`; `Lane 8 - Fingerprint` by Lane 8 becomes `Fingerprint` |
| Web sites | `Glue www.free-mp3.com` becomes `Glue` |
| Video words | `Awake (Official Video) [HD]` becomes `Awake` |
| Underscores and spaces | `bicep_glue` becomes `bicep glue` |
| Remix to its field | `Bodhi Mandala (Drumspyder Remix)` puts `Drumspyder Remix` in Remix |
| Featured artists | `Road (feat. Arctic Lake)` by Lane 8 becomes `Road` by `Lane 8 feat. Arctic Lake` |
| Title Case (off) | `CARBON BASED LIFEFORMS` becomes `Carbon Based Lifeforms` |

Each rule can be turned off in Settings › Library › Stages, under
Clean-up. Titles that start with a number, such as `99 Luftballons`, stay
as they are.

**Genres…** at the bottom of the Library Health page lists every genre with
how many tracks have it. Genres spelled like another but for capitals,
spaces or punctuation (`Electronica` and `electronica`) are lilac. Tick the
ones that mean the same, type the name to keep (the most used one is
filled in), and **Merge**. Undo takes a merge back too.

### Online lookup

**Look up online** on Missing info or Junk in names (or a right-click on
some of their tracks) looks each track up on MusicBrainz, one a second in
the background, and opens the same review table as Clean-up with what it
found: artist, title, album, year, and with a Discogs token also the label
and style (as the genre). Next to each track is where the match came from
and how sure it is.

Only changes that fill an empty field from a match at least 80% sure are
ticked. A change to something the track already has is shown unticked, so
nothing you named yourself is overwritten unless you tick it. Fields you
typed in the track's info panel are never proposed. **Apply** writes the
ticked changes to the library and the files, and Undo takes them back.

With an **AcoustID key** (free on acoustid.org; paste it in Settings ›
Library under Online lookup) openDJ recognises tracks by their sound, so
files called `track01.mp3` are found too. Without one, tracks are looked up
by artist and title, and those with neither are skipped.

### Duplicates

Copies are tracks with the same artist, title and remix, ignoring case,
punctuation and "Original Mix". Under **Duplicates** in the tree, openDJ
sorts them into groups by what tells them apart:

| Group | What it is | What happens |
|---|---|---|
| Same file | The same file, byte for byte, in two places | One is kept |
| Better copy | The same recording, one lossless or at a higher bitrate | The better file is kept |
| Same format | The same recording in the same format, in different folders | One is kept |
| Different lengths | More than 3 seconds apart: an edit and an extended mix, say | You decide: **Link as versions** or **Keep the best copies** |

In each group the copy kept comes first and the others are dimmed. openDJ
keeps the copy in your Collection folder, then a lossless file over a lossy
one, then the higher bitrate, then the one with cues or a checked grid,
then the most played. To keep another one, right-click it and choose
**Keep this copy**. The **Location** column (right-click a heading) shows
which folder each copy is in.

**Fix all easy** merges every group but Different lengths at once. Merging
loses nothing: the kept copy takes the others' lists and history, cues and
grid (when it has none), links, the highest rating, all their plays, the
earliest date added, and any tags it lacks. The other copies leave the
library and their files go to `_Quarantine/<date>/`, next to your
Collection folder. With **Move files** off in Settings › Library, the moves
wait until you turn it on. openDJ never deletes anything there: the bottom
of the Library Health page shows how many files are waiting, and
**Show folder** opens it, so you can empty it once you're sure.

**Undo** in the bar (or the track menu) brings the copies back and moves
their files back. The merged plays and tags stay on the kept copy.

**Link as versions** keeps copies of different lengths and links them, so
each shows the others on its Linked tab, and they leave the Duplicates list.

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

Name grouping and your tag fields in **Settings › Library › Fields**,
for example Trip Stage, Mood and Instrument, and add as many tag fields as
you need. VirtualDJ's User 1 and User 2 come in as the first two. Each name
works as a search word too (`mood:uplifting`, `tripstage:"4 peak"`), and so
does each number (`tag1:uplifting`). The `#` in front of a tag is optional
when searching.
