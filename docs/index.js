import init, { FmrlView, encode_rgba, decode_to_indices } from './pkg/fmrl.js';

// ── Canvas dimensions ───────────────────────────────────────────────────────

let W = 0;
let H = 0;

function computeCanvasDims(srcW, srcH) {
    const MAX   = 1024;
    const scale = Math.min(1, MAX / Math.max(srcW, srcH));
    return [
        Math.min(Math.ceil(srcW * scale / 32) * 32, MAX),
        Math.min(Math.ceil(srcH * scale / 32) * 32, MAX),
    ];
}

// Default palette: ink, paper, accent, highlight
// Matches themes.default in fmrl.toml
const PALETTE = [
    [34, 34, 34],       // 0: ink
    [250, 243, 225],   // 1: paper
    [255, 109, 31],    // 2: accent (orange)
    [245, 231, 198],   // 3: highlight
];

// Alpha-based palette for file storage:
// The FMRL file stores alpha values that map to theme colors:
// 0 = ink (full black, 255 alpha) -> renders as theme --ink
// 1 = paper (transparent, 0 alpha) -> renders as theme --paper
// 2 = accent (black 50%, 128 alpha) -> renders as theme --accent
// 3 = highlight (white 50%, 128 alpha) -> renders as theme --highlight
const STORAGE_TO_THEME = [
    'ink',      // 0: full black
    'paper',    // 1: transparent
    'accent',   // 2: black 50%
    'highlight' // 3: white 50%
];

// Theme palette definitions loaded from themes.json (synced from fmrl.toml)
let THEME_PALETTES = {};
let customPalette = null; // For user-defined colors

// Debug mode - enables PNG export alongside FMRL for inspection
let debugMode = false;

// Load themes from JSON file
async function loadThemes() {
    try {
        const response = await fetch('themes.json');
        if (!response.ok) throw new Error('Failed to load themes');
        const themes = await response.json();

        // Convert object format to array format [ink, paper, accent, highlight]
        for (const [name, data] of Object.entries(themes)) {
            THEME_PALETTES[name] = [
                data.ink,
                data.paper,
                data.accent,
                data.highlight,
            ];
        }
        console.log('Loaded themes:', Object.keys(THEME_PALETTES));
    } catch (e) {
        console.warn('Failed to load themes.json, using defaults:', e);
        // Fallback to default palette
        THEME_PALETTES = { default: PALETTE };
    }
}

function getThemePalette() {
    // Return custom palette if set
    if (customPalette) return customPalette;

    // Get current theme name
    const currentTheme = document.documentElement.getAttribute('data-theme') || 'default';

    // Return palette from theme definition (avoids CSS timing issues)
    if (THEME_PALETTES[currentTheme]) {
        return THEME_PALETTES[currentTheme];
    }

    // Fallback to reading from CSS (shouldn't happen for presets)
    const root = getComputedStyle(document.documentElement);
    const ink = cssColorToRgb(root.getPropertyValue('--ink').trim());
    const paper = cssColorToRgb(root.getPropertyValue('--paper').trim());
    const accent = cssColorToRgb(root.getPropertyValue('--accent').trim());
    const highlight = cssColorToRgb(root.getPropertyValue('--highlight').trim());
    return [ink, paper, accent, highlight];
}

function cssColorToRgb(color) {
    if (!color) return [0, 0, 0];
    // Handle hex colors
    if (color.startsWith('#')) {
        const hex = color.slice(1);
        if (hex.length === 3) {
            return [
                parseInt(hex[0] + hex[0], 16),
                parseInt(hex[1] + hex[1], 16),
                parseInt(hex[2] + hex[2], 16)
            ];
        }
        return [
            parseInt(hex.slice(0, 2), 16),
            parseInt(hex.slice(2, 4), 16),
            parseInt(hex.slice(4, 6), 16)
        ];
    }
    // Handle rgb(r, g, b) format
    const match = color.match(/rgb\((\d+),\s*(\d+),\s*(\d+)\)/);
    if (match) {
        return [parseInt(match[1]), parseInt(match[2]), parseInt(match[3])];
    }
    // Fallback to default palette
    return PALETTE;
}

function rgbToHex(r, g, b) {
    return '#' + [r, g, b].map(x => x.toString(16).padStart(2, '0')).join('');
}

