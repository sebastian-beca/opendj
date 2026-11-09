# openDJ roadmap

The open OpenSpec changes in `openspec/changes/`, in the order they are planned (within a release, in build order). Each change's `proposal.md` says why, and its `specs/` say what will be true when it's done. When a change ships, archive it (`openspec archive <name>`) so `openspec/specs/` takes its requirements.

This file only says which release each change is planned for, and in what order. How far a change has got is not written here: tick its steps in `changes/<name>/tasks.md` and run `scripts/roadmap.py` (or `scripts/roadmap.py v0.6`) to see every change as not started, in progress or built. The notes after a link say what the change is and why it sits in that release, not its progress, so a PR that moves a change forward doesn't edit this file. `scripts/roadmap.py --check` (run in CI) makes sure every change folder is listed, and listed once per release.

Released so far: v0.1.0 and v0.2.0 (2026-10-15, 2026-10-20) and v0.3.0 (prepared 2026-10-21: four decks, mic and aux inputs, stages, Collection Health, in-app updates, signed macOS app) and v0.3.5 (prepared 2026-10-28: sampler with sample packs, phrases, mini waveforms, key lock and fuzzy keymixing, FLX4 pad modes, Controller Editor; from the v0.4 list below, add-analyze-track, add-key-lock-and-fuzzy-keymixing and add-controller-viewer shipped early, the rest stay for v0.4) and v0.4.0 (prepared 2026-10-28: duplicate merging and quarantine, name clean-up and genre merging, online tag lookup, sets and crates, sync deck controls, FX bar, compact 2-Deck layout, Windows preview, experimental Pioneer USB export; add-cloud-storage-care, add-browser-preview-player and add-more-audio-decoders move on) and v0.4.5 (prepared 2026-10-30, early from the v0.5 list: rekordbox and Mixxx library imports, controller mapping import/export, own mappings, Map mode and Quick map, DDJ-FLX6 mapping, first-run guided tour, Beat This! beatgrids with grid lock, streaming browse for Spotify, TIDAL, Apple Music, SoundCloud and Mixcloud, Albums and Mixed Sets folders, "collection" naming) and v0.4.6 (prepared 2026-10-31: headphone cue follows the loaded deck, smaller screens, metronome and Bar 1 here for grids, Automix crossfader lock, kept streaming playlists, Spotify Download Track) and v0.4.7 (prepared 2026-11-02: plugin effects and instruments (CLAP, VST3, AU), phone remote for Automix, FX engine upgrade with six new effects, Traktor Kontrol HID controllers, XDJ-RX3 and Numark V7 mappings, beat counter, slip per trick, Automix Order menu, audio health meter, .deb and .rpm packages, and from add-live-broadcast the loudness meter, true-peak limiter and Icecast/Shoutcast streaming) and v0.5.0 (prepared 2026-11-04: Traktor import, tag groups, batch editing, Plex, Navidrome and Jellyfin sources, Settings › Sources, Collection Health dashboard with five areas, Player layout, Groove Box, Instruments rail, advanced tutorials, Mixcloud export, Xone K2 mapping, Settings › Plugins with plugin windows, one accent colour, M3U8 export and M3U/PLS/XSPF import, the now-playing feed and OBS overlay; add-prep-layout ships hidden for v0.6, and the v0.5 changes not started move to v0.6) and v0.5.1 (prepared 2026-11-06, early from the v0.6 list: deck outputs and external mixer mode, MIDI clock out and in, OSC control, rekordbox XML export, app menus, backups, buy from Wanted, SLICER and FREEZE, Stem FX, quicker scratching, track end warning, imported lists and nested list moves, Intro over outro Automix on better phrase detection, Groove Box with eight tracks and pad controllers; the rest of v0.6 stays for v0.6). `openspec/specs/` describes the app as of v0.3.0 plus PR #56.

## v0.4 · Your collection, cleaned up

