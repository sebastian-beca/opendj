# Design

## Decisions
- Run an htdemucs-class model in-process (ONNX via a pure-Rust or CoreML/Metal runtime) rather than shelling out to Python; keep the Demucs command as a fallback.
- Cache stems as today, keyed by file, and survive tag rewrites.

## Risks / Trade-offs
- Model size grows the download → ship it as an optional download on first use.
- GPU paths differ per platform → CPU path always works, GPU is a speed-up.
