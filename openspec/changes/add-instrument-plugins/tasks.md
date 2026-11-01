# Tasks

## 1. Build

- [x] 1.1 Note events into the plugin helper (`Connection::process_events`, up to 128 MIDI events a block in the shared memory); instruments with no audio input load in CLAP, VST3 (event bus) and AU (`aumu`, `MusicDeviceMIDIEvent`); a `test-synth` test instrument
- [ ] 1.2 Instrument input channel in the engine, next to Mic and Aux, with the MIC panel controls
- [ ] 1.3 MIDI keyboard input, Keyboard pad page and controller mapping as note sources
- [ ] 1.4 Update the user guide page in website/book/src/guide

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.
