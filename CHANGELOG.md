# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-03-08

### Added

- `.fmrl` chunked binary file format (IHDR, DATA, AGE, META, IEND) with CRC-32 verified chunks
- Encode pipeline: RGBA pixels → 4-color palette quantization → 32×32 tile partitioning → zlib/DEFLATE → `.fmrl`
- Decode pipeline with two-pass AGE chunk range tracking for in-place byte mutation
- Decay engine: temporal desaturation, noise injection, and edge erosion per tile
- Deterministic per-tile xoshiro128++ PRNG seeded from `noise_seed` XOR tile coordinates
- `render()` — applies decay to all tiles and writes the mutated AGE chunk back into the file buffer
- `patch_age_chunk()` — reserializes AGE entries and recomputes CRC in-place
- `FmrlView` WASM surface (`decode_and_decay`, `get_mutated_bytes`, `view_count`, `width`, `height`) gated behind the `wasm` feature
- `now_ms()` helper — `SystemTime` on native, `js_sys::Date::now()` under WASM
- 16 integration tests covering encode/decode roundtrip, CRC validation, unknown-chunk tolerance, decay determinism, and AGE mutation correctness

[0.1.0]: https://github.com/philomont/fmrl/releases/tag/v0.1.0
