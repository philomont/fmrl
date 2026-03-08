# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- **Decay model**: replaced desaturation-toward-grayscale + salt-and-pepper noise with fade-toward-paper-color + stochastic edge erosion. Rendered output now always reduces information rather than adding it — noise increases entropy and was incorrect.
- **Web demo**: replaced passive image viewer with an interactive drawing canvas (128×128, 4× display scale). Users draw with ink/crimson/white/eraser in three brush sizes.

### Added
- `encode_rgba(rgba, width, height)` WASM export — encodes raw RGBA pixels to a `.fmrl` file
- `decode_to_indices(data)` WASM export — decodes a `.fmrl` file to flat palette indices for editor loading (no decay applied, no AGE mutation)
- **Age button**: one-press morphological 8-neighbour erosion — pixels with ≥5 paper neighbours convert to paper. Thin strokes erode first; each press demonstrably reduces compressed file size.
- Save `.fmrl` / Load `.fmrl` buttons for round-tripping files
- Touch support on the drawing canvas

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
