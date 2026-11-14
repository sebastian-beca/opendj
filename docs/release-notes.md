openDJ 0.7.0 brings your other DJ apps' libraries in and takes them out to more players. Import djay Pro and the Music app, export USB sticks for Engine DJ players, and play Pioneer DDJ-SX, RX, 800, FLX10 and XDJ-AZ controllers. A new Mixer deck holds up to eight input channels with the mic, an instrument and the Groove Box beside them. openDJ gets a Light mode, a regrouped Settings, and big waveforms you can colour, zoom and resize.

## What's new

**Your library, everywhere**

- **djay Pro import**: tracks, hot cues, saved loops, colors and playlists from djay's media library.
- **iTunes and Music app import** from the Library XML: tags, stars, play counts, dates and playlists in their folders.
- **USB export for Engine DJ players** (Denon DJ, Numark, Rane): grids, cues, loops, overview waveforms and lists, sharing the stick's music with the Pioneer export. Experimental until it has been tried on a player.
- **rekordbox import** reads libraries with dates or empty fields that stopped it before.
- **Find missing files** looks for moved or renamed tracks in more places, closest first.
- Pioneer USB exports are checked with a second, independent reader.

**Mixer deck**

- A **Mixer** item in Jam replaces Aux 1 to 3: up to eight thin channels on one deck, each with its input picker, live meters, trim, pan, mute, solo, cue and an FX switch that sends it through the deck's effects.
- The **mic**, the **instrument plugin** and the **Groove Box** can be channels too, with a view button to open their own deck view.
- Each audio interface opens once and its channels fan out, so inputs stay in step and come back after the interface resets. A channel shows how far behind its input is heard, and marks inputs on another interface's clock.
- Slim channels on 4-Deck and 2-Deck, renamable channels, and the Xone:K2 drives channels 1 to 4 from its columns.

**Waveforms**

- **Waveform color**: RGB, 3-Band with two palettes, Blue, or Stems in each stem's pad color, with a full or half overview.
- **Show vocals** tints the sung parts (off at first).
- **Zoom** the big waveforms together with the mouse wheel, trackpad, keys or controllers, and **resize** them in ten height steps with a grip under them or the WAVE menu.
- **Size follows gain**: a deck's gain knob sizes its waveforms.
- **Edit the grid** from a pencil on each big waveform, and **Tap tempo** in the beat grid menu.
- Detected tempo is read in the octave the track's genre is known by.

**Look and layout**

- **Light mode**: Dark, Light or Auto (follows the system), with the waveforms kept on black.
- **Settings regrouped** into tabs, with Appearance and Player pages, help behind ⓘ and a Help menu.
- A **layout chooser** under the pencil in the layout menu: switch layouts on and off, reorder and rename them.
- **Tempo panel** with a Range picker that keeps the tempo when the range changes.
- **Unload a deck** with ⏏ on its cover, and Put back.
- Reorder the rail's sources and the side panel's tabs by dragging.
- Each library list keeps its scroll and selected track.
- Add-on sources search from a bar at the top, with options under a cog.
- Player search scrolls through all results, and a click plays from the track's album.

**Controllers**

- **Pioneer DDJ-SX, DDJ-SX2, DDJ-RX, DDJ-800, DDJ-FLX10 and XDJ-AZ** (Beta).
- **DDJ-400**: SHIFT + BEAT SYNC steps through all eight tempo ranges, keeping the tempo.
- Pick-up no longer snaps a control back after something else moved it.

**Sound**

- A soft clipper on headphone, booth and deck outputs, click-free effect fades and a tamer Flanger.
- Live, Groove Box and Looper decks show their level on the deck meter.

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
- Controllers marked Beta, including the Xone K2 and Maschine Mikro MK3 mappings, the Traktor Kontrol controllers, the XDJ-RX3 and Numark V7 mappings and the new Pioneer mappings, haven't all been tried on the hardware yet.
- Plugins: their latency isn't compensated. On Linux, plugin windows need X11 (XWayland on Wayland).
- The phone remote works on your local network only.
- Broadcasting: Icecast and Shoutcast only for now. Twitch, YouTube and Mixcloud Live streaming come later.
- USB export for Pioneer and Engine DJ players is experimental.
- One instrument plugin plays at a time.