function updateSwatchColors() {
    const palette = getThemePalette();

    // palette: [ink, paper, accent, highlight]
    const [ink, paper, accent, highlight] = palette;
    const inkRgb = `rgb(${ink[0]}, ${ink[1]}, ${ink[2]})`;
    const paperRgb = `rgb(${paper[0]}, ${paper[1]}, ${paper[2]})`;
    const accentRgb = `rgb(${accent[0]}, ${accent[1]}, ${accent[2]})`;
    const highlightRgb = `rgb(${highlight[0]}, ${highlight[1]}, ${highlight[2]})`;

    // Set colors directly on swatch elements (inline styles override CSS)
    const swatches = document.querySelectorAll('.swatch');
    swatches.forEach(swatch => {
        const idx = parseInt(swatch.dataset.idx, 10);
        if (idx === 0) swatch.style.backgroundColor = inkRgb;
        else if (idx === 1) swatch.style.backgroundColor = paperRgb;
        else if (idx === 2) swatch.style.backgroundColor = accentRgb;
        else if (idx === 3) swatch.style.backgroundColor = highlightRgb;
    });

    // Update color picker values if they exist
    const pickers = document.querySelectorAll('.color-picker');
    pickers.forEach((picker) => {
        const idx = parseInt(picker.dataset.idx, 10);
        if (palette[idx]) {
            picker.value = rgbToHex(palette[idx][0], palette[idx][1], palette[idx][2]);
        }
    });

    // Update accent colors for UI (Age buttons, etc.)
    updateAccentColors(accent);

    // Force a re-render of the canvas with new palette
    render();
}

function updateAccentColors(accentColor) {
    const root = document.documentElement;
    const [r, g, b] = accentColor;
    const hex = rgbToHex(r, g, b);

    // Set accent to accent color with !important to override theme CSS
    root.style.setProperty('--accent', hex, 'important');

    // Calculate a lighter version for accent-hi (mix with white)
    const lighten = (val, amount) => Math.min(255, Math.round(val + (255 - val) * amount));
    const hiR = lighten(r, 0.3);
    const hiG = lighten(g, 0.3);
    const hiB = lighten(b, 0.3);
    root.style.setProperty('--accent-hi', rgbToHex(hiR, hiG, hiB), 'important');
}

function setTheme(themeName) {
    const customOption = document.querySelector('#theme-select option[value="custom"]');

    if (themeName === 'custom') {
        // Load custom colors from localStorage or use current theme as starting point
        const saved = localStorage.getItem('fmrl-custom-palette');
        if (saved) {
            customPalette = JSON.parse(saved);
        } else {
            // Start from current theme colors, not default
            customPalette = getThemePalette().map(c => [...c]);
        }
        document.documentElement.removeAttribute('data-theme');
        // Ensure Custom option is visible
        if (customOption) {
            customOption.disabled = false;
            customOption.hidden = false;
        }
    } else {
        // Clear custom palette when selecting a preset
        customPalette = null;
        localStorage.removeItem('fmrl-custom-palette');
        document.documentElement.setAttribute('data-theme', themeName);
        // Hide Custom option when using presets
        if (customOption) {
            customOption.disabled = true;
            customOption.hidden = true;
        }
    }
    localStorage.setItem('fmrl-theme', themeName);
    // Reset blank size on theme change since palette changed
    blankSize = 0;
    lastMetricSize = 0;
    updateSwatchColors();
    render();
    updateMetric();
}

function setCustomColor(idx, hexColor) {
    if (!customPalette) {
        // Deep copy to avoid reference issues
        customPalette = getThemePalette().map(c => [...c]);
    }
    customPalette[idx] = cssColorToRgb(hexColor);
    localStorage.setItem('fmrl-custom-palette', JSON.stringify(customPalette));
    // Switch to custom theme to ensure palette is used
    document.documentElement.removeAttribute('data-theme');
    localStorage.setItem('fmrl-theme', 'custom');
    // Enable and select the Custom option
    const customOption = document.querySelector('#theme-select option[value="custom"]');
    if (customOption) {
        customOption.disabled = false;
        customOption.hidden = false;
    }
    document.getElementById('theme-select').value = 'custom';
    // Reset blank size on theme change since palette changed
    blankSize = 0;
    lastMetricSize = 0;
    updateSwatchColors();
    render();
    updateMetric();
}

