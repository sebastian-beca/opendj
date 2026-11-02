# Tasks

## 1. Measure (PR 1)

- [x] 1.1 Add an FX benchmark: ns per frame per effect at 48 kHz, an idle and a busy four-deck rack, each Color FX, and the rack's memory use
- [x] 1.2 Add offline quality tests: silence in gives silence out, finite and bounded at every parameter corner, denormal-free tails
- [x] 1.3 Add a test that the rack never allocates on the audio thread

## 2. Faster rack

- [x] 2.1 Lend delay memory from a pre-allocated pool instead of one two-second buffer per instance (PR 1)
- [ ] 2.2 Process each active instance over a block of frames instead of dispatching per frame; keep per-sample smoothing (PR 4: deferred, the benchmark puts dispatch at about 10% of the rack's cost)
- [x] 2.3 Compare against the 1.1 benchmark and record the numbers in each PR

## 3. Better sound

- [x] 3.1 Glide every continuous parameter per sample, crossfade phaser stage changes, cap filter sweeps at about 1/60 octave per sample (PR 2)
- [x] 3.2 Cubic interpolation in echo, echo out, flanger, spiral and brake (after Signalsmith DSP, PR 2); the reverb tank stays linear (see design)
- [x] 3.3 Lock Flanger, Phaser and Gate phases and Roll starts to the deck's beat grid (PR 3)
- [x] 3.4 Drive: 4x oversampled, antiderivative anti-aliased saturator (after Signalsmith Crunch), 0-24 dB (PR 5)
- [x] 3.5 Chorus, beat-locked (after Signalsmith Basics, PR 5)
- [x] 3.6 Ladder filter: zero-delay-feedback Moog-style ladder (Zavalishin), aiming at Mixxx's Huovilainen sound (PR 5)
- [x] 3.7 Hall reverb next to the plate: 8-line Hadamard FDN with pre-delay (after Signalsmith's reverb design, PR 5)
- [x] 3.8 Rev Roll and Filter Echo (LPF/HPF repeats) (PR 5). Our Roll already slips (the deck plays on underneath). Helix is left for later
- [x] 3.9 Add the attributions to NOTICE and the files' headers (PR 5)
- [x] 3.10 Update the effects user guide page in website/book/src/guide (PR 5)

## Workflow follow-up

- Archive the change once its PRs are merged, so the main specs take its requirements.
