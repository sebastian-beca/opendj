# Tasks

## 1. Isolation (format-independent)

- [x] 1.1 `crates/plugin-host`: shared-memory block exchange, `opendj-plugin-host` helper, deadline with dry fallback, crash/hang detection, restart and disable policy, test effects (gain, crash, hang, slow)
- [ ] 1.2 Real-time priority for the helper's audio thread; join openDJ's audio workgroup on macOS
- [ ] 1.3 Supervisor thread in the app: restart after a crash with saved state, kill hung helpers, crash counter kept between sessions

## 2. CLAP

- [ ] 2.1 Host CLAP effects in the helper with `clack-host`: load, process, parameters, state
- [ ] 2.2 Scanner process, cache, Settings › Plugins (folders, Scan, failures with Retry, allow-list, favourites)
- [ ] 2.3 FX chooser "Plugins" group, ON/D/W/slider mapping, transport for beat-synced plugins, controller mapping
- [ ] 2.4 Block processing for plugin slots in the effects engine; per-slot load meter and CPU budget
- [ ] 2.5 Latency compensation by reading the deck ahead
- [ ] 2.6 Editor windows from the helper (macOS, Windows, Linux X11)

## 3. VST3

- [ ] 3.1 Host VST3 effects with the `vst3` crate (coupler-rs) behind a safe layer: processor/controller, parameters, state
- [ ] 3.2 Scanning via `moduleinfo.json` when present; editors

## 4. AU (macOS)

- [ ] 4.1 Host AUv2 in the helper and AUv3 out of process; list from the component registry; editors

## 5. Ship

- [ ] 5.1 Package the helper with the app on all platforms; on macOS sign it with `disable-library-validation` (the app itself stays hardened)
- [ ] 5.2 Update the effects user guide page in website/book/src/guide

## Workflow follow-up

- Archive the change once its PRs are merged, so the main specs take its requirements.
