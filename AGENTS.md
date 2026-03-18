# AGENTS.md

This file provides guidance to AI coding agents (Claude Code, OpenCode) when working with code in this repository.

## Context Files

For detailed module documentation, see:
- `src/CONTEXT.md` — Rust codec architecture, aging algorithms, naming conventions
- `docs/CONTEXT.md` — Web app structure, WASM integration, UI components

## Project Overview

**FMRL** ("Fragile Manuscript Record Layer") is an ephemeral media codec and file format (`.fmrl`) where visual degradation is a core design feature. Images decay over time and with repeated access — simulating the natural aging of physical media. This is intentional art/archival design, not a bug.

## Tech Stack

- **Core codec**: Rust (primary implementation)
- **Web platform**: WebAssembly via `wasm-pack` + `wasm-bindgen`
- **Compression**: `flate2` crate (zlib/DEFLATE, same as PNG)
- **Mobile**: Rust WASM → iOS (WasmKit) / Android (wasm3 runtime) — future
- **CLI**: Cargo binary target — future

## Build Commands

```bash
# Build WebAssembly module
wasm-pack build --target web

# Build and test Rust library
cargo build
cargo test
cargo test <test_name>   # Run a single test

# Lint
cargo clippy

# Serve web demo locally (port 8080)
just serve
# or
python3 -m http.server 8080 --directory docs/
```

## Architecture

### File Format — `.fmrl` (PNG-like chunked binary)

| Chunk | Contents |
|-------|----------|
| `IHDR` | Width, height, color mode, decay policy, **age_type_count**, **age_types[]** (variable: 10-18 bytes) |
| `DATA` | Palette (48B) + tile data (packed index+age) |
| `AGE`  | Per-tile metadata: `last_view`, `fade_level`, `noise_seed`, `edge_damage` (18 bytes each, compressed) |
| `ORIG` | Optional: original strokes for reconstruction |
| `META` | Optional: JSON metadata (`author`, `tags`, `decay_rate`) |
| `IEND` | Terminator |

**IHDR Format** (variable length):
```
width(2) + height(2) + bit_depth(1) + color_type(1) + 
compression(1) + filter(1) + interlace(1) + decay_policy(1) + 
age_type_count(1) + age_types[0..count] (1-8 bytes)
```

**Age Types** (stackable, up to 8):
- `0` = Erosion (morphological erosion)
- `1` = Consolidation (progressive block merging)
- `2` = Bleach (convolutional pattern detection)
- `3-7` = Reserved for future built-in algorithms
- `8-254` = Available for experimental algorithms

### RGB Interface Format

The codec now uses a specialized RGB format for input/output:

**Encoding Input** (3 bytes per pixel):
- `R = index × 16` (0, 16, 32, ... 240 for indices 0-15)
- `G = 0x00` if index=0 (paper), `0xFF` otherwise (contrast)
- `B = age × 16` (0, 16, 32, ... 240 for ages 0-15)

**Decoding Output**:
- Same format as encoding input
- Allows roundtrip preservation of index and age data

### Encoding Pipeline

```
RGB Pixels (R=index×16, G=contrast, B=age×16)
  → Extract index and age from RGB channels
  → Apply aging algorithm stack sequentially
    → For each age_type in stack: apply algorithm
  → Pack index + age into nibbles (1 byte/pixel)
  → Compress tiles with zlib
  → Save .fmrl file (IHDR + DATA + AGE + IEND)
```

### Algorithm Stacking

Multiple aging algorithms can be applied in a single encoding:

```rust
image.age_types = vec![
    AgeType::Erosion,       // First apply erosion
    AgeType::Consolidation, // Then consolidation
    AgeType::Bleach,        // Finally bleach
];
```

Each algorithm is applied sequentially, with the output of one feeding into the next. This allows creating unique aging styles by combining algorithms.

### Decoding Pipeline

```
.fmrl file
  → Parse chunks, verify CRC
  → Decompress tile data
  → Unpack index + age from nibbles
  → Convert to RGB format (R=index×16, G=contrast, B=age×16)
  → Return RGB pixels
```

### Decay Model

- **Aging at encode time**: All degradation happens during `encode()`, not during display
- **Per-tile age tracking**: `AGE.fade_level` stores consolidation level (0=initial, 1=2×2 done, 2=4×4 done, etc.)
- **Content-first aging**: Tiles with non-paper content are prioritized for aging
- **Deterministic degradation**: Same input + same age levels → identical output
- **Information reduction**: Each aging step reduces file size by creating larger uniform areas

