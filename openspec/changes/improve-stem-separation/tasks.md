# Tasks

## 1. Benchmark spike (no app changes)

- [x] 1.1 Spleeter 4stems in Rust (`crates/separation`): weights converted to safetensors (`scripts/models/convert_spleeter.py`), STFT/iSTFT and masking in Rust; burn and CPU versions match a TensorFlow reference built from the original checkpoint to 69–78 dB SDR. HTDemucs through demucs-rs's burn port (vendored as `third_party/demucs-core`); no ONNX export needed
- [ ] 1.2 Time both with `stem-bench` (burn GPU: Metal and Vulkan; CPU) on Seba's Mac and on CI (Linux and Apple Silicon runners, `.github/workflows/stems-bench.yml`); `ort` + CoreML only if burn misses the bars
- [ ] 1.3 Pick the runtime; record the numbers in this design.md

## 2. Separation crate

- [ ] 2.1 `crates/separation`: model download (GitHub release asset, SHA-256), STFT, Spleeter and HTDemucs on the chosen runtime, CPU fallback; unit tests on short generated signals
- [ ] 2.2 Worker: one queue for all decks, fast-tier priority, playhead-first ordering, pause on underruns/dropped frames

## 3. Engine

- [ ] 3.1 Two-layer stems store (fast, high quality) with per-chunk swap and 20 ms crossfade; tests that a swap is click-free
- [ ] 3.2 Original-preserving mix for separated tracks (`original − Σ(1 − g)·stem`); NI files keep the sum; tests that unity gains play the original exactly
- [ ] 3.3 Cache only high-quality stems (three files, Other derived), keep reading old Demucs caches, size limit with least-recently-loaded eviction
- [ ] 3.4 Keep the Demucs command as a fallback for one release

## 4. App

- [ ] 4.1 Settings > Stems: Stems on load, Background quality, Stems cache limit and usage, model download state
- [ ] 4.2 STEMS pad page: stems available after the first window, quality badge (Instant / HQ) while the swap is in progress
- [ ] 4.3 Browser: stems mark for tracks with cached high-quality stems; Separate stems on a list, crate or set (track menu)
- [ ] 4.4 Update the user guide page website/book/src/guide/stems.md and the Settings page

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.
