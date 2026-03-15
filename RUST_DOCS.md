# FMRL Rust Documentation

## Crate Overview

The `fmrl` crate provides an ephemeral image codec where files age with every viewing.

## Modules

### `age` - Aging Algorithms

Core aging implementations. All algorithms follow the pattern of taking indices and returning aged indices.

#### Functions

- `age_step(indices, width, height)` - Erosion-based aging
- `consolidation_step_with_pixel_ages(indices, pixel_ages, width, height)` - Block consolidation
- `bleach_step(indices, width, height)` - Pattern-based bleaching

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
use fmrl::age::consolidation_step_with_pixel_ages;

let (new_indices, new_ages) = consolidation_step_with_pixel_ages(
    &indices, &ages, width, height
);
```