### Web App / Tool Role

The web demo (`docs/index.html`, `docs/index.js`) is a **display tool only**:
- Drawing operations modify the canvas directly
- **Age button**: Triggers save → load cycle (encode → decode → display)
- **Save button**: Encodes with current age_types stack, downloads `.fmrl` file
- **Load button**: Decodes file, displays on canvas (no aging applied)

Aging algorithms live in Rust (`src/age.rs`):
- `age_by_erosion()` — erosion-based aging
- `age_by_consolidation()` — progressive block consolidation
- `age_by_bleaching()` — convolutional pattern bleaching

### Source Layout

```
├── AGENTS.md           # This file — high-level project guidance
├── CLAUDE.md           # Claude Code guidance (same content, slightly different headers)
├── src/
│   CONTEXT.md          # Rust codec documentation (see this!)
│   RUST_DOCS.md        # Rust crate API documentation
│   lib.rs              # Core encoder/decoder + public API
│   format.rs           # File format definitions: ColorMode, Palette, IHDR, AgeType, chunks, CRC
│   encode.rs           # Encoding: RGB input, algorithm stacking, aging
│   decode.rs           # Decoding: TileData with indices(), pixel_ages(), to_rgb()
│   age.rs              # Aging algorithms: age_by_erosion, age_by_consolidation, age_by_bleaching
│   decay.rs            # Rendering: temporal decay, fade-to-paper
│   prng.rs             # xoshiro128++ per-tile deterministic PRNG
│   error.rs            # FmrlError enum for all error conditions
│   wasm.rs             # wasm-bindgen WASM bindings (FmrlView type, WASM exports)
├── docs/
│   CONTEXT.md            # Web app documentation (see this!)
│   index.html            # Web demo with HTML5 Canvas (display only, no aging logic)
│   index.js              # Canvas drawing + WASM integration (calls encode/decode)
│   style.css
│   pkg/                  # WASM build output (generated by wasm-pack)
├── tests/
│   roundtrip.rs          # Encode/decode pixel comparison tests
│   chunk_parse.rs        # CRC validation, unknown chunks, EOF handling
│   decay_det.rs          # Decay determinism tests
│   age_mutation.rs       # In-place AGE mutation + CRC recompute
│   aging_decay.rs        # Aging algorithm convergence tests
```

### Color Mode

FMRL uses **Indexed Mode only** (16-color palette):
- 16-color palette (index 0 = paper, 1-15 = colors that age toward paper)
- Packed storage: high nibble = index, low nibble = age (1 byte/pixel)
- Theme-agnostic: grayscale brightness maps to palette indices
- Small file sizes due to packing + compression

### WASM Surface

The `wasm-bindgen`-exposed type is `FmrlView` with methods like:
- `decode_and_decay()` — returns RGBA pixel data applying current decay state
- `get_mutated_bytes()` — returns updated file bytes with new AGE state
- `view_count()` — returns number of times the image has been viewed
- `age_types()` — returns comma-separated age type values (e.g., "0,1,2")
- `age_levels()` — returns per-tile consolidation levels
- `pixel_ages()` — returns per-pixel ages extracted from packed data

**Encoding Functions**:
- `encode_rgb(rgb, width, height, age_types)` — encode from RGB format with algorithm stack
- `encode_rgb_with_levels(rgb, width, height, age_types, age_levels)` — encode with existing age levels

**Decoding Functions**:
- `decode_to_rgb(data)` — decode to RGB visualization format (R=index×16, G=contrast, B=age×16)

**Aging Functions**:
- `consolidation_step_indices(data, width, height)` — apply one consolidation step
- `bleach_step_indices(data, width, height)` — apply one bleach step
- `consolidation_step_with_ages(indices, pixel_ages, width, height)` — consolidation with per-pixel ages

### Breaking Changes from v0.4.0

1. **Removed RGBA mode**: Only indexed mode is now supported
2. **Changed input format**: Encoder now expects RGB (3 bytes/pixel) instead of RGBA (4 bytes/pixel)
3. **Algorithm stacking**: Single `age_type` replaced with `age_types` vector (up to 8 algorithms)
4. **IHDR format**: Now variable-length to accommodate algorithm stack
5. **AGE entry size**: Reduced from 22 to 18 bytes (removed `reserved` field)