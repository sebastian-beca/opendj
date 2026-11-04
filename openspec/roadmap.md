# openDJ roadmap

The open OpenSpec changes in `openspec/changes/`, in the order they are planned (within a release, in build order). Each change's `proposal.md` says why, and its `specs/` say what will be true when it's done. When a change ships, archive it (`openspec archive <name>`) so `openspec/specs/` takes its requirements.

Released so far: v0.1.0 and v0.2.0 (2026-10-15, 2026-10-20) and v0.3.0 (prepared 2026-10-21: four decks, mic and aux inputs, stages, Collection Health, in-app updates, signed macOS app) and v0.3.5 (prepared 2026-10-28: sampler with sample packs, phrases, mini waveforms, key lock and fuzzy keymixing, FLX4 pad modes, Controller Editor; from the v0.4 list below, add-analyze-track, add-key-lock-and-fuzzy-keymixing and add-controller-viewer shipped early, the rest stay for v0.4) and v0.4.0 (prepared 2026-10-28: duplicate merging and quarantine, name clean-up and genre merging, online tag lookup, sets and crates, sync deck controls, FX bar, compact 2-Deck layout, Windows preview, experimental Pioneer USB export; add-cloud-storage-care, add-browser-preview-player and add-more-audio-decoders move on) and v0.4.5 (prepared 2026-10-30, early from the v0.5 list: rekordbox and Mixxx library imports, controller mapping import/export, own mappings, Map mode and Quick map, DDJ-FLX6 mapping, first-run guided tour, Beat This! beatgrids with grid lock, streaming browse for Spotify, TIDAL, Apple Music, SoundCloud and Mixcloud, Albums and Mixed Sets folders, "collection" naming) and v0.4.6 (prepared 2026-10-31: headphone cue follows the loaded deck, smaller screens, metronome and Bar 1 here for grids, Automix crossfader lock, kept streaming playlists, Spotify Download Track) and v0.4.7 (prepared 2026-11-02: plugin effects and instruments (CLAP, VST3, AU), phone remote for Automix, FX engine upgrade with six new effects, Traktor Kontrol HID controllers, XDJ-RX3 and Numark V7 mappings, beat counter, slip per trick, Automix Order menu, audio health meter, .deb and .rpm packages, and from add-live-broadcast the loudness meter, true-peak limiter and Icecast/Shoutcast streaming). `openspec/specs/` describes the app as of v0.3.0 plus PR #56.

## v0.4 · Your collection, cleaned up

