# Tasks

## 1. Engine cost

- [x] 1.1 Plugin blocks handed to every helper first and collected after: deck FX plugins across decks side by side, the instrument plays while the decks are worked out (`Connection::send`/`collect`, `EffectRack::process_blocks`)
- [x] 1.2 An instrument silent for a second, holding and getting no notes and on no deck sleeps until a note, a knob or a deck wakes it

## 2. One instrument per deck

- [ ] 2.1 Engine: `[InstrumentN]` per deck with its own notes, scale, chord, arpeggiator, knobs and view; `PluginMsg::Instrument` names its deck
- [ ] 2.2 Plugin host: a helper per instrument, each with its own crash/hang/CPU policy, sound kept per deck
- [ ] 2.3 MIDI from per deck, so one controller's tracks play different decks (takes over `rework-instruments` 3.9)
- [ ] 2.4 App: instrument deck, 4-Deck strip, presets, knob pages, art, macro lanes, note roll and the Instruments browser follow each deck's instrument
- [ ] 2.5 Measure four heavy instruments on a Mac and record the audio health meter
