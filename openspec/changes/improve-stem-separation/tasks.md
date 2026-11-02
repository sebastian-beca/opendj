# Tasks

## 1. Benchmark spike (no app changes)

- [x] 1.1 Spleeter 4stems in Rust (`crates/separation`): weights converted to safetensors (`scripts/models/convert_spleeter.py`), STFT/iSTFT and masking in Rust; burn and CPU versions match a TensorFlow reference built from the original checkpoint to 69–78 dB SDR. HTDemucs through demucs-rs's burn port (vendored as `third_party/demucs-core`); no ONNX export needed
- [ ] 1.2 Time both with `stem-bench` (burn GPU: Metal and Vulkan; CPU) on Seba's Mac and on CI (Linux and Apple Silicon runners, `.github/workflows/stems-bench.yml`); `ort` + CoreML only if burn misses the bars
- [ ] 1.3 Pick the runtime; record the numbers in this design.md

## 2. Separation crate

- [x] 2.1 (v0.4) `crates/separation`: model download (GitHub release asset, SHA-256), STFT, Spleeter and HTDemucs on the chosen runtime, CPU fallback; unit tests on short generated signals
- [ ] 2.2 Worker: one queue for all decks, fast-tier priority, playhead-first ordering, pause on underruns/dropped frames
  - v0.4: Spleeter on load runs one track at a time on half the cores, from the cue point outwards, a chunk at a time (`spleeter::separate_in_chunks`); HQ jobs run one at a time. No playhead-first HQ or pausing yet

## 3. Engine

- [x] 3.1 Two-layer stems store
  - v0.4: HQ stems replace the fast ones as a whole once decoded (`Stems::set_upgrade`).
  - v0.5: the swap is a 20 ms crossfade of the stems themselves under one read of the original (`Stems::mix_blend`, `STEM_UPGRADE_SECONDS`); test `high_quality_stems_take_over_without_a_click`. HTDemucs separates the whole track before writing it, so there is nothing to swap per chunk; revisit if HQ ever streams chunks. Separated stems at full gain skip reading stem frames at all (the original plays), so a deck with stems costs no more than one without until a stem is turned down
- [x] 3.2 (v0.4) Original-preserving mix for separated tracks (`original − Σ(1 − g)·stem`); NI files keep the sum; tests that unity gains play the original exactly
- [ ] 3.3 Cache only high-quality stems (three files, Other derived), keep reading old Demucs caches, size limit with least-recently-loaded eviction
  - v0.5: size limit (default 10 GB, Settings) with least-recently-loaded eviction (`cache::trim_stems`, a `last-used` file touched on load), never the decks' tracks; usage and Clear. Still four files (Other not derived yet)
- [x] 3.4 (v0.4) Keep the Demucs command as a fallback for one release (when the HTDemucs model can't be downloaded or the GPU run fails)

## 4. App

- [x] 4.1 Settings > Stems (under Analysis): Stems on load, Stems cache limit and usage, Clear, model download state. Background quality waits for a "Best" tier
- [x] 4.2 (v0.4) STEMS pad page: VirtualDJ's 8 pads (Vocal, Instru, Bass, Acapella / Kick, HiHat, Stems FX, Instrumental), no level bars (levels on the stems knobs), right-click Prepare High Quality Stems; SEPARATE STEMS removed (Seba, 2026-10-28)
- [x] 4.2b Quality badge on the STEMS page: INSTANT, HQ… while preparing, HQ (top-right pad's corner)
- [ ] 4.3 Browser: stems mark for tracks with cached high-quality stems; Separate stems on a list, crate or set (v0.4 has Prepare High Quality Stems in the track menu, for one or several picked tracks)
- [x] 4.4 Update the user guide page website/book/src/guide/stems.md (v0.4, and v0.5 for the badge and Settings › Analysis › Stems)

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.
