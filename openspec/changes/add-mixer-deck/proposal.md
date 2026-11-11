# Proposal

Planned for: **v0.7**, after `add-live-looper`'s input work and next to `add-instrument-per-deck`

## Why

A deck can play one interface input at a time (Aux 1 to 3 from the Jam source), with trim, pan and a deck meter. That is too little for a DJ who plays alongside a turntable, a phone, another DJ's mixer, synths and a drum machine: each needs its own deck, the deck shows no level for what comes in, and three aux inputs is the limit. Ableton, Bitwig, Ardour and the DJM mixers show the answer: thin channel strips, one per input, with the meter and fader in one column.

The engine also opens a stream per input. With eight inputs on one interface that means eight streams on the same device, which ASIO and ALSA `hw:` refuse and which drift apart on their own clocks (research: Mixxx, Ardour, jack_mixer, cpal #204; notes in issue #606).

## What Changes

- **Mixer on a deck.** A **Mixer** item in the Jam source replaces Aux 1 to 3. A Mixer deck holds up to **8 strips**, each an interface input, summed into the deck's channel: the deck's EQ, filter, FX and fader act on the sum, so the deck fader is the Mixer's master. One deck holds the Mixer at a time.
- **Thin strips (about 104 px).** Top to bottom: color band and name; the input box (`1/2`, `3`, with ▼ on the right: picking one channel makes the strip mono, a pair stereo); then, in a column beside the meter: peak readout (red over 0 dB), TRIM, PAN, FX, and at the bottom the lilac **number square** (on/off, grey when off), **S** (solo) and **Cue**. The meter column has a dB scale, one bar for mono or two for stereo, peak hold, and the fader as an arrow on the scale (Ableton style) with its dB above. An empty slot shows **+**. 4-Deck shows a slim version: meter with fader arrow, FX, number, S, Cue.
- **Input picker.** The ▼ lists channel pairs then single channels of the interface, each with a live mini meter, then "Settings › Audio…".
- **FX button, no send knobs.** Strips with FX lit go through the deck's effects; the others stay dry.
- **Solo and mute.** Solo silences the other strips of this Mixer only, never other decks; solos add up, Cmd/Ctrl-click solos one alone; solo wins over off. Cue sends the strip to the headphones like a deck's cue.
- **Mic strip.** The microphone can be one strip, marked Mic with a DUCK button; the MIC button in the top bar stays and controls the same mic (talkover, ducking).
- **Safe starts.** A new strip starts off (grey number), so a mic near the speakers can't howl when added; auto trim sets an untouched TRIM from the first seconds, as live decks do now. The deck's −1 dBTP limiter stays.
- **One stream per interface.** Each interface is opened once with every channel any strip needs, and fanned out to the strips. Inputs on the master's interface share its clock; a strip on another interface works and shows an "own clock" note. Strips are swapped in off the audio thread.
- **Controls.** `[Auxiliary1]` to `[Auxiliary8]` with Mixxx's names (`pregain`, `pan`, `volume`, `mute`, `pfl`, `main_mix`) plus `solo`, `fx`, `input_configured`, `vu_meter` and `peak_indicator`, so mappings (Xone K2 faders) work.
- **Later in this change:** the instrument plugins and the Groove Box as strip sources, with a switch on the deck between the Mixer view and the instrument or Groove Box view; loading one alone on a deck stays.

## Out of scope

- Send knobs and return effects (the FX button covers it).
- A per-strip EQ (the deck's EQ acts on the sum).
- Drift resampling for inputs on another interface (the looper's input stretching may cover it).
- A latency offset per input (the looper's latency measure covers recording).

## Capabilities

### Modified Capabilities

- `mic-aux-inputs`: up to eight aux inputs as strips of a Mixer deck, picked by channel, opened once per interface.

## Impact

- Engine: `output.rs` opens one input stream per device and fans channels out; `inputs.rs` grows to eight auxiliaries with solo and FX; a Mixer `live_input` value in `engine.rs` sums the strips into the deck with a wet/dry split around the deck's FX.
- Settings: saved Aux 1 to 3 routes become strips 1 to 3; a strip list per setup.
- App: Mixer item in Jam, the strip UI in 2-Deck and 4-Deck, the input picker with meters.
- Recording: unchanged (the master after the limiter). With External mixer on, the recording input becomes a choice of strip.
