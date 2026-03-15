# FMRL Rust Codebase Context

## Project Overview

FMRL (Fragile Manuscript Record Layer) is an ephemeral media codec and file format (`.fmrl`) where visual degradation is a core design feature. Images decay over time and with repeated access — simulating the natural aging of physical media.

## Architecture

### Module Structure

```
src/
├── lib.rs       # Public API exports, now_ms(), render()
├── age.rs       # Aging algorithms (erosion, consolidation, bleach)
├── decay.rs     # Temporal decay rendering, tile rendering
├── decode.rs    # File decoding, TileData
├── encode.rs    # File encoding, FmrlImage
├── error.rs     # FmrlError types
├── format.rs    # File format definitions (IHDR, chunks, Palette)
├── prng.rs      # Deterministic PRNG for aging
└── wasm.rs      # WASM bindings (wasm-bindgen)
```

### Aging Algorithms

Three distinct aging methodologies are supported:

1. **Erosion** (`age_step`)
   - Morphological erosion + short-run elimination
   - Gradual edge erosion preserving core regions
   - Two-pass: neighbor check, then run-length elimination

2. **Consolidation** (`consolidation_step_with_pixel_ages`)
   - Hierarchical block merging (2×2 → 4×4 → 8×8 → 16×16)
   - Per-pixel age tracking
   - Youngest pixel in block drives consolidation

3. **Bleach** (`bleach_step`)
   - Convolutional 2×2 window detection
   - Targets noisy/complex patterns
   - Preserves uniform regions

### Key Data Structures

- **Canvas/Indices**: Flat array of palette indices (0-15), where 0 is paper
- **Ages**: Per-pixel age tracking for consolidation (0-4)
- **Block Sizes**: Configurable progression (default: [2, 4, 8, 16])
- **Palette**: 16-color RGB palette, index 0 = paper color

### WASM Surface

The `wasm.rs` module exposes:
- `FmrlView` - Main interface for web apps
- `encode_rgba*()` - Encoding functions
- `decode_to_*()` - Decoding functions
- `consolidation_step_with_ages()` - Direct aging step access

## Naming Conventions

### Functions

- `*_step` - Apply one step of an algorithm (e.g., `age_step`, `bleach_step`)
- `*_with_*` - Variant with additional parameters
- `min_*_in_region` - Region query functions
- `is_*_bleachable` - Boolean check functions

### Types

- `PAPER_INDEX` - Constant for paper (0)
- `RUN_THRESHOLD` - Short-run elimination threshold
- `TILE_SIZE` - 32×32 tile dimension
- `PALETTE_SIZE` - 16 colors

## Extension Points

To add a new aging algorithm:

1. Add function to `age.rs` following the `*_step` pattern
2. Export from `lib.rs` if needed publicly
3. Add WASM binding in `wasm.rs` if web access needed
4. Update documentation in both Rust docs and CONTEXT.md

## Build Commands

```bash
# Native build
cargo build

# WASM build
wasm-pack build --target web --features wasm

# Tests
cargo test

# Linting
cargo clippy -- -D warnings
```

## Design Principles

1. **Information non-increasing** - Aging only converts to paper
2. **Deterministic** - Same input produces same output
3. **Reversible encoding** - Can decode and re-encode
4. **Per-pixel aging** - Each pixel tracks its own age
5. **Block-based consolidation** - Larger blocks at higher ages

## Common Tasks

### Adding a new aging algorithm:

1. Implement in `age.rs` with clear documentation
2. Follow existing patterns (region queries, paper checks)
3. Add to WASM exports if web-facing
4. Test with `cargo test`

### Modifying block sizes:

1. Change `BLOCK_SIZES` constant in `age.rs`
2. Update age advancement logic if needed
3. Ensure WASM bindings match

### Adding WASM exports:

1. Add function to `wasm.rs` with `#[wasm_bindgen]`
2. Handle type conversions (Vec<u8>, etc.)
3. Update TypeScript definitions if applicable
