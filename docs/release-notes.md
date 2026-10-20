openDJ 0.2.0 adds mix recording, a stronger sync, saved loops, neural key detection, a Beat FX unit, an editable track Info panel and a VirtualDJ library import, plus a long list of DDJ-FLX4 fixes.

## What's new

**Mixing**

- Record your mix: the REC button in the top bar records the master to WAV or FLAC in `~/Music/openDJ/Recordings`, with a `.cue` tracklist and a log of every control change. A crash keeps what was already recorded.
- Sync follows a leader deck and keeps the beat locked, nudges are kept, and quantized jumps and play land in phase.
- Saved loops: press an empty hot cue pad while a loop plays to store the loop there.
- Beat FX: one effect sent to deck 1, deck 2 or both, with tap tempo and a release that lets the tail ring out. It has its own FX tab in the mixer and is driven by the DDJ-FLX4's BEAT FX section.
- Automix: a 0 s gapless length, a reorderable list, drag to and from the decks, and a loop on the outgoing deck is released when a mix starts.
- A look-ahead limiter on the master at -0.5 dBFS.
- The headphone mix knob now plays cue and master equally loud at its centre.
- FX, master, phones and booth knobs start from 0 at the far left.

**Waveforms and decks**

- Drag the scrolling waveforms to scrub, scroll over the jog wheel, and see bar numbers on the waveform.
- A WAVEFORMS button in the top bar shows or hides the large waveforms.
- Shift + a hot cue pad deletes that cue.
- A layout menu in the top bar (2-Deck and Simple, with more to come).

**Library**

- Import from VirtualDJ (Settings → Library): a step-by-step dialog brings in your tracks, cues, playlists, folders and history, and can remove the import again.
- The library keeps every field other DJ apps hold, including disc number, label, composer, remixer and ISRC.
- An Info tab in the side panel: edit a track's tags in place (written to the file), see its stats, preview it in the headphones, and search for cover art. Changed covers refresh everywhere at once.
- The side panel's tabs (Automix, Sidelist, Linked, Info) are now icons.
- Neural key detection: Settings → Analysis → Key detection lets you pick Standard or Neural.

**Settings and audio**

- Buffer size and sample rate settings.
- Plugins get their own page in Settings.

**Controllers**

- DDJ-FLX4: tempo slider direction, mixer knobs, pads, LEDs, sync and load alerts fixed; quieter, slower fast search; loop buttons follow the screen.
- Every controller mapping now has its own page in the docs at [opendj.app](https://opendj.app).

## Install

**macOS** (Apple Silicon and Intel, macOS 11 or newer): download `openDJ-…-macos.zip`, unzip it and move openDJ to Applications. The app isn't notarised yet, so macOS blocks the first launch. Either right-click openDJ and choose Open, or run:

```sh
xattr -dr com.apple.quarantine /Applications/openDJ.app
```

**Linux** (x86_64, glibc 2.35 or newer, e.g. Ubuntu 22.04+, Fedora 36+, Debian 12+):

- `openDJ-…-x86_64.AppImage`: make it executable (`chmod +x`) and run it.
- `opendj-…-linux-x86_64.tar.gz`: unpack anywhere and run `bin/opendj`.

openDJ needs ALSA and a Vulkan driver, which most desktops already have.

openDJ reads your `~/Music` folder. MIDI controllers that Mixxx supports load their mapping when plugged in.

## Known limits

- Builds aren't signed yet.
- No Windows build yet.
- Streaming services and stores are coming soon.

Found a bug or have an idea? Use Settings → Feedback in the app, or [open an issue](https://github.com/sebastian-beca/opendj/issues/new/choose).

## Thanks

Thanks to everyone who tested 0.1.0 and sent reports and ideas.
