# Tasks

## 1. Measure

- [ ] 1.1 Add an FX benchmark: ns per frame per effect at 48 kHz, four decks, and the rack's memory use
- [ ] 1.2 Add offline quality tests next to `roughness()`: aliasing, noise floor, denormal stress

## 2. Faster rack

- [ ] 2.1 Size each instance's buffer for its effect, or create instances off the audio thread when a unit is first routed
- [ ] 2.2 Process each active instance over a block of frames instead of dispatching per frame; keep per-sample smoothing
- [ ] 2.3 Compare against the 1.1 benchmark and record the numbers in the PR

## 3. Better sound

- [ ] 3.1 Cubic or Lagrange interpolation in echo, flanger and spiral (after Signalsmith DSP)
- [ ] 3.2 Drive with 2-4x oversampling (after Signalsmith Crunch or an Airwindows saturator)
- [ ] 3.3 Chorus (after Signalsmith Basics)
- [ ] 3.4 Ladder filter flavour (Huovilainen, as in Mixxx)
- [ ] 3.5 Hall reverb next to the plate (after Airwindows Galactic or Signalsmith Basics Reverb)
- [ ] 3.6 Slip Roll, Rev Roll, Helix and filtered echo variants
- [ ] 3.7 Add the attributions to THIRD_PARTY_NOTICES
- [ ] 3.8 Update the effects user guide page in website/book/src/guide

## Workflow follow-up

- Archive the change once its PRs are merged, so the main specs take its requirements.
