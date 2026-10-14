# openDJ

A DJ application in Rust, compatible with [Mixxx](https://mixxx.org).

openDJ keeps Mixxx's public contracts (control names such as `[Channel1],play`,
the library database and controller mapping formats) and rebuilds the rest in
Rust: a [Slint](https://slint.dev) UI, GPU waveforms on [wgpu](https://wgpu.rs),
and a real-time engine that never locks or allocates on the audio thread.

![openDJ](docs/screenshot.png)

## Status: phase 2, real audio and controllers

Two decks play real audio through the mixer to your sound card, driven from the
screen or a MIDI controller.

- **Library**: scans `~/Music` (or the folder in `OPENDJ_MUSIC`) for MP3, FLAC, WAV,
  AIFF, OGG and M4A/AAC files and reads their tags. With no music folder, a built-in
  sample library of synthesised dance tracks is used, so it always makes sound.
- **Loading**: double-click a track or drag it onto a deck (both decks outline while
  you drag; audio files dragged in from Finder or a file manager work too). As in
  Mixxx, the track can play as soon as its file is open; the rest decodes in the
  background while the waveform fills in, and the tempo (or the BPM tag) and first
  beat follow. The analysis is cached (`~/Library/Caches/openDJ` on macOS,
  `~/.cache/opendj` on Linux), so a track loaded again shows everything at once.
- **Saved library**: the collection lives in `library.sqlite` in the settings folder
  (`~/Library/Application Support/openDJ` on macOS, `~/.config/opendj` on Linux).
  Make lists and folders with + or a right-click, drag tracks onto them, rename and
  delete them; set ratings and colours from a track's menu. Hot cues, beat grids,
  play counts and a history of every session (the clock tab beside the lists) are
  kept between runs. Scanning again adds new files and keeps what you set.
- **Search and smart lists**: the search box speaks Mixxx's search syntax (see
  [Searching the library](#searching-the-library)), so `bpm:120-128 ~key:8A -genre:techno`
  works as it does in Mixxx. *Save* beside the box keeps a search as a smart list,
  which stays up to date as the library changes; right-click a smart list to change
  its search. A new library starts with a few: *Mixes with deck*, *Recently added*,
  *Top rated*, *Never played*, *Duplicates* and *Missing files* (tracks whose file
  was moved or deleted; they show with a ! and only in that list).
- **Beat grids that follow the music**: analysis finds tempo changes (live drums,
  edits, old disco) and lays the grid in sections. Click a deck's BPM to fix the grid by hand: set the beat
  under the play position, move the grid, nudge or halve/double the tempo, add or
  remove a section, and undo. Loops, beat jumps, quantize and sync all follow the
  sections. Controllers can use Mixxx's `beats_translate_*` and `beats_adjust_*`.
- **Stems**: NI stem files (`.stem.mp4` / `.stem.m4a`) play as four stems, named and
  coloured as the file says. The STEMS pad page mutes each one and sets its level;
  controllers use Mixxx 2.6's `[ChannelN_StemM],volume` / `mute`. Any other track can
  be split from the same page with [Demucs](https://github.com/adefossez/demucs), in
  the background, if it is installed (`pip install demucs`, or point
  `OPENDJ_DEMUCS` at it); the stems are cached and load by themselves next time.
  Splitting takes a few minutes per track on a laptop CPU.
- **Ableton Link**: the LINK button at the top left joins the Link session on your
  network (Ableton Live, Traktor, phone apps, drum machines) and shows its
  tempo, the deck it follows and how many apps are in it. Link follows the
  master deck: the session takes the tempo and beat of the deck
  that is playing (the one that started first while both play), so the other apps
  play along with the mix. Controllers can use `[Link],enabled` and
  `[Link],master_deck`.
- **Album art and key**: a loaded track shows its album art, found as Mixxx finds it
  (the front cover in the file's tags, else an image beside the file named after the
  track, its album, or cover/front/album/folder). Its key is detected during analysis
  with the Queen Mary method Mixxx uses (a key tag wins). Click a deck's key to switch
  every key display between Camelot ("8A") and notes ("Am").
- **Key lock and key shift**: key lock (on controllers) keeps the key when the tempo changes; the ‹ › buttons and the menu
  beside the key shift it by semitones (marking keys that mix with the other deck) or
  matches it to the other deck, as Mixxx's `pitch`, `pitch_up`/`_down`, `sync_key` and
  `reset_key` controls do. Both run through the Signalsmith Stretch time-stretcher.
- **Decks**: play, cue, hotcues, beat loops, loop in/out, reloop, beat jumps, pitch with
  ±8% range, sync (tempo and phase) and quantize, all sample-accurate.
- **Mixer**: gain, 3-band isolator EQ (kills on controllers), low/high-pass filter knob, channel
  faders, constant-power crossfader, main gain and VU meters. The master, headphone (cue)
  and booth mixes go to any channel pairs of any devices.
- **Effects**: four effect units of three slots each, with meta knobs, super knob and
  dry/wet. Unit 1 is on deck A and unit 2 on deck B. Twelve effects, all beat-synced
  where it makes sense: Echo (1/16 to 16 beats, ping-pong, darkening tail), Reverb (a
  Dattorro plate), Filter (one knob, low-pass left and high-pass right), Flanger,
  Bitcrusher, Phaser, Gate, Roll, Echo Out, Brake, Noise and Spiral. The audio thread
  flushes denormals to zero, so decaying tails don't cost CPU.
- **MIDI controllers**: plug in a controller and its Mixxx mapping loads by itself
  (see below). Jog wheels scratch and nudge, LEDs follow the decks.

Not there yet: headphone effects routing, HID
controllers, and reading Mixxx's own library database.

## Streaming services and stores

The rail down the left of the browser also lists Spotify, TIDAL, Apple Music,
Bandcamp, Beatport, OneLibrary, SoundCloud and other stores. They are coming
soon: for now the collection and Files are the sources that work. The "..."
under the rail hides the ones you don't use.

## Searching the library

The search box and smart lists use Mixxx's search syntax, with a few additions
(the ? beside the box lists them):

| Search | Finds |
|---|---|
| `daft punk` | words in the artist, title, album, genre, comment, grouping or tags |
| `artist:"daft punk"` | one field: also `title:`, `album:`, `genre:`, `comment:`, `grouping:`, `tag:`, `location:` |
| `title:=glue`, `album:""` | the whole field; an empty field |
| `bpm:120-128`, `rating:>=4`, `played:0` | numbers, with `>`, `>=`, `<`, `<=` or a range |
| `duration:>6m`, `duration:<3:30` | length |
| `added:<7d`, `added:>=2026-01-31` | added in the last days or weeks, or since a date |
| `~key:8A`, `~bpm:124` | keys that mix with 8A; tempos within 6%, also at half or double tempo |
| `~deck` | tracks that mix with the one playing now, by key and tempo |
| `color:red`, `is:duplicate`, `is:missing`, `is:cued` | colour; duplicates (same artist and title, lengths within 3 s); missing files; tracks with hot cues |
| `-genre:techno` | leaves out what matches |
| `glue \| daft`, `glue OR daft` | either |

Search runs over the whole collection in memory as you type, with each track's
text lowercased once when the library loads.

## MIDI controllers

openDJ runs Mixxx's own controller mappings unchanged: the `.midi.xml` files and
their JavaScript, bundled in `res/controllers` (and inside the macOS app). A connected
MIDI device is matched to a mapping by its port name; the Controllers page of the
settings window lists what is connected and lets you pick another mapping or turn a
device off. Tested against the Pioneer DDJ-FLX4, Allen & Heath
Xone K2 and DJ TechTools MIDI Fighter Twister mappings.

Xone K2: with two decks, set the K2 to MIDI channel 14 (deck A, FX 1, FX 2, deck B
across the columns). The default, channel 15, gives each column a deck, for four decks.

| Variable | Effect |
| --- | --- |
| `OPENDJ_CONTROLLERS` | Folder of mappings to use instead of the bundled ones |
| `OPENDJ_MIDI_DEBUG=1` | Prints every MIDI message received |
| `OPENDJ_UI_FONT` | A font file to use for the interface instead of the default (San Francisco on macOS, Inter elsewhere); its digits are made one width |
| `OPENDJ_VIRTUAL_MIDI` | `Name=/path/to/fifo;...` adds a fake device that reads hex MIDI lines from a FIFO, for testing without hardware |

The interface:

- Scrolling waveforms for both decks across the top
- Decks with cover, title, key and overview (click or drag on it to glide through the track); a jog circle with tempo, pitch and times; pitch fader; CUE and SYNC above a wide play button that blinks while stopped
- Per-deck effects (click a slot's name to turn it on, the arrow beside it to pick the effect); pad pages for hot cues, Pad FX (hold a pad for an effect on the deck's pad FX unit, 3 for deck A and 4 for deck B), beat jumps and the sampler (eight built-in one-shot sounds on `[Sampler1]`–`[Sampler8]`); a loop stepper (halve, start/stop, double) with IN and OUT under it: IN on the nearest beat shades the waveform light grey as the loop is marked out, OUT closes it and the loop turns lilac
- Centre mixer with gain, 3-band EQ with kills, filter, channel faders, VU meters, headphone cue and crossfader
- Browser with a folder tree (lists, nested sets, smart lists), searchable and sortable track list, and an Automix / Sidelist panel
- Automix: press AUTOMIX and the Automix list plays on decks A and B, each track crossfading into the next where its sound ends. Mix length is 12 s, 15 s, 30 s, 16 bars or 32 bars (bar lengths start on a bar and beat-match the incoming track when the tempos are within the pitch range); the style is Smooth, Bass swap, Filter (the outgoing track is high-passed away) or Cut. If a deck is already playing, Automix carries on from the loudest one and queues the list on the other. Moving the crossfader during a mix hands it back to you. Switching Automix off slides the crossfader back to the centre. Add tracks by dragging them onto the panel or with Add to Automix in a track's right-click menu; click a row and press Delete, or right-click it, to take it out. Mixxx's `[AutoDJ]` controls (`enabled`, `fade_now`, `skip_next`) work from controllers
- Waveforms are rendered by our own wgpu shaders into textures that Slint displays: a
  1000-slices-per-second analysis with peak and RMS for each band, coloured by frequency
  (bass red, mids amber, highs blue), drawn at full screen resolution and scrolled
  smoothly from the engine's position and speed. `OPENDJ_WAVEFORM=bands` draws the
  three bands as separate stacked shapes instead

## Layout

| Crate | What it does |
| --- | --- |
| `crates/control` | Named controls. A name is resolved once to a handle; reads and writes are single atomic operations |
| `crates/engine` | Audio engine: cpal output, decks, EQ, filter and effects DSP, Symphonia decoding, tempo and waveform analysis, folder scan (Lofty tags) |
| `crates/controllers` | MIDI devices (midir), Mixxx mapping XML, and the mapping scripts run in QuickJS with Mixxx's `engine` and `midi` API |
| `crates/waveform` | wgpu waveform renderer (scrolling and overview modes) |
| `crates/app` | The application: Slint UI (`ui/*.slint`) wired to the above |

UI widgets bind to controls by name, e.g. `ControlKnob { group: "[Channel1]"; key: "pregain"; }`,
which is the basis for user skins later on.

## Build and run

Needs a recent stable Rust, CMake and a C++ compiler (for Ableton Link) and, on Linux,
fontconfig, xkbcommon, ALSA and libclang development packages
(`sudo apt install cmake libfontconfig-dev libxkbcommon-dev libasound2-dev libclang-dev`).

```sh
cargo run --release
OPENDJ_MUSIC=/path/to/music cargo run --release   # scan another folder
```

The gear at the right of the top bar (or ⌘, / Ctrl+,) opens the settings window:
Audio, Controllers, Library, Analysis, Decks and Mixer pages down the left, and a
search box that finds an option on any page. Changes apply and are saved to
`settings.json` at once.

Outputs are set on the Audio page of the settings window (and on the SETTINGS tab
of the centre panel), as a table: each row sends the master, headphone or booth mix to a pair of
channels on a device, and + adds a row. The choice is remembered. The master
row's device runs the engine; other devices have to run at its sample rate. The
same tab has the cue mix, cue level and booth level knobs and lists the
connected controllers. Without any device (or with `OPENDJ_NO_AUDIO=1`)
the engine runs silently on a timer.

The UI renders through wgpu (Vulkan, Metal or DirectX 12). On a machine with only
a software Vulkan driver, set `SLINT_WGPU_CPU=1`.

On macOS the interface uses the system font, San Francisco,
read from the system at start with its digits switched to their same-width
forms (nothing is bundled). Elsewhere it uses the bundled Inter Tabular.

```sh
cargo test --workspace
```

## Licence

openDJ's source is GPL-2.0-or-later, the same as Mixxx. See [LICENSE](LICENSE) and
[NOTICE](NOTICE). The app is built with [Slint](https://slint.dev), which is licensed
under GPL-3.0, so a built copy of openDJ (such as the macOS app) is distributed under
GPL-3.0, which the "or later" allows.
openDJ is not affiliated with or endorsed by the Mixxx project.
The openDJ name is set in Zen Dots by Yoshimichi Ohira (SIL Open Font License).

### Trying it on macOS without Rust

Every CI run attaches `opendj-macos`, an unsigned Apple Silicon app. Download
it from the run's Summary page, unzip it, then clear the download quarantine
before the first launch:

```sh
xattr -dr com.apple.quarantine openDJ.app && open openDJ.app
```
