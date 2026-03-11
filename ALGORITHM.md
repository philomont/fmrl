# FMRL Algorithm Documentation

**Fragile Manuscript Record Layer** — An ephemeral image codec where degradation is a design feature.

---

## Overview

FMRL stores images using a 16-color grayscale palette. Files age with every viewing through morphological erosion, genuinely losing information rather than obscuring it with noise.

**Version:** 0.4.0 (16-color indexed format)

---

## Storage Format

### Palette Structure

The format uses a 16-entry RGB palette (48 bytes total). Each entry is 3 bytes (R, G, B).

| Index | Grayscale Value | Alpha | Role |
|-------|-----------------|-------|------|
| 0 | 255 (white) | 0 | Paper/Background — does not age |
| 1 | 0 (black) | 255 | Darkest color — ink |
| 2 | 17 | 255 | Dark gray |
| 3 | 34 | 255 | |
| ... | ... | 255 | |
| 14 | 238 | 255 | Light gray |
| 15 | 255 | 255 | Lightest non-paper |

**Key properties:**
- Index 0 is special: treated as transparent (alpha=0) and does not age
- Indices 1-15 age toward index 0 (paper) over time
- Each step decrements by 17 (256/15 ≈ 17) from white to black

---

## Quantization Algorithm

### Input
RGBA pixel: `(r, g, b, a)` where each channel is 0-255.

### Output
Palette index: `0` to `15`

### Pseudocode

```
function quantize(r, g, b, a) -> u8:
    // Step 1: Alpha check — transparent pixels become paper
    if a < 128:
        return 0                    // paper (transparent)

    // Step 2: Map brightness to color indices 1-15
    brightness = (r + g + b) / 3    // 0-255

    // 15 color steps, each ~17 units wide
    step = 256 / 15                 // ≈ 17
    color_idx = (brightness / step).min(14) + 1

    return color_idx as u8          // 1-15
```

### Mapping

| Brightness Range | Output Index | Color |
|------------------|--------------|-------|
| 0-16 | 1 | Black (ink) |
| 17-33 | 2 | Dark gray |
| 34-50 | 3 | |
| ... | ... | Graduated steps |
| 221-237 | 14 | Light gray |
| 238-255 | 15 | Lightest |

---

## File Format Structure

```
.fmrl file
├── Header (8 bytes)
│   └── "FMRL" + 0x0D 0x0A 0x1A 0x0A
│
├── IHDR chunk
│   ├── width (u16 BE)
│   ├── height (u16 BE)
│   ├── bit_depth (u8 = 8)
│   ├── color_type (u8 = 3 indexed, 6 RGBA)
│   ├── compression (u8 = 0)
│   ├── filter (u8 = 0)
│   ├── interlace (u8 = 0)
│   └── decay_policy (u8)
│
├── DATA chunk (indexed mode)
│   ├── palette (48 bytes: 16 colors × 3 RGB)
│   └── tiles: for each 32×32 tile:
│       ├── compressed_len (u16 LE)
│       ├── flags (u8)
│       └── zlib compressed data
│           └── raw indices (1 byte per pixel)
│
├── DATA chunk (RGBA mode)
│   ├── paper_color (3 bytes RGB)
│   └── tiles: raw RGBA per tile, zlib compressed
│
├── AGE chunk (one entry per tile)
│   ├── tx (u16 LE)          // tile x coordinate
│   ├── ty (u16 LE)          // tile y coordinate
│   ├── last_view (u64 LE)   // epoch milliseconds
│   ├── fade_level (u8)      // accumulated decay
│   ├── noise_seed [4]       // per-tile PRNG seed
│   ├── edge_damage (u8)     // accumulated edge erosion
│   └── reserved (u16)
│
├── META chunk (optional)
│   └── zlib compressed JSON
│
└── IEND chunk (terminator)
```

---

## Encoding Pipeline

```
Raw RGBA pixels
       │
       ▼
┌─────────────────┐
│ Quantize pixels │  ← alpha-aware, 16 grayscale levels
│ to 16 indices   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Apply age_step  │  ← morphological erosion
│ (optional)      │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Partition into  │
│ 32×32 tiles     │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ zlib compress   │  ← per tile
│ each tile       │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Write chunks    │
│ with CRC-32     │
└────────┬────────┘
         │
         ▼
    .fmrl bytes
```

---

## Aging Algorithm

FMRL aging uses morphological erosion to gradually reduce non-paper pixels.

### Phase 1: Morphological Erosion

A non-paper pixel (index > 0) becomes paper if it has **≥ 4 paper 8-neighbors**.

```
    N
  W C E    // C = center pixel being evaluated
    S
   NW NE
   SW SE

if index[C] > 0:  // not paper
    paper_neighbors = count(index[N,S,E,W,NW,NE,SW,SE] == 0)
    if paper_neighbors >= 4:
        index[C] = 0  // erode to paper
```

### Phase 2: Short-Run Elimination

After erosion, eliminate thin isolated features:

1. **Horizontal pass**: Find runs of non-paper pixels ≤ 2 wide, replace with paper
2. **Vertical pass**: Same for columns

### Convergence

The algorithm converges to all-paper (all indices = 0) because:
- Index 0 is the only fixed point
- All operations only convert non-paper to paper
- No operation creates non-paper pixels

---

## Decoding Pipeline

```
.fmrl file
     │
     ▼
┌─────────────────┐
│ Parse chunks    │  ← verify CRC-32
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Decompress tiles│
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Apply decay     │  ← temporal fade toward paper
│ (optional)      │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Update AGE      │  ← mutate timestamps
│ chunk in-place  │
└────────┬────────┘
         │
         ▼
   RGBA output
```

---

## Decay Model

Optional temporal decay based on `last_view` timestamp:

1. **Calculate age**: `age_ms = now_ms - last_view`
2. **Fade factor**: `fade = min(1.0, age_ms / (30 days in ms))`
3. **Apply to render**:
   - Pixels fade toward paper color based on fade factor
   - Edge pixels may stochastically convert to paper

---

## Byte Layout Reference

### Chunk Structure

```
[length: u32 BE][type: 4 bytes][data: length bytes][crc: u32 BE]
```

### AGE Entry (22 bytes)

```
Offset  Size    Field
──────  ────    ─────
0       2       tx (u16 LE)
2       2       ty (u16 LE)
4       8       last_view (u64 LE)
12      1       fade_level (u8)
13      4       noise_seed [u8; 4]
17      1       edge_damage (u8)
18      2       reserved (u16)
20      2       padding (zero)
```

---

## Implementation Notes

### Core Codec (Rust)

| Function | File | Purpose |
|----------|------|---------|
| `quantize_pixel()` | `src/encode.rs` | RGBA → 16 palette indices |
| `age_step()` | `src/age.rs` | Morphological erosion |
| `Palette::default()` | `src/format.rs` | 16-entry grayscale palette |
| `PALETTE_SIZE` | `src/format.rs` | Constant = 16 |

### WASM Surface

- `encode_rgba()` — encode with indexed mode
- `encode_rgba_full()` — encode with RGBA mode
- `decode_to_indices()` — decode to palette indices
- `decode_to_rgba()` — decode to RGBA pixels
- `age_step_indices()` — apply one aging step

---

## Design Principles

1. **Determinism**: Same file + same state = same output everywhere
2. **Information loss**: Aging removes pixels permanently
3. **Self-contained**: All state in AGE chunk
4. **Extensible**: 16-color palette supports future theme mapping
