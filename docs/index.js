import init, { FmrlView, encode_rgba, decode_to_indices } from './pkg/fmrl.js';

// ── Canvas dimensions (mutable — updated when loading a file) ──────────────

let W = 1024;
let H = 1024;

// Default palette: ink, paper, crimson, white
const PALETTE = [
    [  0,   0,   0],  // 0  ink
    [230, 220, 195],  // 1  paper (background / eraser)
    [180,  30,  30],  // 2  crimson
    [255, 255, 255],  // 3  white
];

// ── Drawing state ──────────────────────────────────────────────────────────

let indices   = new Uint8Array(W * H).fill(1); // all paper
let colorIdx  = 0;
let brushSize = 4;
let drawing   = false;
let lastX     = -1;
let lastY     = -1;
let lastSize  = 0;  // previous .fmrl byte count for Δ display

// ── Rendering — canvas logical size set once; CSS scales the display ───────

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

function setCanvasSize(w, h) {
    W = w; H = h;
    canvas.width  = W;
    canvas.height = H;
    indices = new Uint8Array(W * H).fill(1);
}

// ── Drawing ────────────────────────────────────────────────────────────────

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

// ── Aging ──────────────────────────────────────────────────────────────────
//
// Two passes that both lengthen uniform runs → smaller compressed output:
//
// 1. Morphological erosion: a non-paper pixel with ≥ 3 of its 8 neighbours
//    being paper becomes paper.  Threshold 3 means face pixels (3 diagonal/
//    orthogonal neighbours on one side) are eroded each step, guaranteeing
//    convergence to all-paper for any finite mark.
//
// 2. Short-run elimination: scan every row and every column.  Any run of
//    non-paper pixels whose extent is ≤ RUN_THRESHOLD becomes paper.  This
//    collapses thin isolated features and breaks no existing long runs — the
//    surviving non-paper regions are always wider than the threshold, so zlib
//    sees longer regular sequences and produces smaller output.
//
// Neither pass introduces new non-paper pixels; both can only convert to paper.
// File size therefore decreases monotonically with each age step.

const RUN_THRESHOLD = 2; // runs ≤ this many pixels wide are erased

function ageStep() {
    const next = indices.slice();
    const w = W, h = H;

    // Pass 1: morphological erosion (≥3 paper 8-neighbours → paper).
    // Threshold 3 ensures face pixels of solid blocks are eroded each step:
    // every finite non-paper cluster has at least one pixel with 3 paper
    // neighbours, so convergence to all-paper is guaranteed.
    for (let y = 0; y < h; y++) {
        for (let x = 0; x < w; x++) {
            if (indices[y * w + x] === 1) continue;
            let paperCount = 0;
            for (let dy = -1; dy <= 1; dy++) {
                for (let dx = -1; dx <= 1; dx++) {
                    if (dx === 0 && dy === 0) continue;
                    const nx = x + dx, ny = y + dy;
                    if (nx < 0 || nx >= w || ny < 0 || ny >= h ||
                        indices[ny * w + nx] === 1) paperCount++;
                }
            }
            if (paperCount >= 3) next[y * w + x] = 1;
        }
    }

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

    indices = next;
}

// ── Compression metric ─────────────────────────────────────────────────────

function indicesToRgba() {
    const rgba = new Uint8Array(W * H * 4);
    for (let i = 0; i < W * H; i++) {
        const [r, g, b] = PALETTE[indices[i]];
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

// Run n age steps then re-render and update the metric once.
function applyAge(n = 1) {
    for (let i = 0; i < n; i++) ageStep();
    render();
    updateMetric();
}

// ── Passive aging ───────────────────────────────────────────────────────────
//
// Mimics slow environmental degradation — UV bleaching, mineral dissolution,
// water evaporation on stone.  One age step fires every PASSIVE_INTERVAL_MS
// while the toggle is on.  Uses the identical algorithm as the manual Age
// button; the difference is only the rate.

const PASSIVE_INTERVAL_MS = 10_000; // one erosion pass every 10 seconds

let passiveTimer = null;

function setPassiveAging(enabled) {
    clearInterval(passiveTimer);
    passiveTimer = null;
    const btn = document.getElementById('btn-passive');
    if (enabled) {
        passiveTimer = setInterval(() => applyAge(1), PASSIVE_INTERVAL_MS);
        btn.classList.add('active');
        btn.textContent = 'Passive aging  ON';
    } else {
        btn.classList.remove('active');
        btn.textContent = 'Passive aging  off';
    }
}

// ── Save / Load ────────────────────────────────────────────────────────────

function saveFmrl() {
    try {
        const bytes = encode_rgba(indicesToRgba(), W, H);
        const url   = URL.createObjectURL(new Blob([bytes], { type: 'application/octet-stream' }));
        Object.assign(document.createElement('a'), { href: url, download: 'manuscript.fmrl' }).click();
        URL.revokeObjectURL(url);
    } catch (e) { console.error('encode failed:', e); }
}

function loadFmrl(arrayBuffer) {
    try {
        const bytes = new Uint8Array(arrayBuffer);
        // Peek at dimensions without applying any decay.
        const peek = FmrlView.new(bytes);
        const fileW = peek.width(), fileH = peek.height();
        peek.free();

        // Resize canvas to match loaded file.
        canvas.width = fileW; canvas.height = fileH;
        W = fileW; H = fileH;

        indices = new Uint8Array(decode_to_indices(bytes));
        render();
        lastSize = 0;
        updateMetric();
    } catch (e) { alert(`Failed to load .fmrl: ${e}`); }
}

// ── Main ───────────────────────────────────────────────────────────────────

async function main() {
    await init();

    canvas = document.getElementById('canvas');
    canvas.width  = W;
    canvas.height = H;
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
    document.getElementById('btn-clear').addEventListener('click',   () => { indices.fill(1); render(); lastSize = 0; updateMetric(); });
    document.getElementById('btn-save').addEventListener('click',    saveFmrl);
    document.getElementById('file-input').addEventListener('change', e => {
        const file = e.target.files[0]; if (!file) return;
        const reader = new FileReader();
        reader.onload = ev => loadFmrl(ev.target.result);
        reader.readAsArrayBuffer(file);
        e.target.value = '';
    });
}

main().catch(err => {
    console.error('FMRL init failed:', err);
    document.getElementById('overlay').querySelector('span').textContent = 'Failed to load WASM.';
});
