openDJ 0.4.0 brings a sampler with sample packs, phrase detection on the deck overviews, mini waveforms in the track list, key lock and fuzzy key mixing, every DDJ-FLX4 pad mode, and a Controller Editor that shows what each control does. It also fixes crackling on the filter and on jumps, and openDJ no longer uses CPU while idle.

## What's new

**Decks and mixing**

- A key lock padlock above each pitch reset triangle. Settings → Decks → Key mixing picks Harmonic (4 compatible keys) or Fuzzy (6 keys, ignoring the letter), and the browser's lilac keys, the key filter, `~key:` searches and the deck key menu all follow it. Keys are compared with what the playing deck actually sounds like, and keys a semitone away get a dim +1/−1 mark.
- Match key on load (Off, ±1, ±2; ±1 by default): a track loaded while another deck plays is shifted the smallest step that makes it mix, with key lock on.
- Auto reset on load in Settings → Decks: effects, EQ and filter, and gain go back to neutral when a new track loads. Effects keep their beats and dry/wet, and a unit still used by another playing deck is left alone.
- Turning a jog's outer rim bends the speed of a playing deck, as in Mixxx, and springs back when you let go. This works on the DDJ-FLX4 and by dragging round the rim of an on-screen jog.
- New hot cue colours for cues 1 to 8: blue, green, yellow, orange, red, pink, purple and teal, on the pads and the waveforms.
- The pitch reset triangle sits level with 0% on the tempo fader again, with key lock under it.

**Sampler**

- 16 sampler pads, shown 8 at a time on each deck's SAMPLER page with a 1/2 page button. Each pad has its own level and play mode (one shot, hold, on/off or loop).
- Sample packs: the built-in sounds are the Default DJ Sample Pack, and + makes a new one. In edit mode you choose a sound for a pad (with a headphone preview), drop your own files or tracks on it, rename, reset or clear it, and drag pads to swap them.

**Pads and controllers**

- Every DDJ-FLX4 pad mode now has a page on screen: hot cues, PAD FX 1 and 2, beat jump, sampler, beat loop (now down to 1/16 beat), keyboard, key shift and loop roll. KEYBOARD pressed twice switches to the stems pads.
- Controller Editor (Settings → Controllers): see what every control on a controller does, grouped by function, with its layer, MIDI message, LED and options. The DDJ-FLX4, Xone K2 and Midi Fighter Twister get a picture: point at a part to name it, or press a control on the controller to find it. Editing mappings comes later.
- On macOS, controllers plugged in while openDJ runs are picked up, and Settings → Controllers has a Refresh button for one that doesn't show up or stops answering.

**Waveforms and phrases**

- openDJ finds the phrases in a track (intro, build, drop, chorus, break, outro) and shows them as a coloured bar under each deck's overview. Hover it for the phrase's name, length in bars and times; right-click to show or hide phrases.
- A Waveform column in the track list and a waveform in the Info panel show each analysed track's overview. They fill in while a deck analyses the track.

**Library**

- Analyze Track in the track menu (or Analyze N Tracks for a selection), and Analyze All for a list, folder, stage or smart list. Tempo, grid, key, loudness, waveform and phrases are worked out afresh in the background; grids and keys you edited or imported stay. The footer shows progress with a stop button.
- Settings → Library → Update library on start can be switched off for big collections. The library still shows at once, and Rescan updates it.
- The Colour row is gone from the track menu. Colours you already set stay.

**Fixes and speed**

- openDJ no longer uses about 15% CPU while nothing is playing: the window only redraws when something changes.
- No more zipper noise when turning the filter, or a click when it crosses the centre.
- Beat jumps, hot cues, seeks and loop wraps crossfade smoothly instead of clicking, and with key lock on there's no longer a short silent gap after a jump.

## Install

**macOS** (Apple Silicon and Intel, macOS 11 or newer): if you have 0.3.0, openDJ offers this update itself when it starts. Otherwise download `openDJ-…-macos.zip`, unzip it and move openDJ to Applications.

**Linux** (x86_64, glibc 2.35 or newer, e.g. Ubuntu 22.04+, Fedora 36+, Debian 12+):

- `openDJ-…-x86_64.AppImage`: make it executable (`chmod +x`) and run it. 0.3.0 AppImages update themselves.
- `opendj-…-linux-x86_64.tar.gz`: unpack anywhere and run `bin/opendj`.

openDJ needs ALSA and a Vulkan driver, which most desktops already have.

openDJ reads your `~/Music` folder. MIDI controllers that Mixxx supports load their mapping when plugged in.

## Known limits

- No Windows build yet.
- Streaming services and stores are coming soon.
- Automix uses decks 1 and 2 only.
- The Controller Editor only shows mappings; it can't change them yet.

Found a bug or have an idea? Use Settings → Feedback in the app, or [open an issue](https://github.com/sebastian-beca/opendj/issues/new/choose).

## Thanks

Thanks to everyone who tested 0.3.0 and sent reports and ideas, and for the sampler request in #60.
