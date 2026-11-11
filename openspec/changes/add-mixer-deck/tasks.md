# Tasks

## 1. Engine

- [x] 1.1 One input stream per interface carrying every channel the inputs use, fanned out to the inputs; saved Aux 1 to 3 routes keep working; tests for mono, stereo and two devices
- [x] 1.2 Eight auxiliary inputs `[Auxiliary1]` to `[Auxiliary8]` with pan, solo and fx controls, strips swapped in off the audio thread, new strips start off
- [x] 1.3 Mixer as a deck source: the strips summed into the deck's channel, solo inside the Mixer, cue to the headphones
- [x] 1.4 FX split: strips with FX on go through the deck's effects, the rest stay dry
- [x] 1.5 A meter per interface channel for the input picker
- [x] 1.6 The mic as a strip, sharing talkover and ducking with the MIC button

## 2. App

- [x] 2.1 Mixer item in the Jam source in place of Aux 1 to 3
- [ ] 2.2 2-Deck strips: input box, TRIM, PAN, FX, number square, S, Cue, meter with fader arrow and peak readout, + to add
- [ ] 2.3 Input picker: pairs then single channels with live meters, then Settings › Audio
- [ ] 2.4 4-Deck slim strips
- [ ] 2.5 "Own clock" note on a strip from another interface than the master's
- [ ] 2.6 Recording input choice with External mixer on

## 3. Later

- [ ] 3.1 Instrument plugins and the Groove Box as strip sources, with a Mixer / instrument view switch on the deck
- [ ] 3.2 Mappings: Xone K2 columns on strips 1 to 4
