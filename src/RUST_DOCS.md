# FMRL Rust Documentation

## Crate Overview

The `fmrl` crate provides an ephemeral image codec where files age with every viewing.

## Modules

### `age` - Aging Algorithms

Core aging implementations. All algorithms follow the pattern of taking indices and returning aged indices.

#### Functions

- `age_by_erosion(indices, width, height)` - Erosion-based aging (morphological erosion + short-run elimination)
- `age_by_consolidation(indices, pixel_ages, width, height)` - Block consolidation (hierarchical 2×2 → 4×4 → 8×8 → 16×16)
- `age_by_bleaching(indices, width, height)` - Pattern-based bleaching (2×2 convolutional detection)

*Legacy aliases (deprecated): `age_step`, `consolidation_step_with_pixel_ages`, `bleach_step`*

### `encode` - Encoding

- `encode(image, now_ms)` - Encode FmrlImage to bytes
- `FmrlImage` - Builder for encoding

### `decode` - Decoding

- `decode(bytes)` - Decode bytes to DecodedFmrl
- `DecodedFmrl` - Decoded file structure

### `format` - Format Definitions

- `Palette` - 16-color palette
- `ColorMode` - Indexed vs RGBA
- `AgeType` - Erosion, Consolidation, Bleach

### `wasm` - WebAssembly Bindings

WASM-specific exports for browser usage.

## Feature Flags

- `wasm` - Enables WASM bindings (requires wasm-bindgen, js-sys)
- `debug-logging` - Enables console logging in WASM

## Examples

### Basic encoding:

```rust
use fmrl::{FmrlImage, encode, now_ms};

let image = FmrlImage::new(width, height, rgba_bytes);
let bytes = encode(&image, now_ms()).unwrap();
```

### Basic decoding:

```rust
use fmrl::decode;

let decoded = decode(&bytes).unwrap();
```

### Applying aging:

```rust
use fmrl::age::{age_by_erosion, age_by_consolidation, age_by_bleaching};

// Erosion-based aging
let eroded = age_by_erosion(&indices, width, height);

// Consolidation with per-pixel ages
let (new_indices, new_ages) = age_by_consolidation(
    &indices, &ages, width, height
);

// Pattern-based bleaching
let bleached = age_by_bleaching(&indices, width, height);
```
