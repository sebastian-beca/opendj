openDJ 0.4.5 is about switching over. Bring your rekordbox or Mixxx library across, bring your controller mappings from VirtualDJ, Traktor, rekordbox or djay Pro, or map a controller yourself by clicking a control on screen and moving one on the hardware. Beat grids now come from the Beat This! neural detector, a guided tour sets openDJ up the first time it starts, and your Spotify, TIDAL, Apple Music, SoundCloud and Mixcloud playlists show next to your collection.

## What's new

**Switching from another app**

- Settings → Collection → Import now brings in **rekordbox** (6 and 7, from `master.db` or an XML export) and **Mixxx**, next to VirtualDJ. Tags, stars, colours, hot cues with names and colours, memory cues, saved loops, beat grids and playlists in their folders come across, and Mixxx history too. rekordbox phrases become openDJ phrases, My Tags become `#tags`, and intelligent playlists become smart lists where openDJ's search can say the same thing.
- Pick which lists to import in a tree of tick boxes, in the wizard or with Lists › Choose… on the Import tab.
- A second import matches tracks the collection already has by artist, title and length, so the same song isn't added twice.
- A first-run guided tour sets up your sound (with a test tone per output), your controller and your music folders, then walks you through your first mix on the real controls, on screen or on the controller. Every step can be skipped.
- Streamed TIDAL and Spotify tracks in imported VirtualDJ and rekordbox lists stay in their place as greyed placeholders, and become the real track once it reaches your collection.

**Controllers**

- Import a mapping from VirtualDJ (controllers and keyboard), Traktor (`.tsi`), rekordbox (`.midi.csv`) or djay Pro, from the Controller Editor's Mapping menu. Rows that can't come across stay greyed with the reason. Export writes an openDJ mapping, or a Mixxx `.midi.xml`.
- Knobs and faders pick up: after a control moves on screen, the hardware does nothing until it reaches it, and a hollow marker shows where the hardware is. The crossfader always follows the hardware. Choose Scale instead per controller in Settings → Controllers.
- Your own mappings: click a row in the Controller Editor to change what the control does, make it do nothing, or put it back.
- **Map mode** (the MIDI button in the top bar): click a control on screen, then move one on the controller. Keep both, Undo and Done are on the bar.
- **Quick map** walks through a controller's usual controls one by one, for 2 or 4 decks, and copies deck 1 to the other decks. It is the button for a controller with no mapping.
- openDJ's own **Pioneer DDJ-FLX6** mapping, four decks, in Beta. Six more controllers with Mixxx mappings get drawings in the Controller Editor and a Beta label: DDJ-400, DDJ-200, DDJ-SB3, Mixtrack Platinum FX, Mixtrack Pro FX and DJControl Inpulse 500.

**Beat grids**

- Beats and downbeats come from Beat This!, a neural detector, so bar 1 lands on a downbeat. Grids stay steady unless a track really drifts, and tracks analysed before get refined on their next load.
- Slow tracks get the right tempo: the default BPM range is now 60 to 180, and ambient and acoustic tracks no longer read double. A `~` before the BPM marks a grid with tempo changes.
- Click a deck's BPM to see where its grid came from (Detected, Edited, From VirtualDJ, From Mixxx), lock it, or go back to the detected grid. Imported grids are locked.
- A synced deck stays steady when the master deck loads a new track, and a synced deck that plays on alone takes over as master.

**Streaming and sources**

- Browse your **Spotify**, **TIDAL**, **Apple Music**, **SoundCloud** and **Mixcloud** playlists, likes and favourites, signing in with your own developer app. Tracks your collection has play on a deck; the rest open in the browser. Browse only: nothing is streamed or downloaded from these services.
- Change Match… fixes a wrong match by hand, and Copy to openDJ makes a list from a playlist, with the missing tracks put on your wanted list.
- **Internet Archive**, a new source of freely licensed music with covers.
- Add-on sources such as Soulseek can download each song of a playlist search right away, one row per song, and Search again looks only for the songs still missing. Spotify playlists and tracks get Add to Downloads when an add-on takes their links. Add-ons can have on/off switches in Settings.

**Collection**

- openDJ says **collection** for your own tracks: Settings → Collection and Collection Health. Stages are now **Inbox → In Prep → Play Ready**.
- A new sidebar: **Collection** (with Wanted and your stages under it), then **Crates** and **Playlists**, with folders that nest and stay open or closed as you left them. Sets are gone for now; old set folders show as plain folders.
- Collection Health moves to the rail, with a badge counting the checks that find something. New checks: **Low bitrate** (under 192 kb/s) and **Missing or poor metadata** ("Unknown Artist", "Track 01" and the like).
- An **Albums** folder and a **Mixed Sets** folder in Settings → Collection → General, each with its own source in the rail. Albums show as a wall of covers or a list; Play Album plays one gaplessly through Automix. Collection Health finds tracks whose album tag differs from the rest of their folder.
- Drop audio files from Finder, Explorer or a file manager onto a deck to load them.
- Cmd+A (Ctrl+A) selects every track in the list.
- Settings → Audio lists inputs in a table like the outputs, with a device and channel menu for the microphone and aux.
- The search box no longer pops up the search help when you hover it.

## Install

**macOS** (Apple Silicon and Intel, macOS 11 or newer): if you have 0.3.0 or newer, openDJ offers this update itself when it starts. Otherwise download `openDJ-…-macos.zip`, unzip it and move openDJ to Applications.

**Windows** (preview; 64-bit Windows 10 and 11):

- `openDJ-…-windows-x64-setup.exe`: installs for your user, no administrator needed, and updates itself from now on.
- `opendj-…-windows-x64.zip`: portable. Unzip anywhere and run `openDJ.exe`.

The Windows build isn't signed yet, so on first run SmartScreen says "Windows protected your PC": click More info, then Run anyway. ASIO drivers show next to Windows Audio devices in Settings → Audio. Windows on ARM runs it through emulation.

**Linux** (x86_64, glibc 2.35 or newer, e.g. Ubuntu 22.04+, Fedora 36+, Debian 12+):

- `openDJ-…-x86_64.AppImage`: make it executable (`chmod +x`) and run it. AppImages from 0.3.0 on update themselves.
- `opendj-…-linux-x86_64.tar.gz`: unpack anywhere and run `bin/opendj`.

openDJ needs ALSA and a Vulkan driver, which most desktops already have.

Add your music folders in Settings → Collection, or let the guided tour do it the first time openDJ starts. MIDI controllers that Mixxx supports load their mapping when plugged in.

## Known limits

- Windows is a preview: unsigned, and less tested than macOS and Linux.
- Streaming services are browse only, and stores are coming soon.
- Automix uses decks 1 and 2 only.
- Controllers marked Beta, including the new DDJ-FLX6 mapping, haven't all been tried on the hardware yet.
- USB export for Pioneer players is experimental.