- [add-analyze-track](changes/add-analyze-track/proposal.md) (Analyze Track in the browser menu, Seba 2026-10-28)
- [add-key-lock-and-fuzzy-keymixing](changes/add-key-lock-and-fuzzy-keymixing/proposal.md)
- [add-duplicate-resolution](changes/add-duplicate-resolution/proposal.md) (collection maintenance PR 3)
- [add-metadata-cleanup](changes/add-metadata-cleanup/proposal.md) (collection maintenance PR 4)
- [add-online-metadata-lookup](changes/add-online-metadata-lookup/proposal.md) (collection maintenance PR 5)
- [add-crates-and-set-lists](changes/add-crates-and-set-lists/proposal.md) (collection maintenance PR 6)
- [add-cloud-storage-care](changes/add-cloud-storage-care/proposal.md) (collection maintenance PR 7)
- [add-sync-deck-controls](changes/add-sync-deck-controls/proposal.md)
- [add-browser-preview-player](changes/add-browser-preview-player/proposal.md)
- [add-more-audio-decoders](changes/add-more-audio-decoders/proposal.md)
- [add-controller-viewer](changes/add-controller-viewer/proposal.md) (view-only Controller Editor, pulled forward by Seba 2026-10-27)
- [add-pioneer-usb-export](changes/add-pioneer-usb-export/proposal.md) (built on rbox, experimental until tried on a player; Seba 2026-10-28)
- [add-opendjscript](changes/add-opendjscript/proposal.md) (one-line actions for keys and buttons, ⌘1 to ⌘5 rate tracks)
- [add-keyboard-shortcuts-editor](changes/add-keyboard-shortcuts-editor/proposal.md) (change keyboard shortcuts in Settings)

## v0.5 · Stems, performance and switching over

