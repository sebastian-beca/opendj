# Tasks

## 1. Timeline and chains

- [ ] 1.1 Engine: chains of grooves (up to 64 bars), position in the chain in `VisualPositions`; jump to a block on the next bar
- [ ] 1.2 Timeline in the Groove Box's waveform strip: blocks per groove, hit ticks per row, same zoom and playhead as the waveforms, the leading deck's phrase markers behind
- [ ] 1.3 Drag grooves onto the timeline, right-click to repeat, remove or set a fill; empty timeline repeats the selected groove
- [ ] 1.4 Grooves A to H
- [ ] 1.5 4-Deck strip: groove letter and bars left in the chain

## 2. Steps

- [ ] 2.1 Packed step format (on, accent, condition, nudge, roll); groove stays a fixed-size value, nothing allocated on the audio thread
- [ ] 2.2 Accent, nudge, rolls and conditions (chance, X of Y, Fill, Not fill, probability groups) in the engine, with tests at buffer sizes 64, 333 and 2048
- [ ] 2.3 Step right-click menu and hold-a-step editing
- [ ] 2.4 Per-row length (1-64) with an end handle, per-row step size and direction, rotate, 4 pages of 16
- [ ] 2.5 Euclid in the row menu
- [ ] 2.6 Dice with row locks and Busy, using per-kit row rules; Remix; both start on the next bar
- [ ] 2.7 Slop

## 3. Rows and sounds

- [x] 3.0 Eight tracks in a rainbow of colors with sound icons, M and S; the Groove Box fills the whole deck with SEQUENCER, SYNTH (sound, PITCH, DECAY, TONE) and MIXER (PAN, level, meter, M, S) tabs; `rowR_tone`, `rowR_pan`, `rowR_meter`; kits in one sound order (kick, backbeat, closed hat, open hat, second backbeat, tom, rim, percussion)
- [ ] 3.1 Up to 16 sounds per kit (the eight tracks, crash and ride sharing track 8, came with improve-groove-box-play)
- [ ] 3.2 Named fourth knob per row; synth voice parameters for the built-in kits; low-pass for sample rows
- [ ] 3.3 `pack.json`: rows, knobs, kinds, fills, variations, chains; built-in kits updated

## 4. Fills and variation

- [ ] 4.1 FILL (hold) and Auto fill on the last bar before the leading deck's next phrase, driving the Fill and Not fill conditions
- [ ] 4.2 Variation A/B alternating every 1 or 2 bars
- [ ] 4.3 Fills for every built-in groove

## 5. Recording

- [ ] 5.1 REC: pad hits into the groove, quantised, accent from velocity, count-in, undo last pass

## 6. Melodic row

- [ ] 6.1 Melodic row in the engine: scale degree, octave, length, slide; follows the leading deck's key on the bar
- [ ] 6.2 Built-in mono synth voice with CUTOFF and RESONANCE
- [ ] 6.3 Notes to a hosted instrument plugin with sample offsets (after `add-instrument-plugins`)
- [ ] 6.4 Editing: click to toggle, drag for the degree

## 7. Sounds from the mix and replace drums

- [ ] 7.1 Drop a file, pad or waveform region on a row; trim to the first transient; saved as a kit override
- [ ] 7.2 Kick, snare or hat from a loaded track's drum stem
- [ ] 7.3 Replace drums: a groove over a track deck with the drum stem muted, roll and stutter, return on the next beat

## 8. Items, controllers, docs

- [ ] 8.1 Grooves and chains as items in the Instruments source; export with the kit; drag out as a MIDI file
- [ ] 8.2 `[GrooveN]` additions; regenerate website/book/supported-controls.txt
- [ ] 8.3 Steps page: accents by velocity, rows 1-8 on Shift, two-row mode on 16-pad controllers
- [ ] 8.4 User guide: update website/book/src/guide/groove-box.md

## Workflow follow-up

- Archive the change once its last PR is merged, so the main specs take its requirements.
