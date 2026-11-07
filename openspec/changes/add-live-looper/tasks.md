# Tasks

## 1. Engine

- [ ] 1.1 Looper source on a deck (`[ChannelN],live_input` gets a looper): four layers, tap cycle (arm on next bar, record, play or overdub, hold to clear), mute and level per layer, deck PLAY and CUE start and stop all layers
- [ ] 1.2 Lengths 1, 2, 4, 8, 16 bars locked to the sync leader; free first layer sets the tempo and makes the looper the leader; layers are whole multiples and phase-locked
- [ ] 1.3 UNDO and REDO per pass, buffers made and freed off the audio thread
- [ ] 1.4 Input picker per layer: Aux 1-3, Mic, instrument deck, Groove Box, any deck, main mix (resample); input heard through while recording
- [ ] 1.5 Capture rings fed off the audio thread; CAPTURE puts the last 1, 2, 4 or 8 bars into the next free layer
- [ ] 1.6 Loops keep their BPM and follow tempo changes through the key-lock stretcher
- [ ] 1.7 Latency compensation: round-trip measure in Settings › Audio, recorded audio moved back by it
- [ ] 1.8 Input queue stretches instead of dropping or padding while a looper records from it

## 2. App

- [ ] 2.1 Looper row in the Instruments browser, dragged onto a deck like the Groove Box
- [ ] 2.2 2-Deck looper deck: four layer rows (state light, level, mute, input), length, CAPTURE, UNDO, REDO
- [ ] 2.3 4-Deck looper strip: four layer lights and CAPTURE
- [ ] 2.4 Settings: After recording (Play or Overdub), capture length kept (memory)
- [ ] 2.5 Save loop: a layer or all four mixed to the Inbox as WAV with BPM and key
- [ ] 2.6 Big waveform: a lane per layer with the play head going round

## 3. Controllers

- [ ] 3.1 Maschine Mikro MK3: pads 1-4 layers, REC capture, PLAY and STOP, ERASE undo, hold ERASE + pad clear, strip length
- [ ] 3.2 Xone K2: a column per layer (level, record, mute, clear)
- [ ] 3.3 DDJ-FLX4 pad page on a looper deck: layers 1-4, capture, undo, length, clear
- [ ] 3.4 KeyLab mk3 transport drives the looper on the focused deck
