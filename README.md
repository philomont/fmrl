# FMRL

**Fragile Manuscript Record Layer** — An ephemeral media codec and file format (`.fmrl`) where visual degradation is a core design feature. Images decay over time and with repeated access, simulating the natural aging of physical media.

## Overview

FMRL is intentionally designed to be ephemeral. Unlike traditional image formats that preserve data perfectly, FMRL files:

- **Age over time**: Files visibly degrade based on temporal decay (days since creation)
- **Wear with use**: Each viewing accelerates the aging process
- **Are deterministic**: The same file at the same state renders identically across all platforms
- **Preserve the original**: Optional ORIG chunk allows reconstruction (if stored)

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/) (for WebAssembly builds)
- [just](https://github.com/casey/just) (optional, for convenient command running)

### Building

```bash
# Build the Rust library
cargo build

# Build WebAssembly module
wasm-pack build --target web

# Or use just (see Justfile commands below)
just build
just wasm
```

### Running Tests

```bash
# Run all tests
cargo test

# Or with just
just test
```

### Web Demo

```bash
# Build WASM and serve locally
just dev

# Or manually
wasm-pack build --target web
python3 -m http.server 8080 --directory docs/
```

Then open http://localhost:8080 in your browser.

### Theme Configuration

Theme palettes are defined in `fmrl.toml` under `[themes.*]` sections. To sync theme changes to the web app:

```bash
# Sync themes from fmrl.toml to docs/themes.json
just sync-themes

# Or use deploy-all which syncs, builds, and serves
just deploy-all
```

## Justfile Commands

This project includes a [Justfile](https://github.com/casey/just) for convenient command running. Install `just` and use these commands:

### Build Commands

| Command | Description |
|---------|-------------|
| `just build` | Build the Rust library (debug mode) |
| `just build-release` | Build optimized release version |
| `just wasm` | Build WebAssembly module for web |
| `just wasm-release` | Build WASM in release mode |
| `just build-all` | Build both native and WASM targets |

### Test Commands

| Command | Description |
|---------|-------------|
| `just test` | Run all tests |
| `just test-verbose` | Run tests with output visible |
| `just test-one <name>` | Run a specific test by name |
| `just test-roundtrip` | Run encode/decode roundtrip tests |
| `just test-decay` | Run decay determinism tests |
| `just test-chunk` | Run chunk parsing tests |
| `just test-age` | Run age mutation tests |
| `just test-coverage` | Generate test coverage report |

### Code Quality Commands

| Command | Description |
|---------|-------------|
| `just check` | Run clippy lints |
| `just check-all` | Run clippy with all features |
| `just fmt` | Format code with rustfmt |
| `just fmt-check` | Check formatting without modifying |
| `just ci` | Run full CI checks (format, lint, test) |

### Web Demo Commands

| Command | Description |
|---------|-------------|
| `just serve` | Serve web demo on port 8080 (Python) |
| `just serve-npx` | Serve using npx serve on port 8080 |
| `just dev` | Build WASM and serve (development) |
| `just deploy` | Build WASM and serve on port 8080 |
| `just deploy-all` | Sync themes, build WASM, and serve |
| `just sync-themes` | Sync theme palettes from fmrl.toml to docs/themes.json |
| `just halt` | Stop the running demo server on port 8080 |
| `just halt` | Stop the running demo server on port 8080 |

### Utility Commands

| Command | Description |
|---------|-------------|
| `just clean` | Clean build artifacts |
| `just clean-all` | Deep clean including target/ |
| `just docs` | Generate and open documentation |
| `just update` | Update Cargo dependencies |
| `just outdated` | Check for outdated deps |
| `just audit` | Run security audit |
| `just smoke` | Quick build + test verification |
| `just all` | Full build, test, and check cycle |
| `just release` | Prepare release build |

### View All Commands

```bash
just --list
```

## Configuration

FMRL can be configured via the `fmrl.toml` file. Key settings include:

### Color Modes

```toml
[color]
# "indexed" = 4-color palette (classic FMRL)
# "rgba" = full 8-bit RGB + alpha transparency
mode = "rgba"

# Default transparency for new images (0-255)
default_alpha = 0  # 0 = fully transparent, 255 = opaque
```

### Decay Settings

```toml
[decay]
# Days until maximum fade
base_decay_days = 30

# Enable temporal and usage-based decay
enable_temporal_decay = true
enable_usage_decay = true
```

### Encoding Settings

```toml
[encoding]
# Zlib compression level (0-9) — codec uses best (9) for smallest files
compression_level = 9

# Store original strokes for reconstruction
store_original = false
```

See `fmrl.toml` for all available configuration options.

## File Format

FMRL uses a PNG-like chunked binary format:

| Chunk | Contents |
|-------|----------|
| `IHDR` | Width, height, decay policy, color mode |
| `DATA` | Compressed tiles + palette |
| `AGE`  | Per-tile decay metadata |
| `ORIG` | Optional: original strokes for reconstruction |
| `META` | Optional: JSON metadata |
| `IEND` | Terminator |

### Color Modes

**Indexed Mode (16-color)**:
- 16-color grayscale palette (index 0 = paper, 1-15 = aging colors)
- Full-byte storage per pixel (no packing)
- Theme-agnostic: brightness maps to palette indices
- Smaller file sizes than RGBA

**RGBA Mode (Full Color)**:
- Full 8-bit RGB + 8-bit alpha per pixel
- Preserves exact colors (no quantization)
- Larger files but full fidelity

## Architecture

### Encoding Pipeline (Aging Applied Here)

```
Raw Pixels (RGBA)
  → Color quantization (if indexed) or pass-through (if RGBA)
  → Apply aging step (based on age_type)
     • Erosion: morphological erosion + short-run elimination
     • Consolidation: progressive block merging
     • Bleach: convolutional pattern detection
  → 32×32 tile partitioning
  → zlib/DEFLATE per tile (best compression)
  → .fmrl binary (chunked)
```

### Decode Pipeline (Display Only)

```
.fmrl file
  → Decompress tiles
  → Map palette indices to theme colors (indexed mode)
  → RGBA pixel output
```

## API

### Rust API

```rust
use fmrl::{encode, decode, render, FmrlImage, Palette, ColorMode};

// Encode with indexed mode (4-color palette quantization)
let image = FmrlImage::new(width, height, rgba_pixels);
let fmrl_bytes = encode(&image, now_ms)?;

// Encode with RGBA mode (full color preservation)
let rgba_image = FmrlImage::new_rgba(width, height, rgba_pixels);
let fmrl_bytes = encode(&rgba_image, now_ms)?;

// Decode and render with decay
let mut decoded = decode(&fmrl_bytes)?;
let rgba_output = render(&mut decoded, now_ms, &mut fmrl_bytes)?;

// Check color mode
let is_rgba = decoded.ihdr.color_mode == ColorMode::Rgba;
```

### WebAssembly API

```javascript
import init, { FmrlView, encode_rgba, encode_rgba_full, decode_to_rgba } from './pkg/fmrl.js';

await init();

// Load and view an FMRL file
const view = FmrlView.new(fmrlBytes);
const rgbaPixels = view.decode_and_decay();
const updatedBytes = view.get_mutated_bytes();

// Check color mode
const isRgba = view.is_rgba(); // true for RGBA mode, false for indexed

// Encode with indexed mode (4-color palette quantization)
const fmrlData = encode_rgba(rgbaPixels, width, height);

// Encode with RGBA mode (full color preservation)
const fmrlDataFullColor = encode_rgba_full(rgbaPixels, width, height);

// Decode to raw RGBA (works for both modes)
const rawRgba = decode_to_rgba(fmrlBytes);
```

## Decay Model

Aging occurs at **encode time** (during save), not during viewing. Three algorithms available:

- **Erosion** (`age_type=0`): Morphological erosion + short-run elimination
- **Consolidation** (`age_type=1`): Progressive block merging (2×2 → 4×4 → 8×8 → 16×16 → paper)
- **Bleach** (`age_type=2`): Convolutional cleaning of noisy 2×2 patterns

Each save applies one aging step based on the file's configured `age_type`. The AGE chunk tracks per-tile consolidation levels and timestamps.

## Testing

The test suite includes:

- **Roundtrip tests**: Encode → decode pixel comparison
- **Chunk parsing tests**: CRC verification, unknown chunk handling
- **Decay determinism tests**: Identical inputs produce identical outputs
- **Age mutation tests**: In-place AGE chunk updates

```bash
# Run all tests
cargo test

# Run with output visible
cargo test -- --nocapture
```

## License

MIT License - See LICENSE file for details.

## Contributing

Contributions are welcome! Please ensure:

1. Code passes `cargo clippy -- -D warnings`
2. All tests pass (`cargo test`)
3. Code is formatted (`cargo fmt`)
4. New features include tests

Use `just ci` to run the full check suite before submitting.

## Acknowledgments

FMRL is inspired by the ephemeral nature of physical media — manuscripts that yellow, photographs that fade, memories that blur at the edges. The degradation is not a bug; it is the feature.