function initTheme() {
    const saved = localStorage.getItem('fmrl-theme') || 'default';
    const customOption = document.querySelector('#theme-select option[value="custom"]');

    // If there's a saved custom palette, enable the Custom option
    const hasCustomPalette = localStorage.getItem('fmrl-custom-palette');
    if (customOption) {
        if (hasCustomPalette) {
            customOption.disabled = false;
            customOption.hidden = false;
        } else if (saved !== 'custom') {
            // Hide Custom if no saved palette and not currently custom
            customOption.disabled = true;
            customOption.hidden = true;
        }
    }

    document.getElementById('theme-select').value = saved;
    setTheme(saved);
}

// ── Drawing state ───────────────────────────────────────────────────────────

let indices   = null;
let colorIdx  = 0;
let brushSize = 2;   // matches first brush-btn data-size
let drawing   = false;
let lastX     = -1;
let lastY     = -1;

// Tool state memory for color editor
let lastToolBeforeColorEdit = null;

function rememberToolState() {
    // Remember which tool was active
    if (textMode) {
        lastToolBeforeColorEdit = 'text';
    } else {
        const activeBrush = document.querySelector('.brush-btn[data-size].active');
        if (activeBrush) {
            lastToolBeforeColorEdit = activeBrush.dataset.size;
        }
    }
}

function restoreToolState() {
    if (!lastToolBeforeColorEdit) {
        // Default to fine brush if nothing was remembered
        document.querySelectorAll('.brush-btn').forEach(b => b.classList.remove('active'));
        const fine = document.querySelector('.brush-btn[data-size="2"]');
        if (fine) fine.classList.add('active');
        brushSize = 2;
        return;
    }

    document.querySelectorAll('.brush-btn').forEach(b => b.classList.remove('active'));

    if (lastToolBeforeColorEdit === 'text') {
        setTextMode(true);
        document.getElementById('tool-text').classList.add('active');
    } else {
        const brushBtn = document.querySelector(`.brush-btn[data-size="${lastToolBeforeColorEdit}"]`);
        if (brushBtn) {
            brushBtn.classList.add('active');
            brushSize = parseInt(lastToolBeforeColorEdit, 10);
        } else {
            // Fallback to fine brush
            const fine = document.querySelector('.brush-btn[data-size="2"]');
            if (fine) fine.classList.add('active');
            brushSize = 2;
        }
    }
}

// ── Rendering ───────────────────────────────────────────────────────────────

let canvas;

function render() {
    const ctx     = canvas.getContext('2d');
    const imgData = ctx.createImageData(W, H);
    const palette = getThemePalette();
    for (let i = 0; i < W * H; i++) {
        const [r, g, b] = palette[indices[i]];
        imgData.data[i * 4]     = r;
        imgData.data[i * 4 + 1] = g;
        imgData.data[i * 4 + 2] = b;
        imgData.data[i * 4 + 3] = 255;
    }
    ctx.putImageData(imgData, 0, 0);
}

// ── Drawing ─────────────────────────────────────────────────────────────────

function canvasCoords(e) {
    const r  = canvas.getBoundingClientRect();
    const cx = (e.clientX ?? e.touches[0].clientX) - r.left;
    const cy = (e.clientY ?? e.touches[0].clientY) - r.top;
    return [Math.floor(cx * W / r.width), Math.floor(cy * H / r.height)];
}

function paintAt(cx, cy) {
    const r = brushSize;
    for (let dy = -r; dy <= r; dy++) {
        for (let dx = -r; dx <= r; dx++) {
            if (dx * dx + dy * dy <= r * r + 0.5) {
                const px = cx + dx, py = cy + dy;
                if (px >= 0 && px < W && py >= 0 && py < H)
                    indices[py * W + px] = colorIdx;
            }
        }
    }
}

function paintLine(x0, y0, x1, y1) {
    const steps = Math.max(Math.abs(x1 - x0), Math.abs(y1 - y0), 1);
    for (let s = 0; s <= steps; s++) {
        const t = s / steps;
        paintAt(Math.round(x0 + (x1 - x0) * t), Math.round(y0 + (y1 - y0) * t));
    }
}

// ── Aging ───────────────────────────────────────────────────────────────────
//
// _doAgeStep(src, full):
//   full=true  — morphological erosion + short-run elimination (manual Age,
//                Age ×10, save): maximum data removal per step.
//   full=false — morphological erosion only (auto aging): finer-grained steps
//                so high-rate passive aging feels fluid.

const RUN_THRESHOLD = 2;