- [improve-stem-separation](changes/improve-stem-separation/proposal.md) (instant stems on load, studio-quality stems in the background; moved ahead of stores by Seba, 2026-10-27)
- [add-sample-packs](changes/add-sample-packs/proposal.md) (sampler with your own samples and sample packs, issue #60)
- [improve-fx-engine](changes/improve-fx-engine/proposal.md) (faster rack, cleaner delays and new effects; from the FX library review, Seba 2026-10-27)
- [add-more-library-imports](changes/add-more-library-imports/proposal.md) (rekordbox and Mixxx imports, VirtualDJ import updated; moved from v0.7 by Seba 2026-10-29, issue #131, #75)
- Traktor import (`collection.nml`: tracks, cues, loops, grids, nested playlist folders; issue #294)
- [add-more-controllers](changes/add-more-controllers/proposal.md) (popular controllers supported and drawn; Seba 2026-10-29, issue #132)
- [add-hid-controllers](changes/add-hid-controllers/proposal.md) (Traktor Kontrol S2/S3/S4, F1, Z1 and MX2 over USB HID with Mixxx's HID mappings; Seba 2026-11-01)
- [add-controller-mapper](changes/add-controller-mapper/proposal.md) (own mappings, editing, Map mode and Quick map; moved from v1.0 by Seba 2026-10-29, issue #133)
- [add-controller-mapping-import-export](changes/add-controller-mapping-import-export/proposal.md) (VirtualDJ, Traktor, rekordbox and djay Pro mappings in; openDJ and Mixxx out; keyboard sets; Seba 2026-10-29, issue #133, PR #140)
- [improve-beatgrids](changes/improve-beatgrids/proposal.md) (research first, djay Pro as the bar; saved grids and editing; moved from v1.0 by Seba 2026-10-29, issue #134)
- [add-first-run-setup](changes/add-first-run-setup/proposal.md) (first-run guided tour; moved from v1.0 by Seba 2026-10-29, issue #135)
- [add-tutorials](changes/add-tutorials/proposal.md) (advanced tutorials in Help; Seba 2026-10-29, issue #136)
- [add-streaming-browse](changes/add-streaming-browse/proposal.md) (Spotify, TIDAL, Apple Music, SoundCloud, Mixcloud and other streaming playlists, browse only; Seba 2026-10-29, issue #139)
- [improve-metadata-lookup](changes/improve-metadata-lookup/proposal.md) (batch metadata fixes that match more tracks, file names as reviewed suggestions; Seba 2026-11-01)
- [add-slip-vinyl-and-bend](changes/add-slip-vinyl-and-bend/proposal.md) (slip returns after each trick with a ghost playhead, SLIP and VINYL buttons, on-screen bend and censor; from the deck performance review, Seba 2026-11-01)
- [add-phone-remote](changes/add-phone-remote/proposal.md) (control Automix and the main levels from a phone, by QR code; step 1 of 5; Seba 2026-11-01)
- [add-groove-box](changes/add-groove-box/proposal.md) (a step-sequencer Groove Box that takes the place of a deck, beginner first, synthesised classic drum machine kits; replaces add-pattern-sequencer, moved from v1.x by Seba 2026-11-02)
- [add-batch-edit](changes/add-batch-edit/proposal.md) (edit many tracks at once: pick any set, the Info tab edits them together, tag chips, Fix Names into the review table; 2026-11-02)
- [add-tag-groups](changes/add-tag-groups/proposal.md) (openDJ's own tag model: named tag groups with tags as entries, imports map into them; Settings › Collection › Tags; 2026-11-04)
- [add-mixcloud-export](changes/add-mixcloud-export/proposal.md) (a recorded mix as a Mixcloud show: tracklist from the recording's cue sheet, MP3 with chapters, then sign-in and direct upload; 2026-11-03)
- [add-player-layout](changes/add-player-layout/proposal.md) (Player replaces Simple: a music player over Automix with a queue and search; step 1 of 3; issue #297, 2026-11-03)
- [redesign-collection-health](changes/redesign-collection-health/proposal.md) (Collection Health as an Overview dashboard with % clean and the stages, five areas in fixing order, then sure fixes at once and a one-by-one review; 2026-11-04)
- [add-live-broadcast](changes/add-live-broadcast/proposal.md) (steps 1-2: a loudness meter with stream targets and a true-peak limiter, then Icecast/Shoutcast radio with now-playing; moved from v1.x by Seba 2026-11-02 after the radio and livestream research)
- [add-now-playing-feed](changes/add-now-playing-feed/proposal.md) (now-playing text/JSON files and an OBS overlay page; Seba 2026-11-02)
- [add-playlist-files](changes/add-playlist-files/proposal.md) (export any list as M3U8; import M3U, PLS and XSPF; Seba 2026-11-02)
- [add-controller-scripting](changes/add-controller-scripting/proposal.md) (actions on controller rows: press, release, long press, modes; with the controller mapper, issue #133)

## v0.6 · Buy it from the artist

- [add-addon-repositories](changes/add-addon-repositories/proposal.md)
- [enable-stores-and-purchase-inbox](changes/enable-stores-and-purchase-inbox/proposal.md)
- [add-bandcamp-collection-sync](changes/add-bandcamp-collection-sync/proposal.md)
- [add-find-and-buy](changes/add-find-and-buy/proposal.md)
- [add-buy-from-wanted](changes/add-buy-from-wanted/proposal.md) (Buy… on Wanted tracks: every store checked, ranked by coverage, artist-direct and speed to the library, no affiliate links; Beatport cart if partner access is granted; purchases matched back into Inbox; issue #358, 2026-11-05)
- [add-artist-direct-stores](changes/add-artist-direct-stores/proposal.md)
- [add-prep-layout](changes/add-prep-layout/proposal.md) (a Prep layout on Deck 1 for getting In Prep tracks ready: grid tools, hot cues with suggestions from phrases, a checklist and quick review keys; issue #292, 2026-11-03; built behind a switch in v0.5, shown from v0.6 to leave time to test it, 2026-11-04)
- [add-live-broadcast](changes/add-live-broadcast/proposal.md) (steps 3-5: Go live to Twitch, YouTube and Mixcloud Live over RTMP with a now-playing picture, WebRTC (WHIP), radio show tools)
- [add-dvs](changes/add-dvs/proposal.md) (timecode vinyl and CDs; moved from after 1.0, issue #317)
- [add-stem-fx-and-acapella](changes/add-stem-fx-and-acapella/proposal.md) (moved from v0.5 at the 0.5.0 release)
- [improve-scratching](changes/improve-scratching/proposal.md) (moved from v0.5 at the 0.5.0 release)
- [add-slicer-and-freeze](changes/add-slicer-and-freeze/proposal.md) (moved from v0.5 at the 0.5.0 release; SLICER and FREEZE pad pages built, issue #386)
- [add-settings-table](changes/add-settings-table/proposal.md) (first step of add-settings-layers: one option table, Show modified, reset on each row, search words, help in ⓘ; issue #364, split 2026-11-05)
- [add-settings-deck-choices](changes/add-settings-deck-choices/proposal.md) (second step: Decks sections, Hot cue on a playing deck, Track end warning and Back up the collection rows; issue #365, split 2026-11-05)
- [improve-filter-bar](changes/improve-filter-bar/proposal.md) (filter chips that narrow each other, values ordered by tracks, reorder and remove chips, per-chip Key and BPM options, Core and More fields, a finder in the search bar; issue #468, 2026-11-07)
- [add-deck-outputs](changes/add-deck-outputs/proposal.md) (each deck on its own outputs for a DAW such as Ableton, and an external mixer mode; no cost when unused; Seba 2026-11-02; moved from v0.5 at the 0.5.0 release, not started)
- [add-midi-clock](changes/add-midi-clock/proposal.md) (MIDI clock out to drum machines and other gear, clock in, decks can follow Link; Seba 2026-11-02; moved from v0.5 at the 0.5.0 release, not started)
- [improve-list-model](changes/improve-list-model/proposal.md) (imports read-only with Make These Lists Mine, crates as their own kind in folders, drag to move lists; from a review of folders, playlists and crates, 2026-11-05)
- [add-osc-control](changes/add-osc-control/proposal.md) (every control over Open Sound Control, plus beat, phrase and track events for visuals and lights; Seba 2026-11-02; moved from v0.5 at the 0.5.0 release; built, issue #383)
- [add-rekordbox-xml-export](changes/add-rekordbox-xml-export/proposal.md) (collection and lists out as rekordbox XML for rekordbox and library tools; Seba 2026-11-02; moved from v0.5 at the 0.5.0 release, not started)
- [add-phrase-analysis](changes/add-phrase-analysis/proposal.md) (moved from v0.7 2026-11-06: chroma novelty, repeated sections with Verse, energy, rekordbox benchmark; issue #431)
- [add-phrase-aware-automix](changes/add-phrase-aware-automix/proposal.md) (Automix mixes intro over outro on phrase changes, imported automix points win, bass swap on a phrase, deck phrase countdown, start on the leader's next phrase; moved from v1.x 2026-11-06 after the gap review; issue #425)
- [add-groove-pads](changes/add-groove-pads/proposal.md) (the Groove Box on pad controllers: one pad surface for steps, live play with recording, grooves and kits; Maschine Mikro MK3 first, then Launchpad and Akai; issue #420, 2026-11-06)
- [improve-groove-box-play](changes/improve-groove-box-play/proposal.md) (Groove Box own tempo, kick in key, eight tracks, musical randomize, living grooves, Auto fills and Remix, 4x4 view; a user request 2026-11-07)
- [add-live-deck-waveforms](changes/add-live-deck-waveforms/proposal.md) (live decks draw what they play on the scrolling waveform: a live trace on the leading deck's grid, a note roll for instruments, step lanes for the Groove Box, macro lanes as a second view; issue #497, 2026-11-07)
- [add-midi-device-routing](changes/add-midi-device-routing/proposal.md) (DJ controllers never play instruments, Play instruments per device, a Deck menu with Follow instrument; from a user report, issue #470, 2026-11-07)
- [rework-instruments](changes/rework-instruments/proposal.md) (Instruments as a browser like Collection with presets, an instrument deck with knobs, trim, pan, safety limiter, scale, chord and arp, and artwork; clips assessed and left out; 2026-11-07)

## v0.7 · Your library, everywhere

- [add-prepare-list](changes/add-prepare-list/proposal.md)
- [add-pipewire-audio](changes/add-pipewire-audio/proposal.md) (native PipeWire outputs by name, shared devices, one clock for every device; parked from v0.6 2026-11-06, issue #402)
- [add-synced-lyrics](changes/add-synced-lyrics/proposal.md) (lyrics from .lrc files and tags in a Lyrics tab and Player mode, LRCLIB lookup off at first, fixing and syncing; moved from v1.x 2026-11-06 after the lyrics and karaoke research)
- [add-karaoke-screen](changes/add-karaoke-screen/proposal.md) (the words of the track playing on a second display with the word being sung filling, a count-in and a guide vocal slider, then a /karaoke page on the local web server; split from the lyrics and karaoke research 2026-11-06)
- [add-waveform-lyrics](changes/add-waveform-lyrics/proposal.md) (timed lyrics along the bottom of each deck's big waveform, sung part lilac; user request 2026-11-09)
- [add-whisper-word-timing](changes/add-whisper-word-timing/proposal.md) (Time Words: Whisper large-v3-turbo hears the vocal stem and the lyrics get a time for each word, then transcription of tracks with no lyrics; user request 2026-11-09)
- [add-live-looper](changes/add-live-looper/proposal.md) (a four-layer looper on a deck: grab the last bars of the mix, layer instruments and inputs on top, record and overdub locked to the beat, save loops to the Inbox; a clip grid assessed and left out; 2026-11-07)
- [add-system-media-controls](changes/add-system-media-controls/proposal.md) (media keys, headphone buttons and the system's Now Playing for Player mode on macOS, Windows and Linux; from the Player gap review, 2026-11-09)
- [add-keep-going](changes/add-keep-going/proposal.md) (Keep going in Player mode: Up Next tops itself up with tracks that mix, from everything, a list or an artist, with the next picks shown greyed; Start Radio from a track, artist or list; from the Player gap review, issue #536, 2026-11-09)
- [add-player-pages](changes/add-player-pages/proposal.md) (artist and album pages over the list in Player mode with Play, Shuffle and Mix, then a Home page; from the Player gap review, issue #536, 2026-11-09)
- [add-instrument-per-deck](changes/add-instrument-per-deck/proposal.md) (several instrument plugins at once, one per deck, with plugins processed side by side and idle instruments sleeping; issue #523, 2026-11-09)

## v0.8 · From the booth to the studio

- [add-ableton-set-export](changes/add-ableton-set-export/proposal.md) (History sets, lists and recordings as Ableton Live sets: warped clips, tempo, markers, then a full rebuild with automation from the control log; moved from v1.0 2026-11-06)

## v1.0 · Ready for any DJ switching over

- [add-layout-customization](changes/add-layout-customization/proposal.md)
- [add-mcp-server](changes/add-mcp-server/proposal.md)
- [add-settings-layers](changes/add-settings-layers/proposal.md) (the rest of settings in three layers: Advanced with the layer-3 rows, remaining load choices, Reset layout; first steps split out into add-settings-table and add-settings-deck-choices for v0.6, 2026-11-05)
- [add-slip-everywhere](changes/add-slip-everywhere/proposal.md) (slip for scratch, loops, hot cues, reverse and pause and the ghost playhead came early in add-slip-vinyl-and-bend; beat jumps, pad FX and slicer remain)
- [add-super-library](changes/add-super-library/proposal.md)
- [add-user-manual](changes/add-user-manual/proposal.md)

## v1.x · after 1.0

- [add-karaoke](changes/add-karaoke/proposal.md)
- [add-library-cloud-sync](changes/add-library-cloud-sync/proposal.md)
- [add-track-suggestions](changes/add-track-suggestions/proposal.md)

## v2.0

- [add-audio-plugin-hosting](changes/add-audio-plugin-hosting/proposal.md)
- [add-instrument-plugins](changes/add-instrument-plugins/proposal.md)
- [add-cue-sequence-recording](changes/add-cue-sequence-recording/proposal.md) (moved from v0.6 on 2026-11-06)
- [add-fx-pre-post-and-headphone](changes/add-fx-pre-post-and-headphone/proposal.md) (moved from v0.6 on 2026-11-06)
- [add-groove-box-2](changes/add-groove-box-2/proposal.md) (Groove Box 2.0: a bar timeline in the waveform strip with chains, step accents, chance, nudge and rolls, per-row length, 8 rows with a knob per sound, fills, a melodic row in the leading deck's key, recording from pads, sounds from the mix and Replace drums; takes over the Later tasks of add-groove-box)
- [add-windows-support](changes/add-windows-support/proposal.md) (a Windows preview shipped in v0.4.0; code signing waits: SignPath Foundation turned the application down, 2026-10-29, see docs/windows.md)

## Much later (no change yet)

- DMX lighting from phrases and cues.
- Pro DJ Link to CDJs and mixers.

## Decided against, or dropped for now

- Streaming playback of any kind, partner deals and DRM workarounds.
- OneLibrary import (dropped 2026-10-19; rekordbox import is back in v0.5, Seba 2026-10-29).
- Mobile and iPad apps.
- Video mixing (karaoke and lyrics only).
