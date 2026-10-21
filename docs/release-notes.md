openDJ 0.3.0 brings four decks, microphone and aux inputs, Library Health and stages for sorting new music, a much more flexible track browser, and in-app updates. It's also the first release whose macOS app is signed and notarized by Apple.

## What's new

**Decks and mixing**

- Four decks: a djay-style 4-Deck layout (in the layout menu) shows four strips in the order 3, 1, 2, 4, each with its own EQ, fader, transport, loops, sync, tempo, pads and FX. Every deck has a crossfader assignment (left, thru, right), and sync picks its leader among all four.
- Two-deck controllers such as the DDJ-FLX4 get deck 3/4 switches: click a strip's deck number to move that side of the controller there, LEDs included.
- Microphone and aux inputs: a MIC tab in the mixer and a MIC button in the top bar. The mic has its own gain, EQ, fader and an effect unit of its own (Reverb, Echo, Filter), and talkover ducks the music automatically or by hand. Both inputs are in recordings.
- A VirtualDJ-style pitch reset triangle beside each tempo fader: click to glide back to 0%, click again to stop, double-click to jump straight to 0%. Controller mappings can use it too (`rate_reset`, `rate_set_zero`).
- The scrolling waveform glides smoothly between pixels instead of stepping, and bar numbers are drawn sharp at any position.

**Library**

- Stages: Inbox, Triage and Collection. Pick the folders in Settings → Library → Stages, then send tracks along with the track menu or by dropping them on a stage. Moves can be undone, tracks in Triage are analysed in the background, and files only move on disk when you turn Move files on.
- Library Health: pages for not music, strays, duplicates, tracks not analysed, missing info, no cover, no rating, missing files and long tracks, each with batch fixes such as fill from file name, find moved files, analyse or remove (your files are kept).
- A Prep column shows each track's tags, rating, tempo and key at a glance.
- A filter bar with chips for genre, BPM (a range or near the playing track), key (or keys that mix with the playing track), rating, colour, year, date added and more. Filters combine with the search and save as smart lists.
- Name your own tag fields in Settings → Library and search them by name, such as `mood:uplifting`.
- Choose, resize and reorder the list columns: right-click a heading for the column menu, drag a heading to move it, drag its edge to resize.
- A refreshed browser: alternating row tones, a lilac highlight in the focused list, lilac compatible keys and slim scrollbars. Hover the search box for the search help.

**Add-ons**

- Add-ons now run as separate programs from the add-ons folder, so one such as Soulseek installs without rebuilding openDJ.
- A source's searches are listed by the name it gives them, such as a playlist's name.
- Click anywhere on the Downloading bar to open the downloads list.

**Updates and macOS**

- openDJ checks for a new release when it starts and installs it in one click on macOS and the AppImage, then restarts once the decks stop. It never shows while a deck is playing, and Settings → Feedback can turn the check off.
- The macOS app is signed and notarized, so it opens without any workarounds.
- macOS now asks for microphone access, and Settings → Audio → Inputs warns when an input sends only silence.

**Fixes**

- Close buttons show a proper X instead of something that looked like a menu icon.
- Security updates for the TLS certificate libraries.

## Install

**macOS** (Apple Silicon and Intel, macOS 11 or newer): download `openDJ-…-macos.zip`, unzip it and move openDJ to Applications. If you have 0.2.0 or older, replace it by hand this once; from 0.3.0 on openDJ updates itself.

**Linux** (x86_64, glibc 2.35 or newer, e.g. Ubuntu 22.04+, Fedora 36+, Debian 12+):

- `openDJ-…-x86_64.AppImage`: make it executable (`chmod +x`) and run it. It updates itself from now on.
- `opendj-…-linux-x86_64.tar.gz`: unpack anywhere and run `bin/opendj`.

openDJ needs ALSA and a Vulkan driver, which most desktops already have.

openDJ reads your `~/Music` folder. MIDI controllers that Mixxx supports load their mapping when plugged in.

## Known limits

- No Windows build yet.
- Streaming services and stores are coming soon.
- Automix uses decks 1 and 2 only.

Found a bug or have an idea? Use Settings → Feedback in the app, or [open an issue](https://github.com/sebastian-beca/opendj/issues/new/choose).

## Thanks

Thanks to everyone who tested 0.2.0 and sent reports and ideas.
