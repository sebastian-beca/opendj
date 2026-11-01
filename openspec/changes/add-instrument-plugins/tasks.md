# Tasks

## 1. Build

- [x] 1.1 Note events into the plugin helper (`Connection::process_events`, up to 128 MIDI events a block in the shared memory); instruments with no audio input load in CLAP, VST3 (event bus) and AU (`aumu`, `MusicDeviceMIDIEvent`); a `test-synth` test instrument
- [x] 1.2 Instrument input channel in the engine, next to Mic and Aux (`[Instrument1]`, `instrument` picks the plugin, `main_mix`, Mic FX unit route); the MIC panel controls come with 1.3
- [x] 1.3 MIDI keyboard input (devices with no mapping), the MIC panel's pads and the decks' KEYBOARD pads (`[Instrument1],keyboard_pads`; controllers too) as note sources; `pad_1`..`pad_16` and `octave` for mappings
- [x] 1.4 Update the user guide page in website/book/src/guide

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.
