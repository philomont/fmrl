# FMRL Web Application

Web-based reference implementation of the FMRL codec using WebAssembly.

---

## Overview

The web app provides an interactive canvas for creating, editing, and viewing FMRL images. It demonstrates the codec's aging behavior in real-time.

**Note:** This document describes the web app implementation. For the core FMRL codec specification, see `../ALGORITHM.md`.

---

## Architecture

```
┌─────────────────┐
│   HTML5 Canvas  │  ← User interaction (drawing, text)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   JavaScript    │  ← UI state, theme system, file I/O
│   (index.js)    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   WASM (Rust)   │  ← FMRL codec via wasm-bindgen
│   (pkg/fmrl.js) │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   FMRL Codec    │  ← encode/decode/age
│   (Rust core)   │
└─────────────────┘
```

---

## Canvas System

### Layout
- Canvas fills the full viewport
- Dimensions sized to next multiple of 32, capped at 1024px on larger dimension
- CSS scales to fill screen — intentionally pixelated at high DPI

### Coordinate System
- All drawing uses canvas pixel coordinates (not CSS pixels)
- 1:1 mapping between canvas pixels and FMRL indices

---

## Theme System

### Storage vs. Display

- **Storage palette** (in codec): 16 grayscale values (indices 0-15)
  - Index 0 = paper (white, transparent via alpha=0)
  - Index 1 = ink (black)
  - Indices 2-15 = grayscale steps
- **Display palette** (in `themes.json`): CSS custom properties map indices to theme colors

### Default Web App Mapping

| Storage Index | Grayscale | Web App Role | Default Theme |
|---------------|-----------|--------------|---------------|
| 0 | White (255) | Paper/eraser | `--paper` (transparent) |
| 1 | Black (0) | Ink | `--ink` |
| 2 | Dark gray (17) | Dark accent | `--accent-dark` |
| 8 | Mid gray (136) | Accent | `--accent` |
| 15 | Light gray (255) | Highlight | `--highlight` |

**Note:** The web app can map any subset of the 16 indices to theme colors. The codec stores all 16; the app decides which to expose.

### Storage Palette Reference

```javascript
// Mirrors Rust PALETTE_SIZE and default palette
const PALETTE_SIZE = 16;
const STORAGE_PALETTE = [
    [255, 255, 255],   // 0: paper - white (transparent via alpha)
    [0, 0, 0],         // 1: ink - black
    [17, 17, 17],      // 2: dark gray
    [34, 34, 34],      // 3
    [51, 51, 51],      // 4
    [68, 68, 68],      // 5
    [85, 85, 85],      // 6
    [102, 102, 102],   // 7
    [119, 119, 119],   // 8
    [136, 136, 136],   // 9
    [153, 153, 153],   // 10
    [170, 170, 170],   // 11
    [187, 187, 187],   // 12
    [204, 204, 204],   // 13
    [221, 221, 221],   // 14
    [238, 238, 238],   // 15: lightest non-paper
];
```

### Theme JSON Structure
```json
{
  "name": "Theme Name",
  "colors": {
    "paper": "#faf8f0",
    "ink": "#2a2a2a",
    "accent": "#c45c26",
    "highlight": "#d4a373"
  }
}
```

---

## Drawing Tools

### Brush System
Three fixed brush sizes:
| Size | Radius | Font Size |
|------|--------|-----------|
| Fine | 2px    | 40px      |
| Medium | 6px  | 80px      |
| Thick | 14px   | 120px     |

### Text Tool
- Places baseline cursor on click
- Renders in current palette color using National Park font
- Font size tracks active brush
- Enter advances line, Escape cancels

### Color Swatches
- 5 palette colors mapped to indices: 0 (paper/eraser), 1 (ink), 2 (dark), 8 (accent), 15 (highlight)
- Index 0 (paper) acts as eraser
- The web app can expose any subset of the 16 available indices

---

## Aging UI

### Controls
- **Age**: Apply one full aging step (erosion + short-run elimination)
- **Age ×10**: Apply 10 steps at once
- **Auto**: Toggle passive aging with adjustable rate
- **Rate**: Six intervals (50ms, 100ms, 200ms, 500ms, 1s, 2s)

### Two-Tier Aging
- **Full** (`full=true`): Erosion + short-run elimination — used for Age buttons and save
- **Light** (`full=false`): Erosion only — used for auto-aging for fluid animation

---

## File Operations

### Save
1. Canvas → RGBA pixels
2. WASM `encode_rgba()` → quantize + age + compress
3. Download `.fmrl` file
4. Optional: Debug mode also downloads PNG

### Load
1. Read `.fmrl` file
2. WASM `decode_to_indices()` → palette indices
3. Apply one aging step (simulates time passed)
4. Render to canvas

---

## Debug Mode

When enabled in the About tray:
- Save downloads both `.fmrl` and `.png`
- PNG uses grayscale storage palette for inspection
- Useful for verifying quantization and aging

### indicesToGrayscaleRgba()
Converts palette indices to RGBA using storage palette:
```javascript
function indicesToGrayscaleRgba(indices, width, height) {
    // Returns RGBA buffer using STORAGE_PALETTE colors
}
```

---

## Key Implementation Details

### Text Mode State
- `textBaseIndices`: Snapshot of canvas before text entry
- Blink timer restores from this snapshot
- Auto-aging syncs snapshot with aged canvas

### Passive Aging
- Timer applies light aging steps
- Updates file size metric after each step
- Pauses during text entry to prevent conflict

### Canvas Resizing
- Rounds to multiple of 32 before encode
- Prevents WASM panic from invalid dimensions

---

## Dependencies

- **WASM module**: Built from Rust core via `wasm-pack`
- **Font**: National Park variable-weight woff2
- **No external JS frameworks**: Vanilla JS for portability

---

## Mobile Considerations

- Toolbar stacks vertically on narrow screens (≤540px)
- Touch events supported alongside mouse
- Action bar scrolls horizontally when needed
- Info tray slides up from bottom on mobile

---

## See Also

- `../ALGORITHM.md` — Core FMRL codec specification
- `../README.md` — Project overview
