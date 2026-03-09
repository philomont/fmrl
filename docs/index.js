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

// Default palette: ink, paper, crimson, white
const PALETTE = [
    [  0,   0,   0],
    [230, 220, 195],
    [180,  30,  30],
    [255, 255, 255],
];

// ── Drawing state ───────────────────────────────────────────────────────────

let indices   = null;
let colorIdx  = 0;
let brushSize = 2;   // matches first brush-btn data-size
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
    for (let i = 0; i < W * H; i++) {
        const [r, g, b] = PALETTE[src[i]];
        rgba[i * 4] = r; rgba[i * 4 + 1] = g;
        rgba[i * 4 + 2] = b; rgba[i * 4 + 3] = 255;
    }
    return rgba;
}

function updateMetric() {
    const size = encode_rgba(indicesToRgba(), W, H).length;
    const el   = document.getElementById('size-metric');
    let text = `${size.toLocaleString()} B`;
    if (lastSize > 0) {
        const diff = size - lastSize;
        if (diff !== 0) text += `  ${diff < 0 ? '↓' : '↑'}${Math.abs(diff).toLocaleString()}`;
    }
    el.textContent = text;
    lastSize = size;
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
    ctx.fillStyle = '#000000';
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
        textHelper = null;

        indices  = new Uint8Array(decode_to_indices(bytes));
        indices  = _doAgeStep(indices, true);
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

    document.fonts.load(`${textFontSize()}px "National Park"`).catch(() => {});

    document.getElementById('overlay').classList.add('hidden');
    render();
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
