# Tasks

## 1. Isolation (format-independent)

- [x] 1.1 `crates/plugin-host`: shared-memory block exchange, `opendj-plugin-host` helper, deadline with dry fallback, crash/hang detection, restart and disable policy, test effects (gain, crash, hang, slow)
- [ ] 1.2 Real-time priority for the helper's audio thread; join openDJ's audio workgroup on macOS
- [x] 1.3a Plugin host thread in the engine (`opendj_engine::plugins`): a helper per slot and routed target, restart once with the slot off, then disable and empty the slot; hangs and long CPU overruns count; failed starts empty the slot
- [ ] 1.3 Restart with saved state; crash counter kept between sessions

## 2. CLAP

- [x] 2.1 Host CLAP effects in the helper with `clack-host`: load, process, parameters, transport (state comes with 1.3)
- [x] 2.2a Scanner process with a timeout (`opendj-plugin-host --scan`, `scan()`)
- [x] 2.2a Settings › Mixer › FX plugins: folders (system defaults), Scan in throwaway helpers, cache by file size and date (`audio-plugins.json`), failures with Retry, a tick per effect for the FX lists (allow-list)
- [ ] 2.2 Favourites
- [x] 2.3a Ticked plugins in every FX chooser under a line after the built-in effects (the list scrolls); ON, D/W and the slider (first parameter) work as for built-in effects; tempo and beat from the routed deck; crash and failure messages as alerts
- [ ] 2.3 Controller mapping of plugin parameters beyond the first four
- [x] 2.4a Block processing for plugin slots in the effects engine: `loaded_effect` 13 and up are plugins, the first four parameters on the slot knobs, tempo and beat from the routed deck
- [ ] 2.4 Per-slot load meter
- [ ] 2.5 Latency compensation by reading the deck ahead
- [ ] 2.6 Editor windows from the helper (macOS, Windows, Linux X11)

## 3. VST3

- [x] 3.1 Host VST3 effects with the `vst3` crate (coupler-rs): component, processor, separate controller (connection, component state), parameters, process context
- [x] 3.2a Scanning via `moduleinfo.json` when present, else in a throwaway helper
- [ ] 3.2 VST3 editors (with 2.6)

## 4. AU (macOS)

- [x] 4.1a Host Audio Unit effects in the helper through AudioToolbox (AUv3 extensions run in their own process, as macOS does); list from the component registry (`--scan-au`)
- [ ] 4.1 Run the AU tests on a Mac (they use Apple's AUDelay); AU editors (with 2.6)

## 5. Ship

- [x] 5.1a The bundle scripts and workflows build and ship `opendj-plugin-host` beside the app; on macOS it is signed first with `res/macos/plugin-host.entitlements` (`disable-library-validation`, JIT), the app keeps its own entitlements
- [ ] 5.1 Check a notarized build on a Mac with third-party plugins
- [ ] 5.2 Update the effects user guide page in website/book/src/guide

## Workflow follow-up

- Archive the change once its PRs are merged, so the main specs take its requirements.
