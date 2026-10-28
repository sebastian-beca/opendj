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

- [ ] 3.1 Two-layer stems store
  - v0.4: HQ stems replace the fast ones as a whole once decoded (`Stems::set_upgrade`); no per-chunk swap or crossfade yet. Since unity gains play the original either way, only muted stems change at the swap (fast, high quality) with per-chunk swap and 20 ms crossfade; tests that a swap is click-free
- [x] 3.2 (v0.4) Original-preserving mix for separated tracks (`original − Σ(1 − g)·stem`); NI files keep the sum; tests that unity gains play the original exactly
- [ ] 3.3 Cache only high-quality stems (three files, Other derived), keep reading old Demucs caches, size limit with least-recently-loaded eviction
- [x] 3.4 (v0.4) Keep the Demucs command as a fallback for one release (when the HTDemucs model can't be downloaded or the GPU run fails)

## 4. App

- [ ] 4.1 Settings > Stems: Stems on load, Background quality, Stems cache limit and usage, model download state
- [x] 4.2 (v0.4) STEMS pad page: VirtualDJ's 8 pads (Vocal, Instru, Bass, Acapella / Kick, HiHat, Stems FX, Instrumental), no level bars (levels on the stems knobs), right-click Prepare High Quality Stems; SEPARATE STEMS removed (Seba, 2026-10-28)
- [ ] 4.2b Quality badge (Instant / HQ) on the STEMS page while the swap is in progress (v0.4 shows it only in the pads' right-click menu)
- [ ] 4.3 Browser: stems mark for tracks with cached high-quality stems; Separate stems on a list, crate or set (v0.4 has Prepare High Quality Stems in the track menu, for one or several picked tracks)
- [ ] 4.4 Update the user guide page website/book/src/guide/stems.md (done for v0.4) and the Settings page

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.