- [add-analyze-track](changes/add-analyze-track/proposal.md) (Analyze Track in the browser menu, Seba 2026-10-28)
- [add-key-lock-and-fuzzy-keymixing](changes/add-key-lock-and-fuzzy-keymixing/proposal.md) (in progress: opendj PR #57)
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

## v0.5 · Stems, performance and switching over

- [improve-stem-separation](changes/improve-stem-separation/proposal.md) (instant stems on load, studio-quality stems in the background; moved ahead of stores by Seba, 2026-10-27)
- [add-stem-fx-and-acapella](changes/add-stem-fx-and-acapella/proposal.md)
- [improve-scratching](changes/improve-scratching/proposal.md)
- [add-slicer-and-freeze](changes/add-slicer-and-freeze/proposal.md)
- [add-cue-sequence-recording](changes/add-cue-sequence-recording/proposal.md)
- [add-fx-pre-post-and-headphone](changes/add-fx-pre-post-and-headphone/proposal.md)
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
- [add-settings-layers](changes/add-settings-layers/proposal.md) (settings in three layers: good defaults, task-grouped choices, a searchable Advanced list with Show modified; from a review of VirtualDJ's Options & Tweaks; Seba 2026-10-31)
- [improve-metadata-lookup](changes/improve-metadata-lookup/proposal.md) (batch metadata fixes that match more tracks, file names as reviewed suggestions; Seba 2026-11-01)
- [add-slip-vinyl-and-bend](changes/add-slip-vinyl-and-bend/proposal.md) (slip returns after each trick with a ghost playhead, SLIP and VINYL buttons, on-screen bend and censor; from the deck performance review, Seba 2026-11-01)
- [add-phone-remote](changes/add-phone-remote/proposal.md) (control Automix and the main levels from a phone, by QR code; step 1 of 5; Seba 2026-11-01)
- [add-groove-box](changes/add-groove-box/proposal.md) (a step-sequencer Groove Box that takes the place of a deck, beginner first, synthesised classic drum machine kits; replaces add-pattern-sequencer, moved from v1.x by Seba 2026-11-02)
- [add-deck-outputs](changes/add-deck-outputs/proposal.md) (each deck on its own outputs for a DAW such as Ableton, and an external mixer mode; no cost when unused; Seba 2026-11-02)
- [add-midi-clock](changes/add-midi-clock/proposal.md) (MIDI clock out to drum machines and other gear, clock in, decks can follow Link; Seba 2026-11-02)
- [add-osc-control](changes/add-osc-control/proposal.md) (every control over Open Sound Control, plus beat, phrase and track events for visuals and lights; Seba 2026-11-02)
- [add-now-playing-feed](changes/add-now-playing-feed/proposal.md) (now-playing text/JSON files and an OBS overlay page; Seba 2026-11-02)
- [add-playlist-files](changes/add-playlist-files/proposal.md) (export any list as M3U8; import M3U, PLS and XSPF; Seba 2026-11-02)
- [add-batch-edit](changes/add-batch-edit/proposal.md) (edit many tracks at once: pick any set, the Info tab edits them together, tag chips, Fix Names into the review table; 2026-11-02)
- [add-rekordbox-xml-export](changes/add-rekordbox-xml-export/proposal.md) (collection and lists out as rekordbox XML for rekordbox and library tools; Seba 2026-11-02)
- [add-mixcloud-export](changes/add-mixcloud-export/proposal.md) (a recorded mix as a Mixcloud show: tracklist from the recording's cue sheet, MP3 with chapters, then sign-in and direct upload; 2026-11-03)
- [add-player-layout](changes/add-player-layout/proposal.md) (Player replaces Simple: a music player over Automix with a queue and search; step 1 of 3; issue #297, 2026-11-03)
- [redesign-collection-health](changes/redesign-collection-health/proposal.md) (Collection Health as an Overview dashboard with % clean and the stages, five areas in fixing order, then sure fixes at once and a one-by-one review; 2026-11-04)
- [add-live-broadcast](changes/add-live-broadcast/proposal.md) (steps 1-2: a loudness meter with stream targets and a true-peak limiter, then Icecast/Shoutcast radio with now-playing; moved from v1.x by Seba 2026-11-02 after the radio and livestream research)

## v0.6 · Buy it from the artist

- [add-addon-repositories](changes/add-addon-repositories/proposal.md)
- [enable-stores-and-purchase-inbox](changes/enable-stores-and-purchase-inbox/proposal.md)
- [add-bandcamp-collection-sync](changes/add-bandcamp-collection-sync/proposal.md)
- [add-find-and-buy](changes/add-find-and-buy/proposal.md)
- [add-artist-direct-stores](changes/add-artist-direct-stores/proposal.md)
- [add-prep-layout](changes/add-prep-layout/proposal.md) (a Prep layout on Deck 1 for getting In Prep tracks ready: grid tools, hot cues with suggestions from phrases, a checklist and quick review keys; issue #292, 2026-11-03; built behind a switch in v0.5, shown from v0.6 to leave time to test it, 2026-11-04)
- [add-live-broadcast](changes/add-live-broadcast/proposal.md) (steps 3-5: Go live to Twitch, YouTube and Mixcloud Live over RTMP with a now-playing picture, WebRTC (WHIP), radio show tools)
- [add-dvs](changes/add-dvs/proposal.md) (timecode vinyl and CDs; moved from after 1.0, issue #317)

## v0.7 · Your library, everywhere

- [add-phrase-analysis](changes/add-phrase-analysis/proposal.md)
- [add-prepare-list](changes/add-prepare-list/proposal.md)

## v1.0 · Ready for any DJ switching over

- [add-ableton-set-export](changes/add-ableton-set-export/proposal.md)
- [add-layout-customization](changes/add-layout-customization/proposal.md)
- [add-mcp-server](changes/add-mcp-server/proposal.md)
- [add-slip-everywhere](changes/add-slip-everywhere/proposal.md) (slip for scratch, loops, hot cues, reverse and pause and the ghost playhead came early in add-slip-vinyl-and-bend; beat jumps, pad FX and slicer remain)
- [add-super-library](changes/add-super-library/proposal.md)
- [add-user-manual](changes/add-user-manual/proposal.md)

## v1.x · after 1.0

- [add-karaoke](changes/add-karaoke/proposal.md)
- [add-library-cloud-sync](changes/add-library-cloud-sync/proposal.md)
- [add-phrase-aware-automix](changes/add-phrase-aware-automix/proposal.md)
- [add-synced-lyrics](changes/add-synced-lyrics/proposal.md)
- [add-track-suggestions](changes/add-track-suggestions/proposal.md)

## v2.0

- [add-audio-plugin-hosting](changes/add-audio-plugin-hosting/proposal.md)
- [add-instrument-plugins](changes/add-instrument-plugins/proposal.md)
- [add-windows-support](changes/add-windows-support/proposal.md) (a Windows preview shipped in v0.4.0; code signing waits: SignPath Foundation turned the application down, 2026-10-29, see docs/windows.md)

## Much later (no change yet)

- DMX lighting from phrases and cues.
- Pro DJ Link to CDJs and mixers.

## Decided against, or dropped for now

- Streaming playback of any kind, partner deals and DRM workarounds.
- OneLibrary import (dropped 2026-10-19; rekordbox import is back in v0.5, Seba 2026-10-29).
- Mobile and iPad apps.
- Video mixing (karaoke and lyrics only).