function _doAgeStep(src, full = true) {
    const next = src.slice();
    const w = W, h = H;

    for (let y = 0; y < h; y++) {
        for (let x = 0; x < w; x++) {
            if (src[y * w + x] === 1) continue;
            let paperCount = 0;
            for (let dy = -1; dy <= 1; dy++) {
                for (let dx = -1; dx <= 1; dx++) {
                    if (dx === 0 && dy === 0) continue;
                    const nx = x + dx, ny = y + dy;
                    if (nx < 0 || nx >= w || ny < 0 || ny >= h ||
                        src[ny * w + nx] === 1) paperCount++;
                }
            }
            if (paperCount >= 3) next[y * w + x] = 1;
        }
    }

    if (!full) return next;

    for (let y = 0; y < h; y++) {
        for (let x = 0; x < w; ) {
            if (next[y * w + x] !== 1) {
                let e = x + 1;
                while (e < w && next[y * w + e] !== 1) e++;
                if (e - x <= RUN_THRESHOLD)
                    for (let rx = x; rx < e; rx++) next[y * w + rx] = 1;
                x = e;
            } else { x++; }
        }
    }

    for (let x = 0; x < w; x++) {
        for (let y = 0; y < h; ) {
            if (next[y * w + x] !== 1) {
                let e = y + 1;
                while (e < h && next[e * w + x] !== 1) e++;
                if (e - y <= RUN_THRESHOLD)
                    for (let ry = y; ry < e; ry++) next[ry * w + x] = 1;
                y = e;
            } else { y++; }
        }
    }

    return next;
}

function applyAge(n = 1) {
    for (let i = 0; i < n; i++) indices = _doAgeStep(indices, true);
    render();
    updateMetric();
}

// ── Compression metric ──────────────────────────────────────────────────────

function indicesToRgba(src = indices) {
    const rgba = new Uint8Array(W * H * 4);
    const palette = getThemePalette();
    for (let i = 0; i < W * H; i++) {
        const [r, g, b] = palette[src[i]];
        rgba[i * 4] = r; rgba[i * 4 + 1] = g;
        rgba[i * 4 + 2] = b; rgba[i * 4 + 3] = 255;
    }
    return rgba;
}

// ── Size tracking ───────────────────────────────────────────────────────────

let blankSize = 0;     // Size of all-paper canvas
let lastMetricSize = 0; // For tracking change between updates

function formatBytes(bytes) {
    if (bytes >= 1048576) {
        return (bytes / 1048576).toFixed(2) + ' MB';
    } else if (bytes >= 1024) {
        return (bytes / 1024).toFixed(2) + ' kB';
    } else {
        return bytes + ' B';
    }
}

function computeBlankSize() {
    // Create all-paper indices (paper is always index 1)
    const paperIndices = new Uint8Array(W * H).fill(1);
    // Use current palette for blank size calculation
    const palette = getThemePalette();
    const rgba = new Uint8Array(W * H * 4);
    for (let i = 0; i < W * H; i++) {
        const [r, g, b] = palette[1]; // paper color
        rgba[i * 4] = r;
        rgba[i * 4 + 1] = g;
        rgba[i * 4 + 2] = b;
        rgba[i * 4 + 3] = 255;
    }
    blankSize = encode_rgba(rgba, W, H).length;
}

function updateMetric() {
    const size = encode_rgba(indicesToRgba(), W, H).length;
    const el = document.getElementById('size-metric');

    // Compute blank size if not already done (first time)
    if (blankSize === 0) {
        computeBlankSize();
    }

    // Calculate drawing size (information content beyond blank)
    const drawingSize = size - blankSize;

    // Build display: {blank} + {drawing} | {change}
    let text = `${formatBytes(blankSize)} + ${formatBytes(drawingSize)}`;

    // Show change from last update (aging effect)
    if (lastMetricSize > 0) {
        const change = size - lastMetricSize;
        if (change !== 0) {
            text += ` | ${change < 0 ? '↓' : '↑'}${formatBytes(Math.abs(change))}`;
        } else {
            text += ` | --`;
        }
    } else {
        text += ` | --`;
    }

    el.textContent = text;
    lastMetricSize = size;
}

// ── Auto (passive) aging ─────────────────────────────────────────────────────

const PASSIVE_RATES_S = [0.05, 0.1, 0.2, 0.5, 1, 2];
let passiveRateIdx = 4;
let passiveTimer   = null;

function passiveIntervalMs() { return PASSIVE_RATES_S[passiveRateIdx] * 1000; }

function updateRateDisplay() {
    const s = PASSIVE_RATES_S[passiveRateIdx];
    document.getElementById('rate-display').textContent =
        s < 1 ? (s * 1000) + 'ms' : s + 's';
}

