openDJ 0.5.1 brings a big batch of work done since 0.5.0, early from the 0.6 plans. Decks can go to their own outputs for a DAW or an external mixer, openDJ speaks MIDI clock and OSC to other gear, apps and lights, Automix mixes intro over outro on the phrases it finds, and the Groove Box grows to eight tracks with new kits. You can back up your collection, buy Wanted tracks from stores, export rekordbox XML, and the decks get SLICER, FREEZE, Stem FX, quicker scratching and a track end warning.

## What's new

**Playing with other gear and apps**

- **Deck outputs.** Settings › Audio › Outputs can send Deck 1–4 and the Sampler to their own outputs, for recording or processing each deck in a DAW such as Ableton. Pick Clean, Pre-fader or Post-fader. Settings › Audio › Mixer has an **external mixer** mode for playing through a hardware mixer.
- **MIDI clock out** follows the master deck, so drum machines and other apps stay in time. **MIDI clock in**: the sync leader can be a deck, Ableton Link or an incoming clock, and synced decks follow it on the beat.
- **OSC control.** Every control has an Open Sound Control address, with feedback, plus beat, phrase, track and hot cue events for visuals and lights. openDJ advertises itself over Bonjour.
- **App menus.** File, Edit and the rest are native menus on macOS and sit behind the ☰ button on Windows and Linux. **File › Export** holds M3U8, rekordbox XML, USB, Spotify Playlist and Mixcloud.
- **rekordbox XML export** of the collection, a list or a folder, with beat grids, hot cues, loops and colours, for rekordbox and library tools.

**Automix and phrases**

- **Intro over outro (Phrases)** mix length: the mix starts where the outgoing track's outro starts and brings the next track in over its intro, synced, so the next track's first big change lands as the mix ends. New installs start with it.
- **Better phrase detection**, using harmony, repeats and energy, with Verse sections.
- **Gapless tracks join on the exact sample**, so live albums and DJ mixes split into tracks play without a click.
- The Automix list drops the key-fit lines and tempo % between rows.

**Decks**

- **SLICER and FREEZE** pad pages: slice the next 8 beats across the pads, or freeze one beat into eighths.
- **Stem FX**: echo out a single stem (or send it to reverb), with a Stems FX pad layer.
- **Scratching** responds about twice as fast to quick changes on the jog, and the crossfader has a Scratch cut-in curve.
- **Track end warning**: the big remaining time in the deck header blinks red near the end of a playing track, or the jog ring blinks in the deck colour. Set the time and type in Settings › Decks.
- **Key lock** is on by default. The tempo fader shows a lilac line when it's on, and a right-click on the fader toggles it.
- Only PLAY blinks on a paused deck.

**Groove Box**

- **Eight tracks** with Sequencer, Synth and Mixer tabs: per-track sound, pitch, decay, tone, pan and level.
- New **Analog** and **Synthetic** kits, nine rows, soft and accent steps, and a groove library.
- **Pad controllers** can play it, starting with a Native Instruments Maschine Mikro MK3 mapping.

**Collection**

- **Back up and restore** the collection and settings to one `.opendjbackup` file, daily by default. Sign-ins aren't included.
- **Buy… on Wanted tracks** looks them up in stores (Bandcamp, Qobuz, Beatport, Bleep, Boomkat, Juno and more), opens one store page at a time, and brings purchases into the Inbox.
- **Imported lists** from VirtualDJ, rekordbox, Traktor and Mixxx sit read-only under **Imported**. **Make These Lists Mine** moves them to your Playlists.
- **Drag lists and folders** in the sidebar to move them, and new lists are named in place again.
- **History** groups sets by day (Today, Yesterday, then dates).
- **Look up online** from the Info tab's header for the track shown, and load a cover picture from a file in the Cover Art dialog.
- **⌘I (Ctrl+I)** opens Show Info. Hide from Inbox is gone, and hidden tracks are back in the Inbox; unwanted tracks go to Quarantine.
- **Albums downloaded whole** show as one item in the Inbox, and Move to Albums takes them in.
- **VirtualDJ 2024+ playlists** in MyLists are imported.
- **Add to Downloads** sends a list's songs to a download add-on, so it no longer needs its own Spotify access.
- Mixed Sets is now **Recordings**. The sources menu has Built-in and External sections, streaming sources refresh from the side panel header, and the Mixcloud track menu keeps just **Upload to Mixcloud**.

**Player layout**

- Drop a track anywhere on the Player bar to play it now. The overview, transport and search sit on the window's centre.

**Fixes**

- Saved searches run again at launch no longer open sign-in windows.
- Media server tests pass on Windows.

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
- Streaming services are browse only. Buying from stores opens the store's page; openDJ doesn't check out for you.
- Automix uses decks 1 and 2 only.
- Controllers marked Beta, including the Xone K2 and Maschine Mikro MK3 mappings, the Traktor Kontrol controllers and the XDJ-RX3 and Numark V7 mappings, haven't all been tried on the hardware yet.
- Plugins: their latency isn't compensated, and changes made in a plugin's own window aren't kept when openDJ restarts it or quits. On Linux, plugin windows need X11 (XWayland on Wayland).
- The phone remote works on your local network only.
- Broadcasting: Icecast and Shoutcast only for now. Twitch, YouTube and Mixcloud Live streaming come later.
- Some Settings descriptions are still cut short with "…".
- USB export for Pioneer players is experimental.
