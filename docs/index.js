import init, { FmrlView, encode_rgba, decode_to_indices } from './pkg/fmrl.js';

// ── Canvas dimensions ───────────────────────────────────────────────────────
// Set from window on init (capped so the larger dimension ≤ 1024), updated
// on file load. Both dimensions are rounded to the nearest multiple of 32
// (the codec tile boundary). CSS scales the canvas to fill the viewport.

let W = 0;
let H = 0;

function computeCanvasDims(srcW, srcH) {
    const MAX = 1024;
    const scale = Math.min(1, MAX / Math.max(srcW, srcH));
    const w = Math.min(Math.ceil(srcW * scale / 32) * 32, MAX);
    const h = Math.min(Math.ceil(srcH * scale / 32) * 32, MAX);
    return [w, h];
}

// Default palette: ink, paper, crimson, white
const PALETTE = [
    [  0,   0,   0],  // 0  ink
    [230, 220, 195],  // 1  paper (background / eraser)
    [180,  30,  30],  // 2  crimson
    [255, 255, 255],  // 3  white
];

// ── Drawing state ───────────────────────────────────────────────────────────

let indices   = null;
let colorIdx  = 0;
let brushSize = 4;
let drawing   = false;
let lastX     = -1;
let lastY     = -1;
let lastSize  = 0;

// ── Rendering ───────────────────────────────────────────────────────────────

let canvas;

