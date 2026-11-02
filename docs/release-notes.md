openDJ 0.4.7 is a big one. You can now play your own plugin effects and instruments (CLAP, VST3 and Audio Units) inside openDJ, run Automix from your phone, and use six new effects on a smoother, beat-locked FX engine. Traktor Kontrol controllers work, decks show a beat and phrase counter, slip mode returns after every trick, and Linux gets .deb and .rpm packages.

## What's new

**Plugins**

- **Plugin effects.** Settings → Mixer → Plugins scans your plugin folders for CLAP and VST3 effects (and Audio Units on macOS). Tick the ones you want and they appear after the built-in effects in every FX chooser, on the decks, the master, Mic and Aux. The FX bar's slider controls a plugin's first setting.
- Each plugin runs in its own helper process. If it crashes or stalls, the sound carries on dry, openDJ tells you, restarts it once and switches it off if it crashes again. A plugin that keeps using too much CPU is switched off too.
- **Instrument input.** Synth plugins play through a new **Instrument** section in the MIC panel, next to Mic and Aux: pick an instrument, play it from eight on-screen pads or any MIDI keyboard, set its level, gain and EQ, and send it through the Mic FX. **KEYBOARD PADS** lets the decks' KEYBOARD pads, on screen and on controllers, play the instrument.

**Phone remote**

- A **REMOTE** button in the top bar serves a page on your local network. Scan the QR code with your phone's camera (or type the PIN) and the phone becomes an Automix manager, with no app to install.
- It opens on what's playing: a big cover, BPM and key, a progress bar with the mix shaded, Mix now, Order and play/pause. The queue works like Spotify's: drag to reorder, Play next, Move to the end, Remove, or tick several tracks at once. The track waiting on a deck can be changed until 15 s before its mix.
- Search your collection from the phone (`bpm:120-124`, `key:8A`, `crate:Warm-up` all work) and add tracks to the end of Automix or next. Set the master and booth levels, fade out, or set a sleep timer that fades the music and stops Automix.
- The remote is off until you switch it on, and pairing codes are one-time.

**Effects**

- **Six new effects:** Drive, Chorus, Ladder (a Moog-style filter), Hall (a long reverb), Rev Roll and Filter Echo (LPF/HPF Echo).
- Every knob glides instead of jumping, so turning an effect all the way doesn't click. Echo, Flanger and similar delays sound cleaner in the highs.
- Gate, Flanger, Phaser, Chorus and Roll lock to the beat grid: a gate opens on the beat, and two decks' flangers sweep together.

**Decks**

