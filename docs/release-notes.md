openDJ 0.4 is about cleaning up your collection. Library Health can now merge duplicate copies, tidy up track names and genres, look tags up online, find missing covers and quarantine files you don't want. Sets and crates organise your lists. Effects move into an FX bar of their own, the 2-Deck layout gets more compact, and openDJ comes to Windows as a preview.

## What's new

**Collection clean-up**

- Duplicates in Library Health groups copies of a track and picks the one to keep: the copy in your Collection folder first, then lossless, higher bitrate, with cues or a grid, most played. Fix all easy merges every safe group at once; Keep this copy picks another; Link as versions keeps copies of different lengths, linked. Lists, history, cues, ratings, plays and missing tags move onto the kept copy, and the other files go to `_Quarantine/<date>` beside your Collection folder. Undo brings them back.
- Move to Quarantine in the track menu sends tracks you don't want to the same quarantine folder, with undo.
- Junk in names (`is:junk`) finds track numbers, web sites, "(Official Video)" and similar, underscores and double spaces, and moves remixes and featured artists into their own fields. Clean up… shows every change in a review table to tick or untick before anything is written, and every change can be undone. Each rule can be switched off in Settings → Library → Stages → Clean-up.
- A Genres page lists every genre with its track count, shows the ones spelled almost alike, and merges them.
- Look up online fills in missing tags from MusicBrainz, finding badly named files by their sound when you add an AcoustID key, and adding label, year and style from Discogs with a Discogs token. Results open in the review table, and fields you typed yourself are never changed.
- Find covers looks online for tracks with no cover, and Analyze Track now does the same, embedding the first cover at least 600 px big.
- Several music folders in Settings → Library, each with Remove. A new Outside folders check finds files in none of them. openDJ no longer scans `~/Music` unless you add it, and tracks from a folder you remove leave the library (rated, played, staged and listed tracks stay; files on disk are never touched).

**Sets and crates**

- + → New set… makes a set, a lilac folder holding Staging, Set and Live set lists (change the template in Settings → Library → Stages → Sets). A set shows all its tracks with their total length and which still need prep.
- Add to Set List on the History tab puts the tracks you played into a set's Live set.
- Crates are your own lists outside sets. A Crates column shows which crates a track is in, and `crate:name` searches them. Library Health's Lists line finds empty crates and lists holding removed tracks, and Tidy up clears them.

**Decks and effects**

- An FX bar under the decks holds all the effects, shown or hidden with the FX button in the top bar: each deck's three effects across its width, and the Beat FX under the mixer. Deck effects start as Reverb, Echo and Gate.
- Color FX on each deck's filter knob (Filter, Space, Dub Echo, Noise, Crush), picked from a small ▾ beside the knob. Each deck keeps its own.
- A more compact 2-Deck layout: tempo, pads, loop controls and the jog with PLAY and CUE under it, a narrower mixer and wider pads. PLAY, CUE and SYNC light up as an outline in the deck colour.
- SYNC sits over the tempo fader. Right-click it for quantize and its step (1/16 to 4 beats); Shift-click syncs once. A small MASTER badge under the key shows which deck the others follow; click it to pin.
- The 4-Deck layout is cleaned up: no centre mixer, bigger strip controls, and a bar under the strips with each deck's crossfader side, SYNC and tempo.
- The MASTER button in the top bar opens master, phones, booth and cue mix sliders and the Ableton Link switch; the MIC button opens the mic and aux. Levels are remembered between runs, and the aux input starts off.
- A crossfader on/off switch (Settings → Mixer or right-click the crossfader).
- Dragging a deck's overview waveform moves silently instead of playing what it passes.

**Stems**

- Every loaded track gets stems straight away, separated while it plays starting from the cue point. Prepare High Quality Stems in the track menu makes studio-quality stems in the background and swaps them in.
- The STEMS pads are now 8: Vocal, Instru, Bass, Acapella, Kick, HiHat, Stems FX and Instrumental.

**Pads, keyboard and controllers**

- A page shifter left of the pads, and pad pages in the DDJ-FLX4's order. Each FLX4 pad mode plays whatever page is in its tab, so you can put stems where the sampler was.
- Settings → Keyboard lists every shortcut to change, turn off or add. ⌘1 to ⌘5 (Ctrl on Linux and Windows) rate the selected tracks.
- Settings → Controllers lists only controllers with a mapping, with Add, Remove and Edit. The Controller Editor groups controls into sections and can duplicate and rename a mapping as your own copy.
- ⌘, (Ctrl+,) opens Settings.

**Library**

- Settings → Library has General, Stages, Fields and Import tabs. Import from VirtualDJ is on the Import tab; more apps are coming.
- Sorting by a column keeps the selected track selected and in view.
- Export to USB… on a list writes a stick for Pioneer players. It is experimental: it hasn't been tried on a player yet.

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

Add your music folders in Settings → Library. MIDI controllers that Mixxx supports load their mapping when plugged in.

## Known limits

- Windows is a preview: unsigned, and less tested than macOS and Linux.
- Streaming services and stores are coming soon.
- Automix uses decks 1 and 2 only.
- The Controller Editor can copy mappings but not change them yet.
- USB export for Pioneer players is experimental.
