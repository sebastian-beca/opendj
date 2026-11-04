openDJ 0.5 is about switching over and keeping your collection in shape. Bring your Traktor library in next to rekordbox, VirtualDJ and Mixxx, play from your own Plex, Navidrome or Jellyfin server, edit many tracks at once with your own tag groups, and see how clean your collection is on the new Collection Health dashboard. There's a Groove Box drum machine that takes the place of a deck, a Player layout that turns openDJ into a music player over Automix, hands-on tutorials inside the app, Mixcloud export for recorded mixes, and a tidier, lilac look throughout.

## What's new

**Switching over**

- **Import from Traktor.** Settings › Collection › Import reads Traktor's `collection.nml`: tracks, cues, loops, beat grids and nested playlist folders, kept as folders. It sits next to the rekordbox, VirtualDJ and Mixxx imports, and the same song in two apps becomes one track.
- **Your own tag groups.** Tags are no longer shaped like VirtualDJ's User 1 and User 2 fields. Settings › Collection › Tags holds named groups (Mood, Vibe, Set…), each allowing one or several tags per track. Your old tag words move into groups on first start, VirtualDJ's User fields and rekordbox's My Tag categories import into groups, and searches like `mood:dreamy` find them.
- **Wanted only by hand.** Imports no longer fill the Wanted list with every streamed track in your old app. Add to Wanted (and Add to Downloads, with a download add-on) is on streaming tracks and playlists, and Add to Wanted on a streaming playlist keeps adding the tracks that join it later.

**Your music servers**

- **Plex, Navidrome (and any Subsonic server) and Jellyfin (and Emby)** are sources in the rail. Search and browse albums, artists and playlists, play a track straight onto a deck from a size-capped cache, download a song, album or playlist to that cache, or add it to your collection.
- **Settings › Sources** lists every source with its status, an In rail switch and its settings. A server source shows its set-up form in its own panel until it's set up.
- A fresh install shows a shorter rail: the local sources and Spotify, with the services that need an account or a server switched on in Settings › Sources.

**Collection**

- **Collection Health dashboard.** Overview shows how much of your collection is clean, with Inbox, In Prep and Play Ready each on their own bar. The checks are grouped into five areas in the order they're best fixed: Files, Out of place, Duplicates, Info and Prep. Each area lists its tracks with a **Problem** and a **Fix** column and a progress bar, and the first area with work is marked Start here. **Auto-fix** applies the sure fixes of an area (relinking a missing file to its only match, easy duplicate merges, junk in names) in one step that Undo takes back.
- **Edit many tracks at once.** Cmd-click (Ctrl-click) and shift-click pick any set of tracks, and every track-menu action works on all of them. The Info tab edits picked tracks together, tags are chips you add and remove on one or many tracks, and **Fix Names…** finds and replaces, changes case, removes brackets or swaps artist and title, with a preview first. All of it undoes in one step.
- **Track colours** show as a dot column by default (or a strip, a tint or nothing, in Settings), and the track menu sets a colour on the picked tracks.
- **Stages in colour**: Wanted, Inbox, In Prep and Play Ready each have their colour in the sidebar, and the stage strip tops the track menu.
- **Crates** get a colour and an icon from their menu, and new playlists, crates and folders are named in place in the sidebar.
- **Albums** are grouped by artist, then album, newest first, with the year under each cover.
- **Search** is easier to reach and more forgiving: ⌘F (Ctrl+F) from anywhere, Esc clears, the box sits centred, and `olafur`, `acdc` or `dont` find "Ólafur", "AC/DC" and "Don't".
- **BPM and Key chips** in the filter bar match the playing track in one click; their ▾ opens the full filter.
- A hidden column comes back where it was, at its old width.
- Collection always updates when openDJ starts, and the refresh button at the top of the Collection sidebar rescans.

**Layouts and decks**

- **Player layout** replaces Simple: a music player over Automix. One tall bar with edge-to-edge cover art, the track overview, Order, Previous, Play, Next and **Repeat** (played tracks go to the end of the list), and what plays next. Its search box in the top bar plays a track next, adds it to the queue or plays it now.
- **Groove Box**: a 16-step drum sequencer that takes the place of a deck and goes through its EQ, filter, FX and faders. It follows the sync leader (or Link), starts on the next downbeat and stays locked. Four synthesised kits (808-, 909- and 606-style and a dub kit) and six named grooves, with swing and per-row mute, solo, level, pitch and decay.
- **Instruments** in the rail: drag a Groove Box kit, an instrument plugin, the Mic or the Aux input onto a deck.
- **SLIP** is a small word in the jog's corner, lilac when on. Vinyl mode moved to Settings › Decks (on by default); controller VINYL buttons still work per deck.
- Beat grids fitted near a whole or half BPM are rounded to it (60 instead of 59.96), and tracks are analysed again the next time they load.

**Tutorials**

- **Settings › Help › Tutorials** has five hands-on lessons that ring the real controls and wait for you: Looping, Using FX, Filtering tracks, Automix and, with one plugged in, the DDJ-FLX4.

