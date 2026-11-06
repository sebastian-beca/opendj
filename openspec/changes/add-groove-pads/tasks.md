# Tasks

## 1. Pad surface

- [x] 1.1 Engine: `groove_pads.rs` controls, presses (a tap between two buffers still counts), colours; Steps, Play (velocity, record to the nearest step without a double hit) and Grooves modes in `GrooveBox::render`; tests
- [x] 1.2 Maschine Mikro MK3 HID mapping and script: pads, buttons, encoder, touch strip, lights, screen font; support.json Beta; tests with a stand-in device
- [ ] 1.3 Try it on the hardware (pad order, colours, screen, NI Hardware Agent on macOS)

## 2. More controllers

- [ ] 2.1 Novation Launchpad (Mini MK3, X, Pro MK3) in Programmer mode: 8x8 shows four rows of 16 steps
- [ ] 2.2 Akai pad controllers (MPD, MPK, APC, Fire) with note-velocity lights
- [ ] 2.3 Groove controls in the mapper's function list, and the Steps pad page on DJ controllers' pads

## Workflow follow-up

- Archive the change once its last step is merged, so the main specs take its requirements.
