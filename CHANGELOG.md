# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-03-12

### Added

- **Consolidation aging algorithm** — Progressive block merging with per-pixel age tracking
  - 2×2 blocks consolidate first (age 0 → 1), then 4×4 (age 1 → 2), 8×8 (age 2 → 3), 16×16 (age 3 → 4)
  - New drawings age alongside existing content via mixed-age block handling
  - Per-pixel ages allow independent aging of overlapping strokes
- **Bleach aging algorithm** — Convolutional pattern cleaning using sliding 2×2 windows
  - Bleaches blocks with 3+ different indices (information-rich/noisy)
  - Bleaches imbalanced 3:1 patterns
  - Bleaches anti-diagonal [[a,b],[b,a]] arrangements
- **Age type selector** — Web UI dropdown to choose between Erosion (0), Consolidation (1), and Bleach (2)
- `encode_rgba_with_pixel_ages()` WASM export — encode with per-pixel age tracking for consolidation mode

### Changed

- **Maximum zlib compression** — Changed from `Compression::default()` (level 6) to `Compression::best()` (level 9) for ~5-10% smaller file sizes
- Updated documentation (README, ALGORITHM.md, web app About panel) to describe all three aging techniques
- Version bump to 0.5.0

## [0.4.0] - 2026-03-11

### Added

- Theme-independent 16-color grayscale storage palette
- Consolidation aging with per-pixel tracking (initial implementation)
- Bleach convolution algorithm (initial implementation)

### Changed

- Storage format uses full bytes per pixel (no nibble packing)
- AGE chunk now tracks `fade_level` as consolidation level per tile

## [0.3.0] - 2026-03-10

### Added

- Theme-independent storage format using grayscale palette — files now store theme-agnostic palette indices (ink=black, paper=transparent, accent=white, highlight=gray) rather than theme colors, eliminating save/load color corruption when switching themes
- Alpha-aware quantization: transparent pixels (alpha < 128) map to paper (index 1); opaque pixels use brightness thresholds (ink < 64, accent > 191, highlight 64-191)
- Debug mode checkbox in About tray: when enabled, Save downloads both `.fmrl` and `.png` for inspection
- `indicesToGrayscaleRgba()` in JS — converts palette indices to grayscale RGBA using the storage palette for debug PNG generation

### Changed

- Re-enabled aging on save: one `age_step` applied during encode pipeline (was temporarily disabled); Save now ages the image visibly
- Storage palette defined in both Rust (`format.rs`) and JS — single source of truth for theme-independent encoding

### Fixed

- Theme-dependent color mapping causing incorrect colors after save/load cycle — grayscale storage ensures deterministic roundtrip regardless of active theme
- Palette index 1 (paper) now correctly stored as transparent; index 2 (accent) and index 3 (highlight) distinguished by brightness rather than theme color values

## [0.2.0] - 2026-03-09

### Added

- Whiteboard layout: canvas fills the full viewport (sized to the next multiple of 32, larger dimension capped at 1024 px for performance); CSS scales to fill the screen — intentionally pixelated at high display resolutions
- Persistent action bar pinned to the bottom of the screen: Age | Age ×10 | Auto (passive) toggle + rate control | byte-size metric | Clear | Save | Load | ℹ about; scrolls horizontally on narrow screens
- Left drawing toolbar (always visible): color swatches, brush sizes, T button; sits above the action bar on mobile (≤540 px)
- Two-tier aging: `_doAgeStep(src, full)` — `full=true` (erosion + short-run elimination) for Age / Age ×10 and save; `full=false` (erosion only) for auto aging so high-rate steps feel fluid
- Save bakes 10 full erosion steps into the encoded copy; opening applies one more — images are visibly older each time the file changes hands
- Text tool (`T` button): click canvas to place a baseline cursor, type to render text in the current palette color using National Park; Enter advances to the next line, Escape cancels without committing; font size tracks the active brush (fine → 16 px, medium → 40 px, thick → 80 px)
- National Park variable-weight woff2 font family (`ExtraLight` → `ExtraBold`) added to `docs/fonts/`; declared via `@font-face` with an async preload on init
- About/info tray (ℹ button): slides in from the right on desktop, up from the bottom on mobile
- Auto aging rate display shortened to compact form (`500ms`, `1s`) to fit the action bar

### Changed

- Brush radii halved — r = 2 / 6 / 14 (was 4 / 12 / 28) so stroke widths are proportional to corresponding font sizes
- Rate display format condensed: `500ms` / `1s` instead of `500 ms / step` / `1 s / step`

### Fixed

- Canvas dimensions rounded to nearest multiple of 32 before `encode_rgba`; `window.innerWidth/innerHeight` are almost never divisible by 32, causing a WASM panic that blocked all interaction
- Switching away from the text tool no longer leaves a cursor `|` artifact burned into the canvas; `setTextMode(false)` now always stops the blink timer and restores `textBaseIndices` before clearing state
- Auto aging no longer freezes `textBaseIndices`; the snapshot is aged in sync with `indices` on each passive step, preventing the blink-restore from overwriting aged content while typing

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
