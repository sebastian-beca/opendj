# Tasks

## 1. Browser

- [x] 1.1 Instruments tree (All instruments, Groove Box, Plugins by maker, Inputs) and a track-style list with search; rows drag to a deck, double-click loads on a free deck
- [x] 1.2 One Groove Box entry; kit picked on the deck
- [ ] 1.3 Type and Character tag chips over the list; tags on presets (SAVE › Tags)

## 2. Presets

- [x] 2.1 Plugin state save and restore (CLAP state, VST3 component and controller state, AU full state) in the plugin host; kept across restarts
- [x] 2.2 openDJ preset files (`Instruments/Presets/<instrument>/`) for plugins and the Groove Box; SAVE menu on the deck (Save, Save as new, Rename)
- [ ] 2.3 Plugin presets listed from the formats: CLAP preset discovery, `.vstpreset`, AU factory presets and `.aupreset`
- [ ] 2.5 Recently played in the tree; column sort (My presets and presets under their instrument, default sound first, are in)
- [x] 2.4 ‹ › on the deck steps through the instrument's presets in the browser's order

## 3. Instrument deck

- [x] 3.1 Live channels: TRIM (−12 to +24 dB), PAN and an always-on true-peak safety limiter at −1 dBTP for every deck playing an instrument or input; Thru on the crossfader by default
- [ ] 3.2 2-Deck instrument deck: art header, preset ‹ ›, scope, EDIT, SAVE, 8 knobs in pages, TRIM, PAN, meter, keyboard lighting played notes
- [ ] 3.3 Knob pages from the plugin (CLAP remote controls), else its first 8 automatable parameters, else the user's picks (right-click Assign…); saved in the preset
- [ ] 3.4 4-Deck instrument strip: art, preset ‹ ›, thin scope, 4 knobs, TRIM, meter, EDIT; no keyboard
- [ ] 3.5 Input decks (mic, aux): header with the interface input, TRIM, PAN, meter
- [ ] 3.7 TRIM set on first play from the input's level
- [ ] 3.6 MIDI from: each instrument deck takes notes from a chosen device and channel (Any by default), so one controller's tracks can play different decks; built on the MIDI device roles' deck choice

## 4. Scale, chord and arpeggiator

- [ ] 4.1 Scale mode: Off, Auto (master deck's key), or a picked root and scale (Major, Minor, Major pentatonic, Minor pentatonic); notes move to the nearest scale note; keyboard lights playable notes and dims the rest
- [ ] 4.2 Chord mode: one key plays a chord built on the scale
- [ ] 4.3 Arpeggiator synced to the mix's beat (Up, Down, Up-down, Random, Order; rate, gate, octaves, hold)

## 5. Artwork

- [ ] 5.1 Own image dropped on the deck's cover; generated fallback in one consistent style
- [ ] 5.2 NKS artwork when installed, plugin window snapshot, plugin bundle icon
