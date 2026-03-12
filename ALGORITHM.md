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
│ Apply aging     │  ← based on age_type:
│ (if configured) │    • Erosion: morphological + short-run
│                 │    • Consolidation: block merging
│                 │    • Bleach: convolutional cleaning
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
│ zlib compress   │  ← best compression (level 9)
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

## Aging Algorithms

Three distinct aging mechanisms are available via the `age_type` field in IHDR (byte 10):

| Type | Value | Name | Description |
|------|-------|------|-------------|
| 0 | 0x00 | Erosion | Morphological erosion + short-run elimination |
| 1 | 0x01 | Consolidation | Progressive block merging (2×2 → 4×4 → 8×8 → 16×16) |
| 2 | 0x02 | Bleach | Convolutional pattern cleaning (sliding 2×2 windows) |

---

## Algorithm 1: Erosion

Two-pass information-reducing filter.

### Phase 1: Morphological Erosion

A non-paper pixel (index > 0) becomes paper if it has **≥ 4 paper 8-neighbors** (out-of-bounds treated as paper):

```
for each pixel (x,y):
    if index[y][x] > 0:
        paper_neighbors = count_8_neighbors_where(index == 0)
        if paper_neighbors >= 4:
            index[y][x] = 0  // erode to paper
```

### Phase 2: Short-Run Elimination

Scan rows then columns. Any non-paper run of length ≤ `RUN_THRESHOLD` (2) becomes paper:

```
// Horizontal pass (rows)
for each row y:
    find runs of index > 0
    if run_length <= 2:
        set run to paper

// Vertical pass (columns)
for each column x:
    find runs of index > 0
    if run_length <= 2:
        set run to paper
```

### Convergence

Guaranteed to reach all-paper because:
- Paper (index 0) is the only fixed point
- All operations only convert non-paper → paper
- No operation creates non-paper pixels

---

## Algorithm 2: Consolidation

Hierarchical block merging with per-pixel age tracking.

### Per-Pixel Ages

Each pixel has an independent age (0-4):
- Age 0: Just drawn, participates in 2×2 consolidation
- Age 1: Participates in 4×4 consolidation
- Age 2: Participates in 8×8 consolidation
- Age 3: Participates in 16×16 consolidation
- Age 4+: Becomes paper

### Consolidation Rules

A block consolidates when **ALL pixels have age ≤ threshold AND at least one pixel has age == threshold**:

```
2×2 blocks:  min_age == 0  → set all to age 1
4×4 blocks:  min_age == 1  → set all to age 2
8×8 blocks:  min_age == 2  → set all to age 3
16×16 blocks: min_age == 3 → set all to age 4 (paper)
```

### Block Value Assignment

Consolidated blocks take the **minimum non-zero index** of their constituent pixels (0 if all paper):

```
new_index = min(filter(index > 0)) or 0
```

### Key Property

Mixed-age blocks consolidate at the level of the **youngest** pixels, allowing new drawings to age alongside existing content.

---

## Algorithm 3: Bleach

Convolutional pattern cleaning using sliding 2×2 windows.

### Sliding Window

Unlike fixed tiles, bleach uses overlapping windows covering every pixel position:

```
for y in 0..height-1:
    for x in 0..width-1:
        block = [index[y][x],     index[y][x+1],
                 index[y+1][x],   index[y+1][x+1]]
        if is_bleachable(block):
            mark all 4 pixels for bleaching

// Apply bleaching (all marked pixels → paper)
```

### Bleachable Patterns

A 2×2 block becomes paper if:

| Condition | Example | Reason |
|-----------|---------|--------|
| 3+ unique indices | `[[0,1],[2,0]]` | Information-rich/noisy |
| Imbalanced 3:1 | `[[1,1],[1,2]]` | Uneven distribution |
| Anti-diagonal | `[[1,2],[2,1]]` | `a,b` / `b,a` pattern |

```
fn is_bleachable(block[4]) -> bool:
    counts = histogram(block)  // count of each index 0-15
    unique = count_nonzero(counts)

    if unique >= 3:
        return true  // too noisy

    if unique == 2:
        c1, c2 = the two counts
        if c1 == 3 or c2 == 3:
            return true  // 3:1 imbalanced
        if c1 == 2 and c2 == 2:
            // Check anti-diagonal: [[a,b],[b,a]]
            if block[0] == block[3] and block[1] == block[2]:
                return true

    return false  // uniform or acceptable
```

### Convergence

Like erosion, bleach only converts to paper, guaranteeing convergence to all-paper.

---

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
| `age_step()` | `src/age.rs` | Erosion aging |
| `consolidation_step_with_pixel_ages()` | `src/age.rs` | Consolidation aging |
| `bleach_step()` | `src/age.rs` | Bleach aging |
| `Palette::default()` | `src/format.rs` | 16-entry grayscale palette |
| `PALETTE_SIZE` | `src/format.rs` | Constant = 16 |

### WASM Surface

- `encode_rgba()` — encode with indexed mode
- `encode_rgba_full()` — encode with RGBA mode
- `encode_rgba_with_age()` — encode with specified age type
- `decode_to_indices()` — decode to palette indices
- `decode_to_rgba()` — decode to RGBA pixels
- `consolidation_step_indices()` — apply consolidation step
- `bleach_step_indices()` — apply bleach step

---

## Design Principles

1. **Determinism**: Same file + same state = same output everywhere
2. **Information loss**: Aging removes pixels permanently
3. **Self-contained**: All state in AGE chunk
4. **Extensible**: 16-color palette supports future theme mapping
