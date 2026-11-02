# Tasks

## 1. Isolation (format-independent)

- [x] 1.1 `crates/plugin-host`: shared-memory block exchange, `opendj-plugin-host` helper, deadline with dry fallback, crash/hang detection, restart and disable policy, test effects (gain, crash, hang, slow)
- [ ] 1.2 Real-time priority for the helper's audio thread; join openDJ's audio workgroup on macOS
- [ ] 1.3 Supervisor thread in the app: restart after a crash with saved state, kill hung helpers, crash counter kept between sessions

## 2. CLAP

- [x] 2.1 Host CLAP effects in the helper with `clack-host`: load, process, parameters, transport (state comes with 1.3)
- [x] 2.2a Scanner process with a timeout (`opendj-plugin-host --scan`, `scan()`)
- [ ] 2.2 Scan cache, Settings › Plugins (folders, Scan, failures with Retry, allow-list, favourites)
- [ ] 2.3 FX chooser "Plugins" group, ON/D/W/slider mapping, transport for beat-synced plugins, controller mapping
- [ ] 2.4 Block processing for plugin slots in the effects engine; per-slot load meter and CPU budget
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

- [ ] 5.1 Package the helper with the app on all platforms; on macOS sign it with `disable-library-validation` (the app itself stays hardened)
- [ ] 5.2 Update the effects user guide page in website/book/src/guide

## Workflow follow-up

- Archive the change once its PRs are merged, so the main specs take its requirements.
