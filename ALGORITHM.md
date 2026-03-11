# FMRL Algorithm Documentation

**Fragile Manuscript Record Layer** — An ephemeral image codec where degradation is a design feature.

---

## Overview

FMRL stores images using a theme-independent grayscale palette. Files age with every viewing through morphological erosion, genuinely losing information rather than obscuring it with noise.

---

## Storage Format

### Palette Mapping (Theme-Independent)

| Index | Grayscale | Alpha | Meaning   | Renders As                    |
|-------|-----------|-------|-----------|-------------------------------|
| 0     | [0,0,0]   | 255   | ink       | `--ink` (theme color)         |
| 1     | [255,255,255] | 0 | paper   | `--paper` (transparent)       |
| 2     | [255,255,255] | 255   | accent    | `--accent` (theme color)      |
| 3     | [128,128,128] | 255   | highlight | `--highlight` (theme color)   |

**Key insight:** Index 1 (paper) is distinguished from index 2 (accent) by alpha, not by RGB values. This allows theme-independent storage while supporting both "paper" (background/eraser) and "accent" (bright strokes).

---

## Quantization Algorithm

### Input
RGBA pixel: `(r, g, b, a)` where each channel is 0-255.

### Output
Palette index: `0 | 1 | 2 | 3`

### Pseudocode

```
function quantize(r, g, b, a) -> u8:
    // Step 1: Alpha check distinguishes paper from accent
    if a < 128:
        return 1                    // paper (transparent)

    // Step 2: Brightness for opaque pixels
    brightness = (r + g + b) / 3

    if brightness < 64:
        return 0                    // ink (dark)
    else if brightness > 191:
        return 2                    // accent (bright)
    else:
        return 3                    // highlight (mid)
```

### Decision Tree

```
                    alpha < 128?
                   /            \
                 YES             NO
                  |               |
               paper(1)      brightness?
                            /    |    \
                         <64   64-191   >191
                          |      |       |
                        ink(0) highlight(3) accent(2)
```

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
├── DATA chunk
│   ├── Indexed mode:
│   │   ├── palette (12 bytes: 4 colors × 3 RGB)
│   │   └── tiles: for each 32×32 tile:
│   │       ├── compressed_len (u16 LE)
│   │       ├── flags (u8)
│   │       └── zlib compressed nibbles
│   │           └── packed: 2 pixels per byte (high=even, low=odd)
│   └── RGBA mode:
│       ├── paper_color (3 bytes RGB)
│       └── tiles: raw RGBA per tile, zlib compressed
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
│ Quantize pixels │  ← alpha-aware, brightness-based
│ to 4 indices    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Apply age_step  │  ← one aging cycle during save
│ (erosion)       │
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
│ Pack nibbles    │  ← 2 indices per byte (high=even)
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

FMRL aging uses morphological erosion with two phases: **erosion** and **short-run elimination**.

### Phase 1: Morphological Erosion

A non-paper pixel becomes paper if it has **≥3 paper 8-neighbors**.

```
// 8-neighborhood (N = north, S = south, etc.)
    N
  W C E    // C = center pixel being evaluated
    S
   NW NE
   SW SE

if index[C] != 1:  // not already paper
    paper_neighbors = count(index[N,S,E,W,NW,NE,SW,SE] == 1)
    if paper_neighbors >= 3:
        index[C] = 1  // erode to paper
```

**Why ≥3?** Any finite solid shape has corner pixels with exactly 3 paper neighbors. This guarantees convergence to all-paper with enough iterations.

### Phase 2: Short-Run Elimination

After erosion, eliminate thin isolated features for better compression:

1. **Horizontal pass**: Find runs of non-paper pixels ≤2 wide, replace with paper
2. **Vertical pass**: Same for columns

```
Before:  ink ink paper ink ink paper ink    (isolated pairs)
After:   paper paper paper paper paper paper paper
```

This collapses noise-like features and increases zlib compression ratios.

### Convergence Guarantee

The algorithm converges to all-paper because:
- Every solid pixel on a convex corner has exactly 3 paper neighbors
- These corners erode inward, shrinking any finite shape
- Thin features (≤2 pixels wide) are eliminated by short-run removal
- No finite fixed point exists other than all-paper

---

## Decoding Pipeline

```
.fmrl file
     │
     ▼
┌─────────────────┐
│ Parse chunks    │  ← verify CRC-32 on each
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Decompress tiles│
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Unpack nibbles  │  ← expand to indices
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Render with     │  ← theme colors applied here
│ theme palette   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Update AGE      │  ← mutate last_view timestamp
│ chunk in-place  │
└────────┬────────┘
         │
         ▼
   RGBA output
```

---

## Decay Model

Each view applies temporal decay based on `last_view` timestamp:

1. **Calculate age**: `age_ms = now_ms - last_view`
2. **Fade factor**: `fade = min(1.0, age_ms / (30 days in ms))`
3. **Apply to render**:
   - Indices fade toward paper color based on fade factor
   - Edge damage accumulates stochastically
   - Deterministic PRNG per tile (xoshiro128++)

---

## Byte Layout Reference

### Chunk Structure (PNG-compatible)

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

### Rust (Core Codec)
- `quantize_pixel()` in `src/encode.rs` — alpha + brightness logic
- `age_step()` in `src/age.rs` — morphological erosion
- `Palette::default()` in `src/format.rs` — storage palette

### JavaScript (Web App)
- `STORAGE_PALETTE` — mirrors Rust palette for debug PNG export
- `indicesToGrayscaleRgba()` — converts indices to grayscale RGBA

### WASM Surface
- `encode_rgba()` — quantizes and ages on save
- `decode_to_indices()` — loads without decay for editing
- `decode_and_decay()` — renders with decay for display

---

## Design Principles

1. **Theme independence**: Storage uses grayscale, rendering applies theme
2. **Information loss**: Aging removes pixels permanently; compression improves as data degrades
3. **Determinism**: Same file + same state = same render everywhere
4. **Self-contained**: All decay state lives in AGE chunk
5. **Convergence**: Guaranteed to reach all-paper with repeated aging
