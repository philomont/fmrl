# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Whiteboard layout: canvas fills the full viewport (sized to the next multiple of 32 in each dimension); all controls moved to a slide-in tray (right on desktop, bottom-slide on mobile ≤540 px wide)
- Floating mini-toolbar stays visible on the canvas at all times: color swatches, brush sizes, and a ☰ button to open the tray; tray closes via × or backdrop tap
- Two-tier aging: `_doAgeStep(src, full)` replaces `ageStep()` — `full=true` (morphological erosion + short-run elimination, used by Age / Age 10×) for maximum data removal per step; `full=false` (erosion only, used by passive aging) for fine-grained fluid degradation at sub-second rates
- Save bakes 10 full erosion steps into the encoded copy without touching the live canvas; the next open applies one more step, making temporal decay clearly visible across save/load cycles

### Fixed

- Canvas dimensions are now rounded up to the nearest multiple of 32 before being passed to `encode_rgba`; `window.innerWidth/innerHeight` are almost never divisible by 32, causing a "malformed chunk: dimensions must be multiples of 32" WASM panic that prevented all canvas interaction

## [0.1.0] - 2026-03-08

### Added

- `.fmrl` chunked binary file format (IHDR, DATA, AGE, META, IEND) with CRC-32 verified chunks
- Encode pipeline: RGBA pixels → 4-color palette quantization → 32×32 tile partitioning → zlib/DEFLATE → `.fmrl`
- Decode pipeline with two-pass AGE chunk range tracking for in-place byte mutation
- Decay engine: pixels fade toward paper color; edge erosion stochastically converts stroke-edge pixels to paper; each render reduces information rather than adding it
- Deterministic per-tile xoshiro128++ PRNG seeded from `noise_seed` XOR tile coordinates
- `render()` — applies decay to all tiles and writes the mutated AGE chunk back into the file buffer
- `patch_age_chunk()` — reserializes AGE entries and recomputes CRC in-place
- `encode_rgba(rgba, width, height)` WASM export — encodes raw RGBA pixels to a `.fmrl` file
- `decode_to_indices(data)` WASM export — decodes a `.fmrl` file to flat palette indices for editor loading (no decay applied, no AGE mutation)
- `FmrlView` WASM surface (`decode_and_decay`, `get_mutated_bytes`, `view_count`, `last_view_ms`, `avg_fade_level`, `width`, `height`) gated behind the `wasm` feature
- `now_ms()` helper — `SystemTime` on native, `js_sys::Date::now()` under WASM
- 16 integration tests covering encode/decode roundtrip, CRC validation, unknown-chunk tolerance, decay determinism, and AGE mutation correctness
- Interactive drawing canvas web app (GitHub Pages): 1024×768 canvas at 1:1 CSS pixels, palette swatches, three brush sizes (radii 4/12/28), Age / Age 10× buttons, Save/Load `.fmrl`, touch support
- Loading a `.fmrl` file applies one erosion step before display, representing elapsed time since last save
- Passive aging toggle: applies age steps automatically while enabled, mimicking slow environmental degradation (UV bleaching, mineral dissolution); rate adjustable via − / + buttons across six intervals — 50 ms, 100 ms, 200 ms, 500 ms, 1 s, 2 s per step (default 1 s); toggle uses plain CSS active state (replaced `color-mix` which silently failed in some browsers)
- Compression size metric displayed after each age step (absolute byte count + delta from previous press)
- Sans-serif font theme (system-ui stack)
- `src/age.rs` — `age_step(indices, width, height)` encapsulates the aging algorithm in Rust; `age_step_indices` WASM export added
- Aging algorithm guaranteed to converge to all-paper: morphological erosion with ≥3 paper 8-neighbour threshold (face pixels of any solid block always have exactly 3, so no finite fixed point other than all-paper exists); followed by short-run elimination (rows then columns, runs ≤2 pixels wide become paper) to collapse thin isolated features and lengthen uniform runs for better zlib compression
- 5 integration tests in `tests/aging_decay.rs` verifying: convergence to all-paper, file size reduction over time, checkerboard decay, single-pixel erasure in one step, and the monotonic non-paper-count invariant

[0.1.0]: https://github.com/philomont/fmrl/releases/tag/v0.1.0
