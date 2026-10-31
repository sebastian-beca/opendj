openDJ 0.4.6 is a quick update on top of 0.4.5. The headphone cue follows the deck you load, the window fits smaller screens such as a 14" MacBook, you can check a beat grid by ear with a metronome, Automix keeps hold of the crossfader while it mixes, and your streaming playlists are there straight away when openDJ starts.

## What's new

**Decks and mixing**

- Loading a track now cues its deck in the headphones and takes the other decks out of the cue, like VirtualDJ. Turn it off with Settings → Decks → **Headphones follow the loaded deck**. Automix loads leave the cue alone, and the room never hears the cue.
- While Automix is on, the crossfader is locked: the mouse, the controller and scripts can't move it, so a mix can't be broken by accident. It still slides with each mix, shown in Automix green with a padlock, also when the crossfader is switched off. Turning Automix off hands it back.

**Beat grids**

- A **metronome** in the deck's grid menu clicks on every grid beat, higher on bar 1, so you can check a grid by ear. The click follows the deck's fader, crossfader and headphone cue, and works on locked grids too.
- **Bar 1 here** in the grid menu makes the beat at the play position the first beat of a bar, leaving the rest of the grid where it was.

**Screen**

- The window fits smaller screens such as a 1512×982 MacBook: the source rail scrolls instead of pushing the footer off the bottom, and openDJ opens maximized when its usual size doesn't fit.
- New rail icons for Collection Health, Albums and Mixed Sets, and every rail button names itself when you hover it. Collection Health can be hidden from the rail's `•••` menu like the sources.

**Streaming and downloads**

- Spotify, TIDAL, Apple Music, SoundCloud and Mixcloud playlists are kept between runs, so they show at once on start and stay browsable offline. Opening a playlist refreshes it in the background only when it has changed, and a **Refresh** link sits beside Sign out.
- Right-clicking a Spotify track your collection hasn't got offers **Download Track** when an add-on such as Soulseek takes Spotify links. In the track menu, "Add to Downloads" is now **Download Track** (or Download N Tracks).

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
