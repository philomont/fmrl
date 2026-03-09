import init, { FmrlView, encode_rgba, decode_to_indices } from './pkg/fmrl.js';

// ── Canvas dimensions — set from window on init, updated when loading a file ─

let W = 0;
let H = 0;

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
let lastSize  = 0;  // previous .fmrl byte count for Δ display

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
//
//   full=true  — morphological erosion + short-run elimination.
//                Used by the manual Age button, Age 10×, and save.
//                Maximum data removal per step; file size decreases
//                monotonically.  Convergence to all-paper is guaranteed.
//
//   full=false — morphological erosion only (no short-run elimination).
//                Used by passive aging at high rates (50 ms–2 s) so each
//                individual step is fine-grained and the animation reads as
//                continuous decay rather than discrete jumps.
//
// Both modes only convert non-paper pixels to paper — information is strictly
// non-increasing and convergence to all-paper is guaranteed for both.

const RUN_THRESHOLD = 2; // runs ≤ this many pixels wide are erased (full mode)

function _doAgeStep(src, full = true) {
    const next = src.slice();
    const w = W, h = H;

    // Pass 1: morphological erosion (≥3 paper 8-neighbours → paper).
    // Threshold 3 ensures face pixels of solid blocks are always eligible:
    // every finite non-paper cluster has at least one such pixel, so
    // convergence to all-paper is guaranteed.
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
            } else {
                x++;
            }
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
            } else {
                y++;
            }
        }
    }

    return next;
}

// Run n full age steps on the global canvas and re-render once.
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
//
// Uses a light step (erosion only, no run elimination) so high-rate passive
// aging feels fluid — each step removes a thin boundary layer rather than
// collapsing entire short features all at once.

// Available intervals in seconds, fastest → slowest.
// − button moves toward slower (higher index), + toward faster (lower index).
const PASSIVE_RATES_S = [0.05, 0.1, 0.2, 0.5, 1, 2];
let passiveRateIdx = 4; // default: 1 s/step

let passiveTimer = null;

function passiveIntervalMs() {
    return PASSIVE_RATES_S[passiveRateIdx] * 1000;
}

function updateRateDisplay() {
    const s = PASSIVE_RATES_S[passiveRateIdx];
    const text = s < 1 ? (s * 1000) + ' ms / step' : s + ' s / step';
    document.getElementById('rate-display').textContent = text;
}

function setPassiveAging(enabled) {
    clearInterval(passiveTimer);
    passiveTimer = null;
    const btn = document.getElementById('btn-passive');
    if (enabled) {
        passiveTimer = setInterval(() => {
            indices = _doAgeStep(indices, false); // light step — erosion only
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

// ── Save / Load ─────────────────────────────────────────────────────────────

function saveFmrl() {
    try {
        // Bake 10 full erosion steps into the saved copy.  The file will be
        // noticeably aged when next opened (which applies one more step).
        // The live canvas is not affected.
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

        canvas.width = fileW; canvas.height = fileH;
        W = fileW; H = fileH;

        indices = new Uint8Array(decode_to_indices(bytes));
        // Opening a file ages it — one erosion step representing elapsed time.
        indices = _doAgeStep(indices, true);
        render();
        lastSize = 0;
        updateMetric();
    } catch (e) { alert(`Failed to load .fmrl: ${e}`); }
}

// ── Tray ────────────────────────────────────────────────────────────────────

function openTray() {
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
    // Dimensions must be multiples of 32 (tile size required by the codec).
    // Round up so the canvas always covers the full viewport; the extra pixels
    // (at most 31 on each edge) are clipped by the CSS overflow.
    W = Math.ceil(window.innerWidth  / 32) * 32;
    H = Math.ceil(window.innerHeight / 32) * 32;
    canvas.width  = W;
    canvas.height = H;
    indices = new Uint8Array(W * H).fill(1);
    document.getElementById('overlay').classList.add('hidden');
    render();
    updateMetric();

    // Drawing events
    canvas.addEventListener('mousedown', e => {
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
        e.preventDefault(); drawing = true;
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

    // Palette
    document.querySelectorAll('.swatch').forEach(btn =>
        btn.addEventListener('click', () => {
            document.querySelectorAll('.swatch').forEach(b => b.classList.remove('active'));
            btn.classList.add('active');
            colorIdx = parseInt(btn.dataset.idx, 10);
        })
    );

    // Brush
    document.querySelectorAll('.brush-btn').forEach(btn =>
        btn.addEventListener('click', () => {
            document.querySelectorAll('.brush-btn').forEach(b => b.classList.remove('active'));
            btn.classList.add('active');
            brushSize = parseInt(btn.dataset.size, 10);
        })
    );

    document.getElementById('btn-age').addEventListener('click',    () => applyAge(1));
    document.getElementById('btn-age10').addEventListener('click',   () => applyAge(10));
    document.getElementById('btn-passive').addEventListener('click', e => setPassiveAging(!e.currentTarget.classList.contains('active')));
    document.getElementById('btn-rate-down').addEventListener('click', () => {
        if (passiveRateIdx < PASSIVE_RATES_S.length - 1) {
            passiveRateIdx++;
            updateRateDisplay();
            if (passiveTimer) setPassiveAging(true);
        }
    });
    document.getElementById('btn-rate-up').addEventListener('click', () => {
        if (passiveRateIdx > 0) {
            passiveRateIdx--;
            updateRateDisplay();
            if (passiveTimer) setPassiveAging(true);
        }
    });
    updateRateDisplay();
    document.getElementById('btn-clear').addEventListener('click', () => {
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

    // Tray
    document.getElementById('tray-toggle').addEventListener('click', openTray);
    document.getElementById('tray-close').addEventListener('click',  closeTray);
    document.getElementById('tray-backdrop').addEventListener('click', closeTray);
}

main().catch(err => {
    console.error('FMRL init failed:', err);
    document.getElementById('overlay').querySelector('span').textContent = 'Failed to load WASM.';
});
