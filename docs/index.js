import init, { FmrlView, encode_rgba, decode_to_indices } from './pkg/fmrl.js';

// ── Canvas dimensions (mutable — updated when loading a file) ──────────────

let W = 256;
let H = 256;

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
let brushSize = 2;
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
// Two-component degradation, both information-reducing:
//
// 1. Edge erosion (majority vote): a non-paper pixel with ≥ 5 paper
//    neighbours becomes paper.  Strokes thin from the outside in.
//
// 2. Uniform interior thinning (~8% per pass): for pixels that survive
//    edge erosion, a deterministic hash of (x, y, neighbourhood_xor)
//    selects which ones disappear.  The neighbourhood XOR encodes the
//    current image state, so the speckle pattern shifts naturally as the
//    image ages — no Math.random(), no fixed seed, just the existing pixels.
//
// Both paths always convert to paper, never to other colours.

function ageStep() {
    const next = indices.slice();
    const w = W, h = H;

    for (let y = 0; y < h; y++) {
        for (let x = 0; x < w; x++) {
            if (indices[y * w + x] === 1) continue; // paper is immune

            let paperCount = 0;
            let nbXor = 0;

            for (let dy = -1; dy <= 1; dy++) {
                for (let dx = -1; dx <= 1; dx++) {
                    if (dx === 0 && dy === 0) continue;
                    const nx = x + dx, ny = y + dy;
                    const v = (nx < 0 || nx >= w || ny < 0 || ny >= h)
                        ? 1 : indices[ny * w + nx];
                    if (v === 1) paperCount++;
                    nbXor ^= v;
                }
            }

            // 1. Edge erosion
            if (paperCount >= 5) { next[y * w + x] = 1; continue; }

            // 2. Interior thinning — content-derived deterministic hash
            //    Mix position with neighbourhood state; no random variable.
            let h32 = (Math.imul(x + 1, 0x9e3779b9) ^ Math.imul(y + 1, 0x6c62272e)
                       ^ Math.imul(nbXor + 1, 0x27d4eb2f)) >>> 0;
            h32 = Math.imul(h32 ^ (h32 >>> 16), 0x45d9f3b) >>> 0;
            h32 = (h32 ^ (h32 >>> 15)) >>> 0;
            if ((h32 & 0xFF) < 20) next[y * w + x] = 1; // ≈ 7.8 %
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