function setPassiveAging(enabled) {
    clearInterval(passiveTimer);
    passiveTimer = null;
    const btn = document.getElementById('btn-passive');
    if (enabled) {
        passiveTimer = setInterval(() => {
            // Age the base snapshot in sync so the cursor-blink restore doesn't
            // revert the canvas to an un-aged state while text is being typed.
            if (textBaseIndices) textBaseIndices = _doAgeStep(textBaseIndices, false);
            indices = _doAgeStep(indices, false);
            if (textCursor) _blitText(textBuffer + (cursorBlink ? '|' : ''));
            else render();
            updateMetric();
        }, passiveIntervalMs());
        btn.classList.add('active');
        btn.textContent = 'Auto  ON';
    } else {
        btn.classList.remove('active');
        btn.textContent = 'Auto  off';
    }
}

// ── Text tool ────────────────────────────────────────────────────────────────
//
// Font size tracks the active brush: fine → 16 px, medium → 40 px, thick → 80 px.
// Click canvas to place a baseline cursor, type to build the string.
// Enter advances to the next line; Escape cancels without committing.
// Switching tools commits any pending text.

const BRUSH_FONT = { 2: 16, 6: 40, 14: 80 };
function textFontSize() { return BRUSH_FONT[brushSize] ?? Math.round(brushSize * 3); }

let textMode        = false;
let textCursor      = null;
let textBuffer      = '';
let textBaseIndices = null;
let cursorBlink     = true;
let cursorTimer     = null;

let textHelper    = null;
let textHelperCtx = null;

function getTextCtx() {
    if (!textHelper || textHelper.width !== W || textHelper.height !== H) {
        textHelper        = document.createElement('canvas');
        textHelper.width  = W;
        textHelper.height = H;
        textHelperCtx     = textHelper.getContext('2d');
    }
    return textHelperCtx;
}

// Enter/exit text mode cleanly, committing or discarding the pending entry.
function setTextMode(on) {
    if (on === textMode) return;
    textMode = on;
    if (!on) {
        stopCursorBlink();
        if (textCursor) {
            if (textBuffer) {
                _blitText(textBuffer);      // bake glyphs without the cursor bar
            } else if (textBaseIndices) {
                indices.set(textBaseIndices); // nothing typed — clear cursor artifact
                render();
            }
        }
        textCursor = null; textBuffer = ''; textBaseIndices = null;
        canvas.style.cursor = 'crosshair';
        document.getElementById('tool-text').classList.remove('active');
    } else {
        textCursor = null; textBuffer = ''; textBaseIndices = null;
        canvas.style.cursor = 'text';
        document.getElementById('tool-text').classList.add('active');
    }
}

function placeTextCursor(cx, cy) {
    // Commit anything already typed before moving the cursor.
    if (textCursor !== null) {
        stopCursorBlink();
        if (textBuffer) {
            _blitText(textBuffer);
        } else if (textBaseIndices) {
            indices.set(textBaseIndices);
            render();
        }
    }
    textCursor      = { x: cx, y: cy };
    textBuffer      = '';
    textBaseIndices = indices.slice();
    startCursorBlink();
}

function startCursorBlink() {
    stopCursorBlink();
    cursorBlink = true;
    cursorTimer = setInterval(() => {
        cursorBlink = !cursorBlink;
        _blitText(textBuffer + (cursorBlink ? '|' : ''));
    }, 530);
}

function stopCursorBlink() {
    clearInterval(cursorTimer);
    cursorTimer = null;
}

function _blitText(text) {
    if (!textCursor) return;
    if (textBaseIndices) indices.set(textBaseIndices);
    if (!text) { render(); return; }

    const ctx = getTextCtx();
    const fs  = textFontSize();
    ctx.clearRect(0, 0, W, H);
    ctx.font      = `${fs}px "National Park", serif`;
    const palette = getThemePalette();
    const [r, g, b] = palette[colorIdx];
    ctx.fillStyle = `rgb(${r}, ${g}, ${b})`;
    ctx.fillText(text, textCursor.x, textCursor.y);

    const m       = ctx.measureText(text);
    const ascent  = (m.fontBoundingBoxAscent  ?? fs)                   + 4;
    const descent = (m.fontBoundingBoxDescent ?? Math.ceil(fs * 0.3))  + 4;

    const x0 = Math.max(0, Math.floor(textCursor.x - 2));
    const y0 = Math.max(0, Math.floor(textCursor.y - ascent));
    const x1 = Math.min(W, Math.ceil(textCursor.x + Math.max(m.width, 4) + 4));
    const y1 = Math.min(H, Math.ceil(textCursor.y + descent));

    if (x1 > x0 && y1 > y0) {
        const img = ctx.getImageData(x0, y0, x1 - x0, y1 - y0);
        const bw  = x1 - x0, bh = y1 - y0;
        for (let row = 0; row < bh; row++) {
            for (let col = 0; col < bw; col++) {
                if (img.data[(row * bw + col) * 4 + 3] > 64)
                    indices[(y0 + row) * W + (x0 + col)] = colorIdx;
            }
        }
    }
    render();
}

