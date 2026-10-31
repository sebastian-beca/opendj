# Your collection

Your **collection** is every track openDJ knows about, with everything it
knows about them: one database file with every track's tags, rating,
colour, cues, loops and beat grid, your playlists, smart lists and folders,
and the history of what you played. It points at your audio files where
they are, and only moves them between your stage folders when you turn that
on (see [Stages](#stages-inbox-in-prep-and-play-ready)).

**Collection** is the top of the tree on the left, and the first source in
the rail. Together with Files and the streaming services it makes up the
**browser**, the lower half of the screen.

## Sources

The rail at the far left picks where the tree and the track list come from:

- **Collection**: all your tracks, with their stages, crates, sets, playlists
  and history.
- **Files**: any folder on your computer, as it is on disk.
- **Spotify**, **TIDAL**, **Apple Music**, **SoundCloud** and
  **Mixcloud**: your playlists and saved tracks on those services, for
  browsing (see [Streaming services](#streaming-services)).
- **Bandcamp**, **Beatport** and **OneLibrary**: coming soon.

### Streaming services

openDJ shows what you saved on a streaming service, but doesn't play its
audio: a track plays on a deck when your collection holds the same track,
and the others are greyed out. Double-click one to open it on the service's
own site; SoundCloud tracks and Mixcloud shows also have a **▶** button
(and **Play on…** when you right-click) that plays them there in your
browser. Tracks are matched by ISRC when both sides have one, then by
artist and title. If a match is wrong, right-click the track and pick
**Change Match…**. **Copy to openDJ** makes a list of a playlist in your
collection, once; tracks you don't have are kept as wanted, for the
stores to find later.

Each service is signed in to with your own app, so openDJ ships no keys:

- **Spotify**: create an app at developer.spotify.com with the redirect URI
  `http://127.0.0.1:8898/callback` and paste its client ID.
- **TIDAL**: create an app at developer.tidal.com with the same redirect
  URI and the scopes `user.read`, `playlists.read` and `collection.read`,
  and paste its client ID.
- **Apple Music**: needs an Apple Developer account. Under **Certificates,
  Identifiers & Profiles › Keys**, create a key with **MusicKit** and
  download its `.p8` file. Enter your Team ID, the key's Key ID and the
  file's path, then sign in with your Apple ID on the page that opens. You
  see your library's songs and playlists.
- **SoundCloud**: register an app at soundcloud.com/you/apps with the same
  redirect URI and paste its client ID and secret. You see your Likes, your
  own tracks and your playlists.
- **Mixcloud**: just type your username. You see your shows, favourites,
  listening history and playlists; shows play on mixcloud.com.

## How tracks come in

Tracks come into your collection in three ways:

- **Your music folders.** openDJ looks through them each time it starts,
  and when you press **Rescan** in **Settings › Collection**. New files are
  added, and changed files have their tags read again. With a big collection
  you can turn off **Update collection on start** there, and press **Rescan**
  when you add music. Add as many folders as you like there with
  **Add…**, and take one out with **Remove**, or start openDJ with the
  `OPENDJ_MUSIC` environment variable set to one. There's no default folder:
  `~/Music` is where Ableton, GarageBand and other apps keep their samples,
  so openDJ only looks there if you add it. When a folder is no longer looked through,
  its tracks leave your collection, except ones you rated, played, staged or put
  in a list. The files stay where they are.
- **Files.** The **Files** source in the rail browses any folder on your
  computer. Load or drag a track from there to add it.
- **Another DJ app.** openDJ can bring in your VirtualDJ library (see below).

Bringing a track in only reads its tags: title, artist, album, genre, year,
key, BPM, rating, comment and cover art. It is quick, and nothing is played
or analysed yet.

## Stages: Inbox, In Prep and Play Ready

Stages take new music from wherever it lands to tracks that are ready to
play. Set them up in **Settings › Collection › Stages**:

- **Inbox folders**: any number of folders where new music arrives, such
  as Downloads, a Soulseek folder, or where purchases land.
- **In Prep folder**: where tracks you are preparing are kept. Its
  subfolders are **prep crates** (for example `Yoga STG` or
  `Journey STG`), for sorting tracks by where they'll go.
- **Play Ready folder**: where prepared tracks are kept. Its subfolders
  (such as `Albums`) stay as they are.

Once the In Prep and Play Ready folders are set, **Inbox**, **In Prep**
and **Play Ready** show under **Collection** at the top of the tree, after
**Wanted**, each with its track count and subfolders. Every track gets the
stage of the folder it is in.

Right-click tracks to move them along, or drag them onto a stage:

1. **Send to In Prep** (or **Send to In Prep › a crate**) for inbox
   tracks worth keeping. **Not interested** hides the rest.
2. In In Prep, prepare each track: tags, cover, rating, tempo and key. Show
   the **Prep** column (right-click a column heading) for a dot each for
   tags, rating, and tempo and key; tracks in In Prep are analysed in the
   background, so tempo and key fill in on their own.
3. **Send to Play Ready** when it's ready. A track from a prep crate also
   joins the list named after the crate (`Yoga STG` goes to **Yoga**).

**Back to In Prep** and **Back to Inbox** move a track back a stage, and
the track menu's **Undo** line takes back the last change.

Files only move between the folders when **Move files** is on. Leave it
off while another app, such as VirtualDJ, still uses them: tracks change
stage in openDJ at once, their moves wait, and they are made when you turn
it on. Files going from the inbox to In Prep are renamed `Artist - Title
(Remix)` from their tags, unless you turn **Rename files sent to In Prep**
off. A file is never moved over another one.

Search with `stage:inbox`, `stage:prep` or `stage:playready` (the older
`stage:triage` and `stage:collection` work too), and
`is:prepped` for tracks with tags, rating, tempo and key.

## When tracks are analysed

Tracks in In Prep are analysed in the background. Any other track is
analysed the first time it is loaded on a deck: its waveform,
tempo and beat grid, and its key when the tags don't have one. It plays
while that happens, and the waveform fills in as it goes. The result is
saved, so the track loads at once next time; **Clear** in **Settings ›
Collection** throws those results away.

Grids, keys and cues brought in from VirtualDJ are used as they are, so
those tracks don't wait for analysis.

The beat grid stays at one steady tempo unless the track really drifts,
as live drums and older recordings do; only then does it get tempo
sections, and its BPM in the list shows a ~ (as in VirtualDJ). With
**Beat detection** set to **Neural** (the default, in
**Settings › Analysis**), openDJ then listens for each beat and downbeat
with Beat This!, so bar 1 starts on a downbeat. That takes a few seconds
more, after the waveform appears: the track plays on its first grid
meanwhile. The first time, openDJ downloads the model (about 11 MB).

Beat This! also sets the tempo, from the beats it hears, which suits slow
and soft music (ambient, acoustic) far better than the standard detector.
Tempos are kept within **BPM range** (**Settings › Analysis**, 60 to 180
BPM unless you change it): a faster reading is halved, a slower one
doubled.

The grid menu (click the BPM on a deck) says where the grid came from:
**Detected**, **Edited** or the app it was imported from. A detected grid
that fits the beats poorly says **may be off, check by ear**. **Lock grid**
keeps a grid as it is: a locked grid can't be edited, and analysing the
track again leaves it alone. Grids imported from another app come in
locked. **Back to the detected grid** throws away your edits or the
imported grid and uses openDJ's own.

To check a grid by ear, turn on **Metronome** in the same menu: the deck
clicks on every beat of the grid, higher on bar 1, through the deck's
fader and cue like the music. If the clicks drift off the music, fix the
grid while it plays: **Bar 1 here** on a downbeat puts bar 1 there,
**Move grid** ‹ › nudges it, **Tempo here** − + changes the tempo, and
**New tempo section from here** starts a new tempo where a live track
speeds up or slows down.

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
long, so a short fill never gets one of its own. They are saved in your
collection with the track, and move with the grid when you edit it.

Rest the pointer on a deck's overview to see the phrase under it: its
kind, how many bars it runs and where it starts and ends.

Right-click a deck's overview and choose **Hide phrases** (or **Show
phrases**) to turn the bars off or on for every deck.

## Bringing in a VirtualDJ library

Press **Set up…** next to **Import from VirtualDJ** in **Settings ›
Collection › Import**. That tab is always there and lists every DJ app openDJ
can bring a library in from; Serato DJ and Traktor are marked coming soon.
Three steps follow:

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

Streamed tracks (TIDAL and VirtualDJ's other online catalogues, or TIDAL,
Spotify and the other streaming services in rekordbox) have no file to
play, so they stay in their lists as placeholders, in their place. A
placeholder is greyed out and shows its service (**TIDAL**) where the album
would be. It keeps the artist, title and length the app saved, and the
service's own track id. Double-click one to open it on the service's site.
Lists made only of streamed tracks come in too, marked **online only** in
the list picker.

When the song comes into your collection later (you buy it and it lands in
the inbox, or a scan finds the file), the placeholder becomes that track,
in the same place. openDJ matches by ISRC when both sides have one, then by
artist and title at the same length (within 2 seconds). While you're signed
in to TIDAL or Spotify, openDJ asks the service for its tracks' ISRCs, so
the match is sure. If a match is wrong, or one is missed, right-click the
track and pick **Change Match…**.

Placeholders with a page on their service also wait on the **Wanted** list.
While the stores are off, the Wanted list is under Collection. A track
leaves the list once your collection has it.

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
## Bringing in a Mixxx library

**Import from Mixxx** works the same way. openDJ reads Mixxx's
`mixxxdb.sqlite` (only reading it, so Mixxx can stay open), which it finds
where Mixxx keeps it: `~/Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx`
or `~/Library/Application Support/Mixxx` on a Mac, `~/.mixxx` on Linux and
`AppData\Local\Mixxx` on Windows.

Each track keeps its tags, stars, colour, comment, play count, dates, key,
hot cues with their labels and colours, saved loops, the main cue (as a
memory cue), intro and outro (as sections) and its beat grid, whether a
steady grid or a beat map with tempo changes. Playlists come in a
**Playlists** folder, crates in a **Crates** folder, and history sessions go
to the History tab. Auto DJ's queue stays in Mixxx.

### The same song from two apps

When a second app's library has a different file of a song you already have
(the same artist and title, ignoring case and punctuation, and a length
within two seconds), openDJ doesn't add it twice. The copy already in your
collection stays, and the second app only fills in what it left empty. Its
lists point at that copy. Its grid is only used when the track has none from
another app, and its hot cues come in as memory cues when the track already
has hot cues, so nothing is lost. Removing the second import takes out what
it added and leaves the track as it was.

## The tree

The collection's tree has three parts:

- **Collection**, with how many tracks you have. Select it for all of
  them; open it for **Wanted** (tracks you went to buy, or streamed tracks
  in imported lists you don't have a file for yet) and the stages.
- **Crates**: lists you keep, such as Yoga, Journeys, Dance or a style.
  The **+** beside the heading adds one.
- **Playlists**: folders of lists, for gigs or anything else. The **+**
  beside the heading adds a folder; right-click a folder for **New
  Playlist** or **New Folder**, so folders can hold folders. Lists brought
  in from other apps and your smart lists are here too.

Everything starts closed, and the tree remembers which folders you left
open. Drag tracks onto a crate or playlist to add them, or onto a stage to
move them there.

A track can be in as many crates as you like, and the **Crates** column
(right-click a heading) shows which ones it is in. Search with
`crate:yoga` for tracks in a crate with "yoga" in its name, `crate:=yoga`
for that crate only, and `crate:""` for tracks in no crate yet.

## Collection Health

[Collection Health](collection-health.md), the heart under Collection in the
rail, finds what needs fixing in your collection: duplicates, junk in names,
missing or poor tags, covers, low bitrates, files that went missing, and
more.

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

Name grouping and your tag fields in **Settings › Collection › Fields**,
for example Trip Stage, Mood and Instrument, and add as many tag fields as
you need. VirtualDJ's User 1 and User 2 come in as the first two. Each name
works as a search word too (`mood:uplifting`, `tripstage:"4 peak"`), and so
does each number (`tag1:uplifting`). The `#` in front of a tag is optional
when searching.