function render() {
    const ctx     = canvas.getContext('2d');
    const imgData = ctx.createImageData(W, H);
    for (let i = 0; i < W * H; i++) {
        const [r, g, b] = PALETTE[indices[i]];
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
//   full=true  — morphological erosion + short-run elimination.
//   full=false — morphological erosion only (for fluid passive aging).

const RUN_THRESHOLD = 2;

function _doAgeStep(src, full = true) {
    const next = src.slice();
    const w = W, h = H;

    // Pass 1: morphological erosion (≥3 paper 8-neighbours → paper).
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

    // Pass 2a: short-run elimination — rows
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

    // Pass 2b: short-run elimination — columns
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
    for (let i = 0; i < W * H; i++) {
        const [r, g, b] = PALETTE[src[i]];
        rgba[i * 4] = r; rgba[i * 4 + 1] = g;
        rgba[i * 4 + 2] = b; rgba[i * 4 + 3] = 255;
    }
    return rgba;
}

function updateMetric() {
    const fmrl = encode_rgba(indicesToRgba(), W, H);
    const size = fmrl.length;
    const el   = document.getElementById('size-metric');
    let text = `${size.toLocaleString()} bytes`;
    if (lastSize > 0) {
        const diff = size - lastSize;
        if (diff !== 0) text += `  ${diff < 0 ? '↓' : '↑'} ${Math.abs(diff).toLocaleString()}`;
    }
    el.textContent = text;
    lastSize = size;
}

// ── Passive aging ────────────────────────────────────────────────────────────

const PASSIVE_RATES_S = [0.05, 0.1, 0.2, 0.5, 1, 2];
let passiveRateIdx = 4;
let passiveTimer   = null;

function passiveIntervalMs() { return PASSIVE_RATES_S[passiveRateIdx] * 1000; }

function updateRateDisplay() {
    const s = PASSIVE_RATES_S[passiveRateIdx];
    document.getElementById('rate-display').textContent =
        s < 1 ? (s * 1000) + ' ms / step' : s + ' s / step';
}

function setPassiveAging(enabled) {
    clearInterval(passiveTimer);
    passiveTimer = null;
    const btn = document.getElementById('btn-passive');
    if (enabled) {
        passiveTimer = setInterval(() => {
            indices = _doAgeStep(indices, false);
            render();
            updateMetric();
        }, passiveIntervalMs());
        btn.classList.add('active');
        btn.textContent = 'Passive aging  ON';
    } else {
        btn.classList.remove('active');
        btn.textContent = 'Passive aging  off';
    }
}

// ── Text tool ────────────────────────────────────────────────────────────────
//
// Renders text in the current palette color using National Park (woff2).
// Click canvas to place the baseline cursor; type to build up a string.
// Enter moves the cursor down one line; Escape cancels without committing.
// Switching to any other tool or clicking T again commits pending text.

let textMode        = false;
let textCursor      = null;   // {x, y} baseline in canvas pixels
let textBuffer      = '';
let textBaseIndices = null;   // indices snapshot at start of current line
let textFontSize    = 32;
let cursorBlink     = true;
let cursorTimer     = null;

// Persistent offscreen canvas for text rasterisation — avoids repeated
// allocations on every keypress and cursor blink.
let textHelper    = null;
let textHelperCtx = null;

function getTextCtx() {
    if (!textHelper) {
        textHelper    = document.createElement('canvas');
        textHelper.width  = W;
        textHelper.height = H;
        textHelperCtx = textHelper.getContext('2d');
    }
    return textHelperCtx;
}

function setTextMode(on) {
    if (on === textMode) return;
    textMode = on;
    if (!on) {
        _commitText();
        stopCursorBlink();
        canvas.style.cursor = 'crosshair';
        document.getElementById('tool-text').classList.remove('active');
    } else {
        textCursor      = null;
        textBuffer      = '';
        textBaseIndices = null;
        canvas.style.cursor = 'text';
        document.getElementById('tool-text').classList.add('active');
    }
}

function placeTextCursor(cx, cy) {
    if (textCursor !== null) _commitText();
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

function _commitText() {
    if (!textCursor || !textBuffer) return;
    stopCursorBlink();
    _blitText(textBuffer);
    textBaseIndices = indices.slice();
    textBuffer      = '';
}

// Rasterise `text` into the global `indices` at `textCursor` using the
// current colorIdx. Reads the offscreen canvas pixel-by-pixel and maps
// any opaque pixel to the selected palette slot.
function _blitText(text) {
    if (!textCursor) return;
    if (textBaseIndices) indices.set(textBaseIndices);
    if (!text) { render(); return; }

    const ctx  = getTextCtx();
    const font = `${textFontSize}px "National Park", serif`;
    ctx.clearRect(0, 0, W, H);
    ctx.font      = font;
    ctx.fillStyle = '#000000';
    ctx.fillText(text, textCursor.x, textCursor.y);

    const m       = ctx.measureText(text);
    const ascent  = (m.fontBoundingBoxAscent  ?? textFontSize)           + 4;
    const descent = (m.fontBoundingBoxDescent ?? Math.ceil(textFontSize * 0.3)) + 4;

    const x0 = Math.max(0, Math.floor(textCursor.x - 2));
    const y0 = Math.max(0, Math.floor(textCursor.y - ascent));
    const x1 = Math.min(W, Math.ceil(textCursor.x + Math.max(m.width, 4) + 4));
    const y1 = Math.min(H, Math.ceil(textCursor.y + descent));

    if (x1 > x0 && y1 > y0) {
        const img = ctx.getImageData(x0, y0, x1 - x0, y1 - y0);
        const bw  = x1 - x0, bh = y1 - y0;
        for (let row = 0; row < bh; row++) {
            for (let col = 0; col < bw; col++) {
                if (img.data[(row * bw + col) * 4 + 3] > 64) {
                    indices[(y0 + row) * W + (x0 + col)] = colorIdx;
                }
            }
        }
    }
    render();
}

// ── Save / Load ─────────────────────────────────────────────────────────────

function saveFmrl() {
    try {
        let aged = indices.slice();
        for (let i = 0; i < 10; i++) aged = _doAgeStep(aged, true);
        const bytes = encode_rgba(indicesToRgba(aged), W, H);
        const url   = URL.createObjectURL(new Blob([bytes], { type: 'application/octet-stream' }));
        Object.assign(document.createElement('a'), { href: url, download: 'manuscript.fmrl' }).click();
        URL.revokeObjectURL(url);
    } catch (e) { console.error('encode failed:', e); }
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
        textHelper = null; // reset offscreen canvas to match new dims

        indices = new Uint8Array(decode_to_indices(bytes));
        indices = _doAgeStep(indices, true);
        render();
        lastSize = 0;
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

    canvas = document.getElementById('canvas');
    [W, H] = computeCanvasDims(window.innerWidth, window.innerHeight);
    canvas.width  = W;
    canvas.height = H;
    indices = new Uint8Array(W * H).fill(1);

    // Kick off async font load so it's ready when the text tool is first used.
    document.fonts.load(`${textFontSize}px "National Park"`).catch(() => {});

    document.getElementById('overlay').classList.add('hidden');
    render();
    updateMetric();

    // ── Drawing events ──────────────────────────────────────────────────────
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
            if (textBaseIndices) indices.set(textBaseIndices);
            textBuffer = ''; textCursor = null;
            stopCursorBlink(); render(); return;
        }
        if (e.key === 'Enter') {
            const lineH = Math.round(textFontSize * 1.4);
            _commitText();
            textCursor = { x: textCursor.x, y: textCursor.y + lineH };
            textBaseIndices = indices.slice();
            startCursorBlink(); return;
        }
        if (e.key === 'Backspace') {
            textBuffer = textBuffer.slice(0, -1);
            _blitText(textBuffer + (cursorBlink ? '|' : '')); return;
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
            if (btn.id === 'tool-text') return; // handled separately
            document.querySelectorAll('.brush-btn').forEach(b => b.classList.remove('active'));
            btn.classList.add('active');
            brushSize = parseInt(btn.dataset.size, 10);
            setTextMode(false);
        })
    );

    // ── Text tool ────────────────────────────────────────────────────────────
    document.getElementById('tool-text').addEventListener('click', () => {
        setTextMode(!textMode);
        if (!textMode) {
            // Re-activate whichever brush-btn was last active.
            const active = document.querySelector('.brush-btn[data-size].active');
            if (!active) document.querySelector('.brush-btn[data-size]').classList.add('active');
        }
    });

    document.querySelectorAll('.textsize-btn').forEach(btn =>
        btn.addEventListener('click', () => {
            document.querySelectorAll('.textsize-btn').forEach(b => b.classList.remove('active'));
            btn.classList.add('active');
            textFontSize = parseInt(btn.dataset.size, 10);
        })
    );

    // ── Age controls ─────────────────────────────────────────────────────────
    document.getElementById('btn-age').addEventListener('click',    () => applyAge(1));
    document.getElementById('btn-age10').addEventListener('click',  () => applyAge(10));
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
        indices.fill(1); render(); lastSize = 0; updateMetric();
    });
    document.getElementById('btn-save').addEventListener('click', saveFmrl);
    document.getElementById('file-input').addEventListener('change', e => {
        const file = e.target.files[0]; if (!file) return;
        const reader = new FileReader();
        reader.onload = ev => loadFmrl(ev.target.result);
        reader.readAsArrayBuffer(file);
        e.target.value = '';
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
