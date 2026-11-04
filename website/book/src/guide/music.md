# Your collection

Your **collection** is every track openDJ knows about, with everything it
knows about them: one database file with every track's tags, rating,
color, cues, loops and beat grid, your playlists, smart lists and folders,
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
- **Instruments**: the Groove Box, your instrument plugins and the Mic and
  Aux inputs, dragged onto a deck to play there instead of a track (see
  [Instruments on a deck](decks.md#instruments-on-a-deck)).
- **Plex**, **Navidrome** and **Jellyfin**: your own music server, searched from openDJ
  with its tracks downloaded into your collection (see
  [Your own music server](#your-own-music-server)).
- **Bandcamp**, **Beatport** and **OneLibrary**: coming soon.

**Settings › Sources** lists every source by kind (streaming services,
stores, your own servers, free music, add-ons and the built-in ones). Each
row says whether the source is signed in or set up, and its **In rail**
switch shows or hides it in the rail, as right-clicking the rail does. Click
a source to open its settings: its server and sign-in, the folder its
downloads go to, or **Sign out** for a streaming service. The cache for
tracks played from your own servers is at the top of the page: one size
for all of them (10 GB unless you pick another); **Manage** under
**Settings › Collection › Storage** clears it.

### Streaming services

openDJ shows what you saved on a streaming service, but doesn't play its
audio: a track plays on a deck when your collection holds the same track,
and the others are greyed out. Double-click one to open it on the service's
own site; SoundCloud tracks and Mixcloud shows also have a **▶** button
(and **Play on…** when you right-click) that plays them there in your
browser. Tracks are matched by ISRC when both sides have one, then by
artist and title. If a match is wrong, right-click the track and pick
**Change Match…**.

To keep track of songs you mean to get, right-click a greyed-out track
(or several picked ones) and pick **Add to Wanted**. Right-click a
playlist and pick **Add to Wanted** to use it as a shopping list: its
tracks you don't have go on the Wanted list, and so does anything you add
to it later, each time openDJ signs in or you open it. Tracks you take off
the Wanted list stay off. **Stop Adding to Wanted** leaves what's there.
With a download add-on such as the Soulseek add-on installed, **Add to
Downloads** sits beside it and searches the add-on for each track.

Under the playlists, **Signed in as** shows your account; click it to
**Sign out**.

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

openDJ keeps your playlists and the tracks of those you opened, so after a
restart they show straight away. A playlist you open is checked again in
the background and updated if it changed (Spotify, TIDAL, SoundCloud and
Apple Music say when a playlist changed, so unchanged ones aren't fetched
again at all). The refresh button at the top right of the track list
fetches the list of playlists again. Signing out forgets what was kept.

### Your own music server

If your music lives on a **Plex Media Server**, on **Jellyfin** (or Emby),
or on **Navidrome** (or any other server that speaks the Subsonic API, such
as Gonic, Airsonic, Ampache or LMS), openDJ can search it and bring tracks into your collection. These
are your own files, so unlike the streaming services they play on decks.

Pick the source in the rail: until it's set up, its panel asks for the
server and sign-in, and **Connect** saves them. You can also fill them in
later in **Settings › Sources**: click **Plex**, **Jellyfin** or
**Navidrome** to open its settings.

- **Navidrome**: the server's address (for example
  `http://192.168.1.20:4533`), your username and your password. The
  password is only ever sent as a salted token.
- **Jellyfin** (or Emby): the server's address (for example
  `http://192.168.1.20:8096`), your username and your password. openDJ signs
  in once and then uses the server's access token.
- **Plex**: the server's address (for example `http://192.168.1.20:32400`)
  and your Plex token. To find the token, open any track's **⋯** menu in
  Plex Web, pick **Get Info › View XML**, and copy the `X-Plex-Token` at
  the end of the address that opens.

Then pick the source in the rail and start a **New search**: **Auto** finds
albums and songs, **Track**, **Album** and **Artist** narrow it down (Artist
lists that artist's albums), and **Playlist** lists your server's playlists
whose name has the text in it (leave it as `*` for all of them). Open an
album or playlist to see its songs.

Each song, album and playlist offers two ways to bring it in:

- **Play** (double-click a song, or its **▶**) loads it onto a deck that
  isn't playing, fetching it into the cache first. A FLAC on your home
  network takes a second or two.
- The **cloud** downloads it into the cache, to play later or offline. A
  filled cloud means it's there. Cached tracks stay out of your collection,
  and the oldest go when the cache is full (10 GB for all sources together
  unless you pick another size in **Settings › Sources**). Their waveform, tempo and key are kept as
  long as the file stays in the cache.
- **+ Collection** (or **+ Album**) brings it into your collection for good:
  `Downloaded/Plex`, `Downloaded/Jellyfin` or `Downloaded/Navidrome` in your
  music folder, one
  folder per artist and album with its cover, or the **Collection folder**
  set for the source in **Settings › Sources**. A song already in the cache moves over from
  there instead of downloading again.

Tick several rows to **Download** them to the cache or **Add to Collection**
together. openDJ fetches the original files, so a FLAC stays a FLAC.

## How tracks come in

Tracks come into your collection in three ways:

- **Your music folders.** openDJ looks through them each time it starts,
  and when you press the circling arrows at the top right of the Collection
  sidebar. New files are added, and changed files have their tags read
  again. Add as many folders as you like under **Music Collection** in
  **Settings › Collection** with **+** (pick the folder in Finder or your
  file manager), and take one out by selecting it and pressing **−**, or start openDJ with the
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

- **Inbox**: the folder where new music arrives, such as Downloads, a
  Soulseek folder, or where purchases land.
- **In Prep**: where tracks you are preparing are kept. Its
  subfolders are **prep crates** (for example `Yoga STG` or
  `Journey STG`), for sorting tracks by where they'll go.
- **Play Ready**: where prepared tracks are kept. Its subfolders
  (such as `Albums`) stay as they are.

Once the In Prep and Play Ready folders are set, **Inbox**, **In Prep**
and **Play Ready** show under **Collection** at the top of the tree, after
**Wanted**, each with its track count and subfolders. Every track gets the
stage of the folder it is in.

Right-click tracks to move them along, or drag them onto a stage. The
track menu shows **Inbox › In Prep › Play Ready** with the track's stage
lit; click another stage to move it there, or use the lines below it, whose
arrows say whether a track goes back or forward:

1. **Send to In Prep** (or **Send to In Prep › a crate**) for inbox
   tracks worth keeping. **Hide from Inbox** takes the rest out of the
   Inbox for good, without touching the file (search `stage:dismissed` to
   see them again).
2. In In Prep, prepare each track: tags, cover, rating, tempo and key. Show
   the **Prep** column (right-click a column heading) for a dot each for
   tags, rating, and tempo and key; tracks in In Prep are analysed in the
   background, so tempo and key fill in on their own. The In Prep list has
   its own columns, to see what each track still needs: **Grid** (✓ locked,
   ✓ edited, ? check when the analysis wasn't sure, the number of tempo
   sections, or detected), **Cues** (how many hot cues), **Tags** (✓ with a
   genre and at least one of your tags) and **Cover**. Pick other columns
   from the header's menu as in any list; In Prep keeps its own set.
3. **Send to Play Ready** when it's ready. A track from a prep crate also
   joins the list named after the crate (`Yoga STG` goes to **Yoga**).

**Back to In Prep** and **Back to Inbox** move a track back a stage, and
**⌘Z** (Ctrl+Z on Windows and Linux), or **Edit › Undo** on a Mac, takes
back the last change.

Files only move between the folders when **Move files** is on. Leave it
off while another app, such as VirtualDJ, still uses them: tracks change
stage in openDJ at once, their moves wait, and they are made when you turn
it on. Files going from the inbox to In Prep are renamed `Artist - Title
(Remix)` from their tags, unless you turn **Rename files** off. A file is never moved over another one.

Search with `stage:inbox`, `stage:prep` or `stage:playready` (the older
`stage:triage` and `stage:collection` work too), and
`is:prepped` for tracks with tags, rating, tempo and key.

## When tracks are analysed

Tracks in In Prep are analysed in the background. Any other track is
analysed the first time it is loaded on a deck: its waveform,
tempo and beat grid, and its key when the tags don't have one. It plays
while that happens, and the waveform fills in as it goes. The result is
saved, so the track loads at once next time.

**Storage** at the bottom of **Settings › Collection › General** shows how
the disk with your music is used: your music, the analysis results, high
quality stems, tracks played from your servers, everything else, and how
much is free. **Manage** clears the analysis results, the stems or the
played tracks, each after saying what it costs, and **Show cache folder**
opens the folder they are kept in.

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
(or **Analyze 12 Tracks** when you picked several), or right-click a
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
3. **Choose whose tags win** when a track's title, rating or color differ:
   VirtualDJ's, while you still use it, or openDJ's.

Press **Import**. Each track keeps VirtualDJ's tags, stars, color, comment,
play count, dates, BPM, beat grid, key, hot cues with their names and
colors, saved loops, gain, and its remix and automix points. Anything
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

Placeholders aren't put on the **Wanted** list for you: right-click one
(or several) for **Add to Wanted** or **Add to Downloads**, or right-click
the list in the tree to add every placeholder in it. To have every
streamed track in the app's playlists go on the Wanted list, turn on **Add
streamed tracks to Wanted** under that app in Settings › Library › Import.
New ones join at each import; tracks you take off stay off. History is
never added: what you played isn't always what you want to keep.

The Wanted list holds only what you put there: tracks you clicked Buy on
and tracks you added with **Add to Wanted**. While the stores are off, it's under Collection. The search
box narrows it. A track leaves the list once your collection has it; to
take tracks off by hand, pick them (⌘A picks all) and right-click for
**Remove from Wanted**.

## Bringing in a rekordbox library

**Import from rekordbox** works the same way. openDJ reads rekordbox 6 and
7's own database, `master.db`, which it finds where rekordbox keeps it
(`~/Library/Pioneer/rekordbox` on a Mac, `AppData\Roaming\Pioneer\rekordbox`
on Windows). For rekordbox 5, or if the database can't be opened, export
your collection from rekordbox (**File › Export Collection in xml format**)
and choose that file in the first step.

From `master.db` each track keeps its tags, stars, color, comment, play
count, date added, key, My Tags (each category a tag group, see
[Your tags](#your-tags)), hot cues A to H
with their names and colors, memory cues, saved loops, its beat grid with
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

Each track keeps its tags, stars, color, comment, play count, dates, key,
hot cues with their labels and colors, saved loops, the main cue (as a
memory cue), intro and outro (as sections) and its beat grid, whether a
steady grid or a beat map with tempo changes. Playlists come in a
**Playlists** folder, crates in a **Crates** folder, and history sessions go
to the History tab. Auto DJ's queue stays in Mixxx.

## Bringing in a Traktor library

**Import from Traktor** works the same way. openDJ reads Traktor's
`collection.nml` (only reading it), which it finds in the newest
`Documents/Native Instruments/Traktor x.y.z` folder on a Mac or Windows. If
yours is elsewhere, choose the file in the first step.

Each track keeps its tags, stars, color, comment, play count, dates, key,
hot cues 1 to 8 with their names, saved loops, its beat grid, other cues (as
memory cues) and fade in and fade out (as Automix points). Playlists come in
their folders, folders within folders just as Traktor shows them. Traktor's
own `_LOOPS` and `_RECORDINGS` lists and its smart lists stay out.

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
  them; open it for **Wanted** (tracks you went to buy or added with **Add to
  Wanted**) and the stages.
- **Crates**: lists you keep, such as Yoga, Journeys, Dance or a style.
  The **+** beside the heading adds one.
- **Playlists**: folders of lists, for gigs or anything else. The **+**
  beside the heading adds a folder; right-click a folder for **New
  Playlist** or **New Folder**, so folders can hold folders. Lists brought
  in from other apps and your smart lists are here too.

Everything starts closed, and the tree remembers which folders you left
open. Drag tracks onto a crate or playlist to add them, or onto a stage to
move them there. Or right-click tracks and point at **Add to playlist**: a
panel opens beside the menu with the lists you added to most recently at
the top. Type to find one, or pick **New crate** to make one with the
tracks in it. **Show in playlist** lists the crates and playlists a track
is in and opens the one you pick.

**Go to Artist** and **Go to Album** on the track menu show the whole
collection narrowed to that artist (any track they're on) or album, with
a chip on the filter bar; clear the chip to see everything again.

A track can be in as many crates as you like, and the **Crates** column
(right-click a heading) shows which ones it is in. Search with
`crate:yoga` for tracks in a crate with "yoga" in its name, `crate:=yoga`
for that crate only, and `crate:""` for tracks in no crate yet.

### History

Every track that plays on a deck for 30 seconds goes into that night's
history session, named after when it started (such as "Sat 31 Oct 2026,
22:15"), in the History tab beside the tree. A session opens in the order
you played it: the **#** column is the play order and **Played At** the time
each track was played. Click any heading to sort by it, and **#** to go back
to the order played. History sessions have their own columns, so changing
them doesn't change your other lists. Playlists and crates open in their own
order too, and the collection keeps its sort.

Right-click a session, playlist or crate and pick **Export to Spotify
Playlist** to make a private Spotify playlist of it in that order. openDJ
finds each track on Spotify by its ISRC, then by artist, title and length;
the list's header says how many went in and names those Spotify doesn't
have, and the new playlist opens in your browser. If you signed in to
Spotify before this was added, sign out and in again so openDJ may make
playlists.

## Collection Health

[Collection Health](collection-health.md), the cross under Collection in the
rail, finds what needs fixing in your collection: duplicates, junk in names,
missing or poor tags, covers, low bitrates, files that went missing, and
more.

## Search and filters

Search, the filter bar and smart lists have a page of their own:
[Filtering tracks](filtering.md).

## Picking several tracks

Click a track to pick it, **shift-click** another to pick every track in
between, and **⌘-click** (Ctrl-click on Windows and Linux) to add or drop
one track at a time; ⌘A picks the whole list. Right-click, drag, rate or
add to a playlist and it applies to every picked track. Sorting or a
refreshed list keeps the same tracks picked. With several picked, the list's
heading says how many and their total time ("12 selected · 1h 14m") in
place of the whole list's time.

The track menu's **Rating** and **Color** rows set every picked track at
once; the colors are the ones `color:red`, `color:blue` and so on search for.

A track's color shows as a dot in the **Color** column (headed ●, sorted
red, orange, yellow, green, cyan, blue, purple, pink). **Settings › Library ›
Track color** changes that to a strip at the row's left edge or a soft tint
of the whole row, or turns it off; color searches work either way.

## Track info

The **Info** tab on the right of the browser shows the picked track (or
the track on a deck) and lets you edit it in place. The rating is under the
title. Below, three tabs keep everything in view without scrolling:

- **Track**: track, artist, remix, album and genre, then year and track
  number, BPM and key, gain and time, two to a line.
- **Tags**: your tag groups, grouping and the comment. Each tag is a
  chip: **×** takes it off, and the **+ tag** box adds one (or several,
  with commas between), suggesting tags you already use.
- **File**: format, audio, size, the file (with Show in Finder), its folder,
  plays, when it was added and last played.

openDJ remembers the tab you were on. Tags go into the file as well as your
collection.

Pick several tracks and **Info** edits them together. A field they share
shows its value; one that differs says how many values it has. Type a value
to give it to them all, or click **▾** to pick one of the values they have
(with how many tracks have it), keep each track's own, or clear it in all.
Changed fields are marked in lilac (a tab with changes gets a dot) and wait
for **Apply** (or **Discard**).
Apply writes them to your collection and the files' tags in one go, and
⌘Z takes the whole change back. Fields you typed this way are left alone by
Clean up and Look up online. The stars set the rating of every picked track
at once.

Tags work on all the picked tracks at once, with no Apply. A tag on only
some of them shows how many have it ("dreamy 5/12"): click it to put it on
all. **×** takes a tag off all of them, and **+ tag** adds one to all.
⌘Z takes a tag change back. You can also right-click tracks and choose
**Add Tag**: it lists the tags you added lately, then the ones you use most,
and you can type to find one or make a new one.

## Fixing names

Right-click one or more tracks and choose **Fix Names…** to tidy their names
in one go. Pick a tool, choose **In**: all names (track, artist, remix and
album) or one field such as Genre or Comment, and watch the preview:

- **Find and Replace**: what to find and what to put instead (leave it empty
  to take it out). **Match case**, **Whole words**, and **Pattern** for a
  regular expression, where `$1` in the replacement puts back what the first
  brackets found.
- **Change Case**: Title Case (small words like "of" and "the" stay lower,
  DJ, MC and UK stay in capitals, names like McCartney are kept), Sentence
  case, UPPER or lower.
- **Remove Brackets**: takes out what's in (round) and [square] brackets.
- **Swap Artist and Title**: for tracks that have them the wrong way round.

**Review** opens the review table: untick what you'd rather keep, then
**Apply**. Undo takes the whole change back. A track or artist name is never
emptied.

## Columns

Right-click any column heading to choose which columns the list shows, your
tag groups included, or **Reset columns** to go back to the default. Drag a
divider on a heading's right edge to make that column wider or narrower (the
columns after it move along and keep their widths), and drag the heading itself
to move it. Click a heading to sort by it. openDJ remembers your columns.

## Your tags

Tags sit in groups you make in **Settings › Collection › Tags**, for example
Mood, Instruments or Vibe: add as many as you need, rename them, and set
each to **Several** tags per track or **One**. A tag is a name of its own,
spaces included ("Peak Time"), with no `#` needed. Removing a group takes its
tags off every track (openDJ asks first when tracks use it).

Each group's name works as a search word (`mood:uplifting`,
`vibe:"peak time"`), and so does its place (`tag1:uplifting` for the first
group); `tag:` searches every group. The `#` in front of a tag is optional
when searching. On the same tab, give Grouping the name you use it for, such
as Trip Stage (`tripstage:"4 peak"`). **Copy into a tag group** there
makes a group of that name with each track's grouping as a tag, so a track
can then have more than one (the Grouping tag itself stays as it is).

Tags from other apps come into your groups: VirtualDJ's User 1 and User 2
each fill a group (each word a tag, the `#` dropped), and each rekordbox My
Tag category becomes a group of the same name. A group you already have
with that name takes them. Rename a group made this way and later imports
still go into it.
