# Design

## Context

openDJ's effects are its own Rust code (`crates/engine/src/effects.rs` and `dsp.rs`). The FX library review (`notes/fx-engine-review.md`, 2026-10-27) decided to keep that engine and port MIT-licensed algorithms into it by hand, rather than depend on fundsp, bosai-dsp, Airwindows or Signalsmith DSP. Seba's goal for v0.5 (2026-11-01): high sound quality and low CPU, on four decks at once.

The engine runs everything frame by frame inside the audio callback: each deck's sample goes through EQ, Color FX, Sculpt and the effect rack, then into the main mix, whose effects run before the limiter. Effects read their controls once per buffer in `EffectRack::prepare` and run per frame in `EffectRack::process`.

## Goals

- Every effect sounds clean: no clicks on any knob or switch, no zipper noise, no dulled highs in swept delays, no aliasing where it isn't the point of the effect.
- Every effect is cheap: a busy set (four decks with three effects each, plus Beat FX) costs a small share of one core, measured by a benchmark that ships with the code.
- Nothing allocates, locks or waits on the audio thread, checked by a test.
- Time-based effects lock to the beat, not just to the tempo.
- The same engine serves deck FX, Beat FX, pad FX, the mic's unit and Color FX.

## Non-goals

- Replacing the engine with a library, or adding a DSP dependency.
- Third-party plugin hosting (`add-audio-plugin-hosting`, v2.0).
- Per-stem effects (`add-stem-fx-and-acapella`).

## Decisions

### Measure first

A harness in `crates/engine/src/effects/bench.rs` runs with the engine's tests:

- `fx_bench` (ignored by default; run with `cargo test --release -p opendj-engine fx_bench -- --ignored --nocapture`) prints ns per frame and the share of a core for each effect, each Color FX, an idle rack and a busy rack, plus the rack's memory.
- A counting global allocator (test builds only) checks that `prepare` and `process` never allocate, while effects are switched, released and turned on and off.
- Quality checks: silence in gives silence out, every effect stays finite and bounded with every parameter at 0 or 1 on full-scale noise, and feedback tails decay to exact zeros with flush-to-zero on.

Every later PR reports the benchmark before and after.

### Delay memory from a pool

Each (unit, slot, target) instance used to own one buffer sized for a two-second stereo echo, about 108 MB for 141 instances. Instances now own no delay memory. The rack keeps a pool of 32 long buffers (two seconds of stereo, for Echo, Roll, Brake, Echo Out and Spiral) and 32 short ones (the reverb's tank, also enough for the flanger), allocated once at start-up. An instance borrows a buffer when it starts sounding and gives it back once it is silent (faded out and any release tail finished). Borrowing and returning only move a pointer between the instance and a pre-sized stack, so nothing allocates. If the pool were ever empty, the effect stays off and tries again next buffer; 32 is well above the most effects a real set sounds at once (about 20 with every deck's FX on, the Beat FX on two decks and pad FX held). Each deck's Color FX keeps its own buffer, since it must be ready the moment the knob moves.

### Smooth everything per sample

Every continuous parameter glides per sample towards the value read once per buffer (one-pole, about 5 ms, matching the filters' cutoff glide), including reverb decay, damping and send, flanger depth, centre and feedback, phaser depth and feedback, gate depth and width, and echo send and ping-pong. Switches that can't glide (phaser stages, bit depth steps) crossfade or change at a zero crossing.

### Better interpolation

Swept delay reads move from linear to 4-point cubic (Catmull-Rom/Hermite) interpolation, after Signalsmith DSP's interpolators: Echo while its time glides, Flanger, Spiral, Brake and the reverb's modulated allpasses. Linear interpolation halfway between samples loses about 2 dB at 10 kHz; cubic keeps it within a fraction of a dB, for about four multiply-adds more per read.

### Locked to the beat

`Ctx` gains the target's beat position. LFOs and gates (Flanger, Phaser, Gate) take their phase from the deck's position in the beat grid, so a gate opens on the beat and two decks' flangers sweep together, and Roll starts its slice on the nearest beat division. The main mix follows the master deck. Tap tempo and units with their own `bpm` keep a free-running phase.

### Block processing

Once the above lands, the rack processes each active instance over a block of frames instead of dispatching per frame: the engine renders each deck's chain into a per-deck scratch buffer (sized at start-up for the largest callback), runs the rack over it, then mixes. Parameters still glide per sample inside the block. The benchmark decides whether this is worth its engine restructure: if the busy rack already costs only a few percent of a core, block processing waits.

### New effects

Ported by hand from MIT code, each file header naming its source and `NOTICE` listing it: Drive (2× to 4× oversampled saturation, after Signalsmith Basics' Crunch or an Airwindows saturator), Chorus (Signalsmith Basics), a Moog-style ladder filter (Huovilainen's model, as in Mixxx, GPL-2.0-or-later), a hall reverb next to the plate (Signalsmith Basics Reverb or Airwindows Galactic), Slip Roll, Rev Roll, Helix and filtered echoes (ideas from bosai-dsp's list). New effects join the end of `AVAILABLE_EFFECTS`, so saved `loaded_effect` values and mappings keep their meaning.

## Risks

- **A pooled effect could find no memory.** It stays silent rather than glitching; the pool is sized far above real use, and a test covers the empty case.
- **Beat-locked phases jump when the user seeks or loops.** Phases follow the grid, so a jump moves the LFO with it, which is what a DJ expects; gates already slew their edges, so a jump doesn't click.
- **Block processing changes the engine's main loop.** It comes last, behind the benchmark, in its own PR.

## PR plan

1. Benchmark and quality harness, delay memory pool, this design (milestone v0.5).
2. Smooth parameters and cubic interpolation.
3. Beat-locked LFOs, gates and rolls.
4. Block processing, if the benchmark says so.
5. New effects, menus, pad FX pages, notices and the user guide.