// ── Save / Load ─────────────────────────────────────────────────────────────

function saveFmrl() {
    try {
        // Encode current canvas state (no aging during save)
        const bytes = encode_rgba(indicesToRgba(indices), W, H);

        // Save FMRL file
        const url   = URL.createObjectURL(new Blob([bytes], { type: 'application/octet-stream' }));
        Object.assign(document.createElement('a'), { href: url, download: 'manuscript.fmrl' }).click();
        URL.revokeObjectURL(url);

        // In debug mode, also save PNG for inspection
        if (debugMode) {
            saveDebugPng(bytes, W, H);
        }
    } catch (e) { console.error('encode failed:', e); }
}

/// Save a debug PNG showing what was actually encoded
function saveDebugPng(fmrlBytes, width, height) {
    try {
        // Decode the FMRL back to see what was actually stored
        const decodedIndices = decode_to_indices(fmrlBytes);

        // Create a canvas to render the decoded image
        const debugCanvas = document.createElement('canvas');
        debugCanvas.width = width;
        debugCanvas.height = height;
        const ctx = debugCanvas.getContext('2d');
        const imgData = ctx.createImageData(width, height);

        // Use theme palette to render the decoded indices
        const palette = getThemePalette();
        for (let i = 0; i < width * height; i++) {
            const [r, g, b] = palette[decodedIndices[i]];
            imgData.data[i * 4]     = r;
            imgData.data[i * 4 + 1] = g;
            imgData.data[i * 4 + 2] = b;
            imgData.data[i * 4 + 3] = 255;
        }
        ctx.putImageData(imgData, 0, 0);

        // Convert to PNG and download
        debugCanvas.toBlob((blob) => {
            const pngUrl = URL.createObjectURL(blob);
            Object.assign(document.createElement('a'), {
                href: pngUrl,
                download: 'manuscript-debug.png'
            }).click();
            URL.revokeObjectURL(pngUrl);
        }, 'image/png');
    } catch (e) {
        console.error('Debug PNG export failed:', e);
    }
}

function loadFmrl(arrayBuffer) {
    try {
        const bytes = new Uint8Array(arrayBuffer);
        const peek  = FmrlView.new(bytes);
        const fileW = peek.width(), fileH = peek.height();
        peek.free();

        [W, H] = [fileW, fileH];
        canvas.width  = W;
        canvas.height = H;
        textHelper = null;

        // Decode without additional aging (file already contains aged data)
        indices  = new Uint8Array(decode_to_indices(bytes));
        render();
        lastMetricSize = 0;
        blankSize = 0;
        updateMetric();
    } catch (e) { alert(`Failed to load .fmrl: ${e}`); }
}

// ── Tray ────────────────────────────────────────────────────────────────────

function openTray()  {
    document.getElementById('tray').classList.add('open');
    document.getElementById('tray-backdrop').classList.add('visible');
}
function closeTray() {
    document.getElementById('tray').classList.remove('open');
    document.getElementById('tray-backdrop').classList.remove('visible');
}

// ── Main ────────────────────────────────────────────────────────────────────

