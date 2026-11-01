# Tasks

## 1. Measure (PR 1)

- [x] 1.1 Add an FX benchmark: ns per frame per effect at 48 kHz, an idle and a busy four-deck rack, each Color FX, and the rack's memory use
- [x] 1.2 Add offline quality tests: silence in gives silence out, finite and bounded at every parameter corner, denormal-free tails
- [x] 1.3 Add a test that the rack never allocates on the audio thread

## 2. Faster rack

- [x] 2.1 Lend delay memory from a pre-allocated pool instead of one two-second buffer per instance (PR 1)
- [ ] 2.2 Process each active instance over a block of frames instead of dispatching per frame; keep per-sample smoothing (PR 4, if the benchmark says so)
- [ ] 2.3 Compare against the 1.1 benchmark and record the numbers in each PR

## 3. Better sound

- [x] 3.1 Glide every continuous parameter per sample, crossfade phaser stage changes, cap filter sweeps at about 1/60 octave per sample (PR 2)
- [x] 3.2 Cubic interpolation in echo, echo out, flanger, spiral and brake (after Signalsmith DSP, PR 2); the reverb tank stays linear (see design)
- [ ] 3.3 Lock Flanger, Phaser and Gate phases and Roll starts to the deck's beat grid (PR 3)
- [ ] 3.4 Drive with 2-4x oversampling (after Signalsmith Crunch or an Airwindows saturator)
- [ ] 3.5 Chorus (after Signalsmith Basics)
- [ ] 3.6 Ladder filter flavour (Huovilainen, as in Mixxx)
- [ ] 3.7 Hall reverb next to the plate (after Airwindows Galactic or Signalsmith Basics Reverb)
- [ ] 3.8 Slip Roll, Rev Roll, Helix and filtered echo variants
- [ ] 3.9 Add the attributions to NOTICE and the ported files' headers
- [ ] 3.10 Update the effects user guide page in website/book/src/guide

## Workflow follow-up

- Archive the change once its PRs are merged, so the main specs take its requirements.
