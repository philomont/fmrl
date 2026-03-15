# FMRL Web App Context

## Overview

The FMRL web demo is a browser-based display tool for the FMRL (Fragile Manuscript Record Layer) ephemeral image codec. It demonstrates the aging algorithms and provides a canvas-based drawing interface.

## File Structure

```
docs/
├── index.html          # Main HTML structure
├── index.js            # Application logic (~1300 lines)
├── style.css           # Theme-aware styling (~570 lines)
├── themes.json         # Theme palette definitions
├── test-accent-color.js # Debug utility for theme colors
└── pkg/                # WASM build output (generated)
    ├── fmrl.js         # WASM bindings
    ├── fmrl_bg.wasm    # Compiled Rust
    └── ...
```

## Architecture

### State Management

The app uses module-level state variables (not a framework):

```javascript
// Core state
let indices = null;           // Current canvas pixel indices (0-15)
let colorIdx = 1;             // Current drawing color (1=ink, 8=accent, etc.)
let brushSize = 2;            // Current brush size (2, 6, or 14)
let drawing = false;          // Mouse/touch is down

// Aging state
let currentAgeType = 0;       // 0=erosion, 1=consolidation, 2=bleach
let currentAgeLevels = null;  // Per-tile consolidation levels
let currentPixelAges = null;  // Per-pixel ages (for consolidation mode)

// Canvas dimensions (multiples of TILE_SIZE=128)
let W = 0, H = 0;
const TILE_SIZE = 128;
```

### Color System

**Palette indices (v0.4+ format):**
- Index 0 = paper (eraser/background)
- Index 1 = ink (primary drawing color)
- Index 8 = accent (orange by default)
- Index 15 = highlight (light accent)
- Indices 2-7, 9-14 = unused (map to paper)

**Theme system:**
- Themes defined in `themes.json` (paper, ink, accent, highlight)
- Custom palette support via color pickers (saved to localStorage)
- CSS custom properties for real-time theme switching

### WASM Integration

The app imports from `./pkg/fmrl.js`:

```javascript
import init, {
    FmrlView,                    // Main decoder interface
    encode_rgba,                 // Encode with default settings
    encode_rgba_with_age,        // Encode with age type
    encode_rgba_with_age_and_levels,  // Encode with existing age levels
    encode_rgba_with_pixel_ages, // Encode with per-pixel ages
    decode_to_indices,           // Decode to palette indices
    consolidation_step_with_ages, // Direct consolidation access
    bleach_step_indices          // Direct bleach access
} from './pkg/fmrl.js';
```

### Key Functions

#### Rendering
- `render()` - Blits `indices` to canvas using current theme palette
- `indicesToRgba(src)` - Converts indices to RGBA for encoding

#### Drawing
- `paintAt(cx, cy)` - Draws single point with current brush
- `paintLine(x0, y0, x1, y1)` - Bresenham line drawing
- `setTextMode(on)` - Toggles text input mode
- `placeTextCursor(cx, cy)` - Positions text cursor

#### Aging
- `applyAge(n=1)` - Applies n aging steps via encode/decode cycle
- `_doAgeStep(src, full)` - Internal: applies one step using WASM
- `updateTileAgeLevelsFromPixelAges()` - Syncs tile ages from pixel ages

#### File I/O
- `saveFmrl()` - Encodes and downloads .fmrl file
- `loadFmrl(arrayBuffer)` - Decodes and loads file
- `saveDebugPng()` - Exports PNG alongside FMRL (debug mode)

### Event Flow

1. **Drawing**: mousedown → paintAt → render
2. **Aging**: click Age → encode → decode → update indices → render
3. **Loading**: file input → loadFmrl → decode_to_indices → render
4. **Theming**: select change → setTheme → updateSwatchColors → render

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Space` | Age one step |
| `a` | Toggle auto-aging |
| `f` | Cycle through aging algorithms |
| `t` | Cycle through themes |
| `↑` (Up) | Increase aging speed (slower interval) |
| `↓` (Down) | Decrease aging speed (faster interval) |
| `Delete` / `Backspace` | Clear canvas |
| `Esc` | Exit text mode |

**Note**: Hotkeys are disabled when in text mode or when any input/textarea is focused.

### UI Components

**Toolbar (left):**
- 4 color swatches (indices 1, 8, 15, 0)
- 3 brush size buttons (2, 6, 14)
- Text tool button
- Color editor button

**Action bar (bottom):**
- Age / Age ×10 buttons
- Auto-aging toggle + rate controls
- Aging technique selector (erosion/consolidation/bleach)
- File size metric
- Clear / Save / Load buttons
- Theme selector
- Info tray toggle

**Tray (right, collapsible):**
- About FMRL description
- Aging technique explanations
- Debug mode toggle

## Extension Points

### Adding a new aging technique:

1. Add to `age-type-select` in index.html
2. Update `currentAgeType` handling in `applyAge()`
3. Add WASM export if direct access needed
4. Update tray documentation

### Adding a new theme:

1. Add entry to `themes.json`
2. Option automatically appears in theme selector
3. Colors: paper, ink, accent, highlight

### Adding a new tool:

1. Add button to mini-toolbar in index.html
2. Add state variable(s)
3. Implement event handlers
4. Update `rememberToolState()` / `restoreToolState()`

## Build Process

The web app is static files - no build step required for JS/CSS.
WASM must be built from Rust:

```bash
# Using just (recommended)
just deploy-all    # Build WASM, sync themes, copy to docs, serve on :8080

# Or manually
wasm-pack build --target web --features wasm
cp -r pkg docs/
python3 -m http.server 8080 --directory docs/
```

Server runs on **port 8080**. Use `just halt` to stop the server.

## Browser Compatibility

- Modern browsers with ES6 module support
- WebAssembly support required
- File System Access API used for save (with fallback)
- localStorage for theme persistence

## Naming Conventions

### Functions
- `camelCase` for all JavaScript functions
- Event handlers: `on*`, `handle*` (e.g., `onMouseDown`, `handleSave`)
- State getters: `get*`, `is*` (e.g., `getThemePalette`, `isRgba`)

### Variables
- Module state: `camelCase` (e.g., `currentAgeType`, `brushSize`)
- Constants: `SCREAMING_SNAKE_CASE` (e.g., `TILE_SIZE`, `PALETTE`)
- DOM elements: cached in `init()` with descriptive names

### CSS Classes
- Component: `kebab-case` (e.g., `mini-toolbar`, `bar-btn`)
- State: `is-*` or `active` (e.g., `active`, `bar-accent`)

## Common Tasks

### Debugging:
- Enable debug mode in tray to save PNG alongside FMRL
- Check browser console for WASM errors
- Use `test-accent-color.js` for theme debugging

### Testing changes:
- Hard refresh (Ctrl+Shift+R) to clear module cache
- Increment `?v=N` in import URL if WASM changes
- Check localStorage for persisted state issues