- **Beat counter** in each deck header: four beat squares, a four-bar strip that turns lilac in the last bar before a new phrase, and how many bars to the next phrase (`↓ 6 › DROP`). Settings → Decks → Beat counter switches it off or to beats only.
- **Slip mode** now returns after each trick (letting go of the jog, ending a loop, letting go of a hot cue, reverse off) and stays on. A dashed lilac line shows where the track will carry on. **SLIP** and **VINYL** buttons sit at the bottom of each deck's loop column, and « » in the jog's corners bend the pitch.
- **PLAY starts where you paused**, even with quantize on. Before, a deck could jump up to two beats to line up with the other deck.
- A paused deck blinks only the outline of PLAY and CUE, together.
- Loading a track always unmutes its stems and lets go of its Pad FX. Before, a deck that had Acapella on could play the next track as vocals only and seem to go silent.
- Beat grids: tracks whose bass plays across the beat (such as Peace Sine's "Ancient Sands") get the right tempo instead of a slow grid in sections. A tempo found by an earlier analysis no longer holds later analyses to it, and tracks analysed before this version are analysed again the next time they're loaded.

**Stems and sampler**

- The switch to high quality stems crossfades instead of clicking, a deck with stems costs nothing extra until a stem is turned down, and the stems cache is limited (10 GB by default, oldest go first). Settings → Analysis → Stems sets the limit, turns stems on load off, and shows which models are downloaded.
- The STEMS pad shows INSTANT, HQ… or HQ. Sample packs can be shared, and the sampler is lighter on the audio thread.

**Automix**

- The next track stays on the Automix list until it has played, and reordering the list swaps the waiting deck to the new top track. The deck overviews shade where the mix happens. The padlock on the crossfader is gone; it still can't be moved while Automix holds it.
- An **Order** button sorts the list once by Shuffle, BPM up or down, Harmonic path, Smooth flow or Reverse, with Undo. A mark between rows shows whether two tracks mix in key and how big the tempo jump is.

**Library**

- **History** shows the order and time each track was played (`#` and `Played At` columns), and any history session, playlist or crate can be exported to a Spotify playlist.
- **The track right-click menu** is reorganized into sections with icons. **Add to playlist ›** opens a searchable panel with your most recently used lists first and a New crate button. **Show in playlist ›** lists the lists a track is in, and Go to Artist / Go to Album jump to the rest of their tracks.
- Drop tracks from Automix, the Sidelist or a deck straight onto the open playlist or crate. **Playlists +** offers Playlist, Smart Playlist or Playlist Folder.
- **Keys** are written with sharps by default. Settings → Analysis → Key notation picks Camelot, Open Key, sharps or flats, and the Key column sorts by wheel number.
- **Metadata look-up** matches more tracks (it also searches without "(Original Mix)", "feat." and the like), ignores placeholders such as "Unknown Artist", fills genres and years from MusicBrainz, and **Fill from file name…** shows the review table before writing anything.
- Collection Health shows just its icon on the rail, without a count.

**Controllers**

- **Traktor Kontrol S2, S3, S4, F1, Z1 and MX2** work, using Mixxx's HID mappings. On Linux the .deb and .rpm set up the USB permissions for you.
- New **Pioneer XDJ-RX3** mapping (Beta), written from Pioneer's MIDI list: four decks, the mixer, Color FX and Beat FX.
- The **Numark V7** mapping gets its FX, Reloop/Exit and Tap back (Beta). Reloop/Exit and tap tempo also start working in the many older Mixxx mappings that use them.
- **Map mode** reaches the FX bar, the pad mode tabs and the headphone cue buttons. Quick map becomes the **Mapping wizard**, which asks which controller and how many decks first. Settings → Controllers works like Audio: a row per controller with its mapping, Edit and ×.
- The **Controller Editor** can run an openDJscript line instead of a function, make a button a Shift or a Mode button, and limit a row to **Only when** shift or a mode is on, so SHIFT + CUE can do something else.

**Audio**

- Settings → Audio shows the audio load and counts dropouts, and mappings can read them.
- If an audio output disappears, openDJ tells you and reopens it when it comes back, and the decks keep their place meanwhile.
- The audio thread runs at real-time priority on Linux and Windows too, as it already did on macOS.

**Linux**

- `.deb` and `.rpm` packages next to the AppImage and tarball, which install openDJ with your other apps and offer the new package when there's an update. GNOME Software and KDE Discover show a description and screenshot.

**Test builds**

- About openDJ on macOS shows the real version on test builds from CI, with the commit it was built from.

## Install

**macOS** (Apple Silicon and Intel, macOS 11 or newer): if you have 0.3.0 or newer, openDJ offers this update itself when it starts. Otherwise download `openDJ-…-macos.zip`, unzip it and move openDJ to Applications.

**Windows** (preview; 64-bit Windows 10 and 11):

- `openDJ-…-windows-x64-setup.exe`: installs for your user, no administrator needed, and updates itself from now on.
- `opendj-…-windows-x64.zip`: portable. Unzip anywhere and run `openDJ.exe`.

The Windows build isn't signed yet, so on first run SmartScreen says "Windows protected your PC": click More info, then Run anyway. ASIO drivers show next to Windows Audio devices in Settings → Audio. Windows on ARM runs it through emulation.

**Linux** (x86_64, glibc 2.35 or newer, e.g. Ubuntu 22.04+, Fedora 36+, Debian 12+):

- `opendj_…_amd64.deb` (Ubuntu, Debian, Mint, Pop!_OS) or `opendj-…x86_64.rpm` (Fedora, openSUSE): open it to install with your software centre, or run `sudo apt install ./opendj_…_amd64.deb` / `sudo dnf install ./opendj-…x86_64.rpm`. openDJ then shows up with your other apps, Traktor Kontrol controllers work without extra setup, and when a new version is out openDJ offers the new package.
- `openDJ-…-x86_64.AppImage`: make it executable (`chmod +x`) and run it. AppImages from 0.3.0 on update themselves.
- `opendj-…-linux-x86_64.tar.gz`: unpack anywhere and run `bin/opendj`.

openDJ needs ALSA and a Vulkan driver, which most desktops already have.

Add your music folders in Settings → Collection, or let the guided tour do it the first time openDJ starts. MIDI controllers that Mixxx supports load their mapping when plugged in.

## Known limits

- Windows is a preview: unsigned, and less tested than macOS and Linux.
- Streaming services are browse only, and stores are coming soon.
- Automix uses decks 1 and 2 only.
- Controllers marked Beta, including the new XDJ-RX3 and Numark V7 mappings and the Traktor Kontrol controllers, haven't all been tried on the hardware yet.
- Plugins are new: their own editor windows don't open yet (openDJ shows up to 32 of their settings), their latency isn't compensated, and their settings aren't saved between runs. Audio Units haven't been tried on a signed release yet.
- The phone remote works on your local network only.
- USB export for Pioneer players is experimental.
