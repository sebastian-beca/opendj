# Design

## Where the taps sit

`Engine::process` renders each deck into `deck_buf` frame by frame, applies gain, EQ, Color FX filter and deck FX, then the channel fader and crossfader, and sums into `main_buf` and `cue_buf`. The three tap points map onto that chain:

| Tap | Taken after | Taken before |
|---|---|---|
| Clean | tempo, key lock, stems, pregain and auto gain | EQ, filter, deck FX |
| Pre-fader | EQ, filter, deck FX | channel fader, crossfader |
| Post-fader | channel fader, crossfader | the sum into master |

Post-fader taps sum to the master mix before master FX and the limiter, which lets a DAW rebuild the mix and apply its own master chain.

## Render buffer

Today the engine renders a fixed 6-channel interleaved buffer (master, headphones, booth) and `write_pairs` copies pairs to each device. With deck routes, the layout becomes:

```
0-1 master · 2-3 headphones · 4-5 booth · 6-7 deck 1 · 8-9 deck 2 · 10-11 deck 3 · 12-13 deck 4 · 14-15 sampler · 16-17 groove box
```

The engine is told the channel count (`process(out, channels)` already takes it). Only the pairs that have routes are written; others are skipped, so the active width is 6 plus two per routed source. The buffer is allocated once at configure time for the widest layout (18 channels × the largest block), so the callback never allocates.

## Keeping it free when unused

- `taps: [Option<Tap>; 6]` is resolved from the routes when outputs are configured, not per buffer.
- Inside the frame loop, the deck path writes the tap only when its slot is set. That branch is constant for the whole buffer and predicts perfectly; for the common no-route case the compiler can hoist it.
- External mixer mode skips EQ, faders, crossfader, master FX and limiter for decks: strictly less work.

## Devices and timing

- Deck routes on the master's device: same callback, sample-aligned.
- Deck routes on another device: fed through the existing ring buffers with drift correction (drop or pad a little). Each extra device is drift-corrected separately, so two decks on two different devices are not sample-aligned with each other. Settings warn when deck outputs are split across devices and suggest one interface or a macOS aggregate device.
- For a DAW on the same computer, a virtual multichannel device (BlackHole 16ch, Loopback, VB-Cable, PipeWire/JACK) as the master device keeps everything in one clock domain.

## External mixer mode

- `[Master],external_mixer` = 1. Decks output at the Pre-fader point with EQ, filter and fader treated as flat; deck FX remain. Master, headphones and booth routes may be empty.
- Mixer controls (EQ, volume, crossfader, headphone cue, master levels) are ignored while on and hidden in the UI; mappings that send them don't error.
- Automix needs a crossfader, so it is off with a note. Recording and broadcasting need a master mix: in this mode they offer recording from an input (the hardware mixer's record out into Aux), as the recorder's input option.
- Mic and Aux inputs are not routed (the hardware mixer has its own); the sampler and Groove Box go to their own pair, or, when none is set, into Deck 1's pair.

## Testing

- Unit: tap points against known signals (EQ kill shows in Pre-fader but not Clean; crossfader at one side zeroes the other deck's Post-fader); Post-fader taps sum to master within float rounding.
- Engine benchmark (stem-bench style) comparing buffer render time with no routes, four deck routes, and external mode, on the CI runner and Seba's Mac. Merge only if the no-route case is unchanged and four routes add under 1%.
- Manual: Ableton Live with BlackHole 16ch, four decks on four stereo tracks; DDJ-FLX4's audio interface with two deck outputs (it has four output channels).