async function main() {
    await init();

    // Load themes from themes.json (synced from fmrl.toml)
    await loadThemes();

    canvas = document.getElementById('canvas');
    [W, H] = computeCanvasDims(window.innerWidth, window.innerHeight);
    canvas.width  = W;
    canvas.height = H;
    indices = new Uint8Array(W * H).fill(1);

    document.fonts.load(`${textFontSize()}px "National Park"`).catch(() => {});

    document.getElementById('overlay').classList.add('hidden');
    render();
    // Initialize size tracking
    blankSize = 0;
    lastMetricSize = 0;
    updateMetric();

    // ── Canvas events ───────────────────────────────────────────────────────
    canvas.addEventListener('mousedown', e => {
        if (textMode) { placeTextCursor(...canvasCoords(e)); return; }
        drawing = true;
        [lastX, lastY] = canvasCoords(e);
        paintAt(lastX, lastY); render();
    });
    canvas.addEventListener('mousemove', e => {
        if (!drawing) return;
        const [cx, cy] = canvasCoords(e);
        paintLine(lastX, lastY, cx, cy);
        [lastX, lastY] = [cx, cy];
        render();
    });
    const stopDrawing = () => { drawing = false; };
    canvas.addEventListener('mouseup',    stopDrawing);
    canvas.addEventListener('mouseleave', stopDrawing);

    canvas.addEventListener('touchstart', e => {
        e.preventDefault();
        if (textMode) { placeTextCursor(...canvasCoords(e)); return; }
        drawing = true;
        [lastX, lastY] = canvasCoords(e);
        paintAt(lastX, lastY); render();
    }, { passive: false });
    canvas.addEventListener('touchmove', e => {
        e.preventDefault(); if (!drawing) return;
        const [cx, cy] = canvasCoords(e);
        paintLine(lastX, lastY, cx, cy);
        [lastX, lastY] = [cx, cy]; render();
    }, { passive: false });
    canvas.addEventListener('touchend', e => { e.preventDefault(); stopDrawing(); }, { passive: false });

    // ── Keyboard (text tool) ────────────────────────────────────────────────
    document.addEventListener('keydown', e => {
        if (!textMode || !textCursor) return;
        e.preventDefault();
        if (e.key === 'Escape') {
            stopCursorBlink();
            if (textBaseIndices) { indices.set(textBaseIndices); render(); }
            textCursor = null; textBuffer = ''; textBaseIndices = null;
            return;
        }
        if (e.key === 'Enter') {
            const lineH = Math.round(textFontSize() * 1.4);
            stopCursorBlink();
            if (textBuffer) _blitText(textBuffer);
            else if (textBaseIndices) { /* nothing typed, cursor stays clean */ }
            const newY = textCursor.y + lineH;
            textCursor      = { x: textCursor.x, y: newY };
            textBuffer      = '';
            textBaseIndices = indices.slice();
            startCursorBlink();
            return;
        }
        if (e.key === 'Backspace') {
            textBuffer = textBuffer.slice(0, -1);
            _blitText(textBuffer + (cursorBlink ? '|' : ''));
            return;
        }
        if (e.key.length === 1) {
            textBuffer += e.key;
            _blitText(textBuffer + (cursorBlink ? '|' : ''));
        }
    });

    // ── Palette ─────────────────────────────────────────────────────────────
    document.querySelectorAll('.swatch').forEach(btn =>
        btn.addEventListener('click', () => {
            document.querySelectorAll('.swatch').forEach(b => b.classList.remove('active'));
            btn.classList.add('active');
            colorIdx = parseInt(btn.dataset.idx, 10);
        })
    );

    // ── Brush ────────────────────────────────────────────────────────────────
    document.querySelectorAll('.brush-btn').forEach(btn =>
        btn.addEventListener('click', () => {
            if (btn.id === 'tool-text') return; // handled separately below
            setTextMode(false);
            document.querySelectorAll('.brush-btn').forEach(b => b.classList.remove('active'));
            btn.classList.add('active');
            brushSize = parseInt(btn.dataset.size, 10);
        })
    );

    // ── Text tool ────────────────────────────────────────────────────────────
    document.getElementById('tool-text').addEventListener('click', () => {
        const entering = !textMode;
        if (!entering) {
            setTextMode(false);
            // Restore the previously active brush button visual
            const prev = document.querySelector('.brush-btn[data-size]');
            if (prev && !document.querySelector('.brush-btn.active')) prev.classList.add('active');
        } else {
            document.querySelectorAll('.brush-btn').forEach(b => b.classList.remove('active'));
            setTextMode(true);
            document.getElementById('tool-text').classList.add('active');
        }
    });

    // ── Age controls ─────────────────────────────────────────────────────────
    document.getElementById('btn-age').addEventListener('click',   () => applyAge(1));
    document.getElementById('btn-age10').addEventListener('click', () => applyAge(10));
    document.getElementById('btn-passive').addEventListener('click', e =>
        setPassiveAging(!e.currentTarget.classList.contains('active')));
    document.getElementById('btn-rate-down').addEventListener('click', () => {
        if (passiveRateIdx < PASSIVE_RATES_S.length - 1) {
            passiveRateIdx++; updateRateDisplay();
            if (passiveTimer) setPassiveAging(true);
        }
    });
    document.getElementById('btn-rate-up').addEventListener('click', () => {
        if (passiveRateIdx > 0) {
            passiveRateIdx--; updateRateDisplay();
            if (passiveTimer) setPassiveAging(true);
        }
    });
    updateRateDisplay();

    document.getElementById('btn-clear').addEventListener('click', () => {
        setTextMode(false);
        indices.fill(1); render(); lastMetricSize = 0; blankSize = 0; updateMetric();
    });
    document.getElementById('btn-save').addEventListener('click', saveFmrl);
    document.getElementById('file-input').addEventListener('change', e => {
        const file = e.target.files[0]; if (!file) return;
        const reader = new FileReader();
        reader.onload = ev => loadFmrl(ev.target.result);
        reader.readAsArrayBuffer(file);
        e.target.value = '';
    });

    // ── Debug mode ───────────────────────────────────────────────────────────
    const debugCheckbox = document.getElementById('debug-mode');
    if (debugCheckbox) {
        // Load saved preference
        debugMode = localStorage.getItem('fmrl-debug-mode') === 'true';
        debugCheckbox.checked = debugMode;

        debugCheckbox.addEventListener('change', e => {
            debugMode = e.target.checked;
            localStorage.setItem('fmrl-debug-mode', debugMode);
            console.log('Debug mode:', debugMode ? 'enabled' : 'disabled');
        });
    }

    // ── Theme ───────────────────────────────────────────────────────────────
    initTheme();
    document.getElementById('theme-select').addEventListener('change', e => {
        const theme = e.target.value;
        setTheme(theme);
    });

    // ── Color Editor Tool ───────────────────────────────────────────────────
    document.getElementById('tool-colors').addEventListener('click', (e) => {
        e.stopPropagation(); // Prevent immediate close
        const panel = document.getElementById('color-picker-panel');
        const isVisible = panel.classList.contains('visible');
        if (isVisible) {
            panel.classList.remove('visible');
            document.getElementById('tool-colors').classList.remove('active');
            // Restore the previous tool if we remembered one
            restoreToolState();
            // Clear the remembered state
            lastToolBeforeColorEdit = null;
        } else {
            // Remember current tool before switching to color editor
            rememberToolState();
            // Exit text mode if open
            if (textMode) {
                setTextMode(false);
            }
            // Deselect all brush buttons
            document.querySelectorAll('.brush-btn').forEach(b => b.classList.remove('active'));
            // Sync picker values
            updateSwatchColors();
            panel.classList.add('visible');
            document.getElementById('tool-colors').classList.add('active');
        }
    });

    // Close color picker panel when clicking outside
    document.addEventListener('click', e => {
        const panel = document.getElementById('color-picker-panel');
        const colorBtn = document.getElementById('tool-colors');
        if (panel.classList.contains('visible') &&
            !panel.contains(e.target) &&
            !colorBtn.contains(e.target)) {
            panel.classList.remove('visible');
            colorBtn.classList.remove('active');
            // Restore the previous tool
            restoreToolState();
            // Clear the remembered state
            lastToolBeforeColorEdit = null;
        }
    });

    // Prevent panel clicks from closing the panel
    document.getElementById('color-picker-panel').addEventListener('click', e => {
        e.stopPropagation();
    });

    // Color picker event listeners
    document.querySelectorAll('.color-picker').forEach(picker => {
        picker.addEventListener('input', e => {
            const idx = parseInt(e.target.dataset.idx, 10);
            setCustomColor(idx, e.target.value);
        });
        picker.addEventListener('change', e => {
            // Color selection complete - close panel and return focus
            const panel = document.getElementById('color-picker-panel');
            panel.classList.remove('visible');
            document.getElementById('tool-colors').classList.remove('active');
            // Ensure color picker loses focus
            e.target.blur();
            // Return focus to document body (not canvas, to avoid text cursor)
            document.body.focus();
            // Restore the previous tool and clear remembered state
            restoreToolState();
            lastToolBeforeColorEdit = null;
        });
    });

    // ── Tray ─────────────────────────────────────────────────────────────────
    document.getElementById('tray-toggle').addEventListener('click', openTray);
    document.getElementById('tray-close').addEventListener('click',  closeTray);
    document.getElementById('tray-backdrop').addEventListener('click', closeTray);
}

main().catch(err => {
    console.error('FMRL init failed:', err);
    document.getElementById('overlay').querySelector('span').textContent = 'Failed to load WASM.';
});