**Recording and broadcasting**

- **Recordings show up in Mixed Sets at once**, with a tracklist file next to them.
- **Export for Mixcloud** on a mixed set makes a 320 kbps MP3 with the tracklist and chapters, and you can sign in to Mixcloud and upload it from openDJ.
- Settings › Broadcast has a **Broadcasting** switch, off by default; the ON AIR pill only shows once it's on and a station is set up. The loudness meter lives in the ON AIR panel.

**Plugins**

- **Settings › Plugins** is its own page with Effects, Instruments and Folders tabs, plugins grouped by maker, CLAP/VST3/AU chips and search. Scans run several plugins at once, so they finish sooner.
- **EDIT** opens a plugin's own window, with its presets and every setting.
- Every FX menu has **openDJ FX** and **Plugins** tabs, with effects grouped by kind (Delay & Echo, Reverb, Filter & EQ, Modulation…).
- On macOS, Audio Units open their own editor, VST3 editors sit in place, and plugin helpers run at real-time priority, so busy plugins no longer drop out.

**Controllers**

- **Allen & Heath Xone K2**: openDJ's own two-deck mapping, picked automatically: filters, gain and loop size on the encoders, FX and EQ on the knobs, hot cues, beat jumps, SYNC and PLAY on the matrix, with Shift layers. openDJ now ships mappings in its own format, and they can be duplicated in the Controller Editor like any other.
- Quick hot cue deletes and pressed browse buttons in openDJscript no longer get missed.
- The Controller Editor's press-to-find finds the SHIFT row when you hold SHIFT.

**A tidier look**

- One lilac accent everywhere, including Settings and the welcome dialog.
- New coloured icons in the rail, all the same size.
- Settings shows folders by name with + and −, short labels, and ⓘ for the longer help. Collection has General, Stages, Tags, Import and Health tabs.
- The top bar loses the MIDI and Settings buttons (Map mode starts from the Controller Editor, Settings opens with ⌘, or Ctrl+,), the Remote button is an icon, and REC shows just the dot and the time.
- The MIC panel shows the Mic and one Mic FX slot.
- Side panel and sidebar tabs are words (Automix, Sidelist, Related, Info; Lists, History), and the sidebar's heading reads Library.
- The sync MASTER badge only shows on loaded decks, the layout menu drops "Automix · Soon", the track menu's colour row fits, and the streaming footer just says who's signed in, with a refresh button at the top of the list.

## Install

**macOS** (Apple Silicon and Intel, macOS 11 or newer): if you have 0.3.0 or newer, openDJ offers this update itself when it starts. Otherwise download `openDJ-…-macos.zip`, unzip it and move openDJ to Applications.

**Windows** (preview; 64-bit Windows 10 and 11):

- `openDJ-…-windows-x64-setup.exe`: installs for your user, no administrator needed, and updates itself from now on.
- `opendj-…-windows-x64.zip`: portable. Unzip anywhere and run `openDJ.exe`.

The Windows build isn't signed yet, so on first run SmartScreen says "Windows protected your PC": click More info, then Run anyway. ASIO drivers show next to Windows Audio devices in Settings › Audio. Windows on ARM runs it through emulation.

**Linux** (x86_64, glibc 2.35 or newer, e.g. Ubuntu 22.04+, Fedora 36+, Debian 12+):

- `opendj_…_amd64.deb` (Ubuntu, Debian, Mint, Pop!_OS) or `opendj-…x86_64.rpm` (Fedora, openSUSE): open it to install with your software centre, or run `sudo apt install ./opendj_…_amd64.deb` / `sudo dnf install ./opendj-…x86_64.rpm`. openDJ then shows up with your other apps, Traktor Kontrol controllers work without extra setup, and when a new version is out openDJ offers the new package.
- `openDJ-…-x86_64.AppImage`: make it executable (`chmod +x`) and run it. AppImages from 0.3.0 on update themselves.
- `opendj-…-linux-x86_64.tar.gz`: unpack anywhere and run `bin/opendj`.

openDJ needs ALSA and a Vulkan driver, which most desktops already have.

Add your music folders in Settings › Collection, or let the guided tour do it the first time openDJ starts. MIDI controllers that Mixxx supports load their mapping when plugged in.

## Known limits

- Windows is a preview: unsigned, and less tested than macOS and Linux.
- Streaming services are browse only, and stores are coming later.
- Automix uses decks 1 and 2 only.
- Controllers marked Beta, including the Xone K2 mapping, the Traktor Kontrol controllers and the XDJ-RX3 and Numark V7 mappings, haven't all been tried on the hardware yet.
- Plugins: their latency isn't compensated, and changes made in a plugin's own window aren't kept when openDJ restarts it or quits. On Linux, plugin windows need X11 (XWayland on Wayland).
- The phone remote works on your local network only.
- Broadcasting: Icecast and Shoutcast only for now. Twitch, YouTube and Mixcloud Live streaming come later.
- Some Settings descriptions are still cut short with "…".
- USB export for Pioneer players is experimental.
