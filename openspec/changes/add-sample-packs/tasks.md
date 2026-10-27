# Tasks

## 1. Engine

- [ ] 1.1 16 sampler slots, `[App],num_samplers` = 16; Default DJ Sample Pack sounds on slots 1-8
- [ ] 1.2 Load audio files onto a slot off the audio thread (decode, resample, stereo, 60-second limit, errors back to the app)
- [ ] 1.3 `[SamplerN],play_mode` (one shot, hold, on/off, loop) applied to `cue_gotoandplay` press and release; `repeat`, `cue_gotoandstop`; short fades
- [ ] 1.4 Level on `pregain` (silent to +6 dB)
- [ ] 1.5 Check the DDJ-FLX4 sampler pads and LEDs for slots 1-16
- [ ] 1.6 Regenerate website/book/supported-controls.txt

## 2. Packs and pads

- [ ] 2.1 Pack store in `<config>/samples/` (`pack.json`), Default pack, overrides in `settings.json` under `sampler`
- [ ] 2.2 SAMPLER page: 16-slot layout per deck side, SAMPLE PACK header with pack menu (covers, +, Get more Packs placeholder)
- [ ] 2.3 Edit mode: level knob and "…" on each pad; right-click menu (Choose Sample, Load file, Rename, Play mode, Reset, Clear)
- [ ] 2.4 Choose Sample popover with headphone preview and lengths
- [ ] 2.5 Drag and drop from the file manager and the track list onto pads; missing-file state and Locate file
- [ ] 2.6 Update the user guide performance pads page in website/book/src/guide, with screenshots

## 3. Share packs

- [ ] 3.1 New pack, rename, delete, show in Finder
- [ ] 3.2 `.opendjpack` export and import (menu and drop)

## 4. Record and sync

- [ ] 4.1 Record New Sample from a deck, the mic or the master into "My Recordings"; loop-length capture
- [ ] 4.2 Loop samples follow the master tempo and start quantised

## 5. Get more Packs

- [ ] 5.1 Design the dialog and the pack source (see design.md), then build it

## Workflow follow-up

- Archive the change once its last PR is merged, so the main specs take its requirements.
