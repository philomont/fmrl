import init, { encode_rgba, decode_to_indices } from './pkg/fmrl.js';

// ── Constants ──────────────────────────────────────────────────────────────

const W = 128;
const H = 128;
const SCALE = 4; // display at 4× → 512×512 CSS px

// Default palette: ink, paper, crimson, white
const PALETTE = [
    [0,   0,   0  ],   // 0 ink
    [230, 220, 195],   // 1 paper (background / eraser)
    [180, 30,  30 ],   // 2 crimson
    [255, 255, 255],   // 3 white
];

// ── Drawing state ──────────────────────────────────────────────────────────

let indices   = new Uint8Array(W * H).fill(1); // start with paper
let colorIdx  = 0;   // selected palette index
let brushSize = 1;   // radius in logical pixels
let drawing   = false;
let lastX     = -1;
let lastY     = -1;

// ── Rendering ──────────────────────────────────────────────────────────────

function render() {
    const canvas = document.getElementById('canvas');
    const ctx    = canvas.getContext('2d');

    const tmp    = document.createElement('canvas');
    tmp.width    = W;
    tmp.height   = H;
    const imgData = tmp.getContext('2d').createImageData(W, H);

    for (let i = 0; i < W * H; i++) {
        const [r, g, b] = PALETTE[indices[i]];
        imgData.data[i * 4]     = r;
        imgData.data[i * 4 + 1] = g;
        imgData.data[i * 4 + 2] = b;
        imgData.data[i * 4 + 3] = 255;
    }

    tmp.getContext('2d').putImageData(imgData, 0, 0);
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(tmp, 0, 0, W * SCALE, H * SCALE);
}

// ── Drawing ────────────────────────────────────────────────────────────────

function canvasCoords(e, canvas) {
    const r  = canvas.getBoundingClientRect();
    const cx = (e.clientX ?? e.touches[0].clientX) - r.left;
    const cy = (e.clientY ?? e.touches[0].clientY) - r.top;
    return [
        Math.floor(cx * W / r.width),
        Math.floor(cy * H / r.height),
    ];
}

// Draw a circle of radius `brushSize` at (cx, cy).
function paintAt(cx, cy) {
    const r = brushSize;
    for (let dy = -r; dy <= r; dy++) {
        for (let dx = -r; dx <= r; dx++) {
            if (dx * dx + dy * dy <= r * r + 0.5) {
                const px = cx + dx;
                const py = cy + dy;
                if (px >= 0 && px < W && py >= 0 && py < H) {
                    indices[py * W + px] = colorIdx;
                }
            }
        }
    }
}

// Interpolate between (x0,y0) and (x1,y1) to avoid gaps when the mouse moves fast.
function paintLine(x0, y0, x1, y1) {
    const steps = Math.max(Math.abs(x1 - x0), Math.abs(y1 - y0), 1);
    for (let s = 0; s <= steps; s++) {
        const t = s / steps;
        paintAt(
            Math.round(x0 + (x1 - x0) * t),
            Math.round(y0 + (y1 - y0) * t),
        );
    }
}

// ── Aging — morphological erosion ─────────────────────────────────────────
//
// Each non-paper pixel with 5 or more paper neighbours becomes paper.
// Out-of-bounds positions are treated as paper (border erodes naturally).
// This strictly removes information: strokes thin from the edges inward,
// creating larger uniform paper regions that compress smaller under zlib.

function applyAge() {
    const next = indices.slice();

    for (let y = 0; y < H; y++) {
        for (let x = 0; x < W; x++) {
            if (indices[y * W + x] === 1) continue; // paper stays

            let paperNeighbours = 0;
            for (let dy = -1; dy <= 1; dy++) {
                for (let dx = -1; dx <= 1; dx++) {
                    if (dx === 0 && dy === 0) continue;
                    const nx = x + dx;
                    const ny = y + dy;
                    if (nx < 0 || nx >= W || ny < 0 || ny >= H) {
                        paperNeighbours++; // border counts as paper
                    } else if (indices[ny * W + nx] === 1) {
                        paperNeighbours++;
                    }
                }
            }
            if (paperNeighbours >= 5) next[y * W + x] = 1;
        }
    }

    indices = next;
    render();
}

// ── Save / Load ────────────────────────────────────────────────────────────

function indicesToRgba() {
    const rgba = new Uint8Array(W * H * 4);
    for (let i = 0; i < W * H; i++) {
        const [r, g, b] = PALETTE[indices[i]];
        rgba[i * 4]     = r;
        rgba[i * 4 + 1] = g;
        rgba[i * 4 + 2] = b;
        rgba[i * 4 + 3] = 255;
    }
    return rgba;
}

function saveFmrl() {
    try {
        const bytes = encode_rgba(indicesToRgba(), W, H);
        const blob  = new Blob([bytes], { type: 'application/octet-stream' });
        const url   = URL.createObjectURL(blob);
        const a     = document.createElement('a');
        a.href      = url;
        a.download  = 'manuscript.fmrl';
        a.click();
        URL.revokeObjectURL(url);
    } catch (e) {
        console.error('encode failed:', e);
    }
}

function loadFmrl(arrayBuffer) {
    try {
        const loaded = decode_to_indices(new Uint8Array(arrayBuffer));
        // loaded is a Uint8Array of W×H palette indices
        if (loaded.length !== W * H) {
            alert(`Unsupported size: expected ${W * H} pixels, got ${loaded.length}`);
            return;
        }
        indices = new Uint8Array(loaded);
        render();
    } catch (e) {
        alert(`Failed to load .fmrl: ${e}`);
    }
}

// ── Wire up UI ─────────────────────────────────────────────────────────────

async function main() {
    await init();

    const canvas = document.getElementById('canvas');
    document.getElementById('overlay').classList.add('hidden');
    render();

    // — Canvas drawing events —
    canvas.addEventListener('mousedown', e => {
        drawing = true;
        const [cx, cy] = canvasCoords(e, canvas);
        lastX = cx; lastY = cy;
        paintAt(cx, cy);
        render();
    });

    canvas.addEventListener('mousemove', e => {
        if (!drawing) return;
        const [cx, cy] = canvasCoords(e, canvas);
        paintLine(lastX, lastY, cx, cy);
        lastX = cx; lastY = cy;
        render();
    });

    const stopDrawing = () => { drawing = false; lastX = -1; lastY = -1; };
    canvas.addEventListener('mouseup',    stopDrawing);
    canvas.addEventListener('mouseleave', stopDrawing);

    // Touch support
    canvas.addEventListener('touchstart', e => {
        e.preventDefault();
        drawing = true;
        const [cx, cy] = canvasCoords(e, canvas);
        lastX = cx; lastY = cy;
        paintAt(cx, cy);
        render();
    }, { passive: false });

    canvas.addEventListener('touchmove', e => {
        e.preventDefault();
        if (!drawing) return;
        const [cx, cy] = canvasCoords(e, canvas);
        paintLine(lastX, lastY, cx, cy);
        lastX = cx; lastY = cy;
        render();
    }, { passive: false });

    canvas.addEventListener('touchend', e => { e.preventDefault(); stopDrawing(); }, { passive: false });

    // — Palette swatches —
    document.querySelectorAll('.swatch').forEach(btn => {
        btn.addEventListener('click', () => {
            document.querySelectorAll('.swatch').forEach(b => b.classList.remove('active'));
            btn.classList.add('active');
            colorIdx = parseInt(btn.dataset.idx, 10);
        });
    });

    // — Brush size —
    document.querySelectorAll('.brush-btn').forEach(btn => {
        btn.addEventListener('click', () => {
            document.querySelectorAll('.brush-btn').forEach(b => b.classList.remove('active'));
            btn.classList.add('active');
            brushSize = parseInt(btn.dataset.size, 10);
        });
    });

    // — Age —
    document.getElementById('btn-age').addEventListener('click', applyAge);

    // — Clear —
    document.getElementById('btn-clear').addEventListener('click', () => {
        indices.fill(1);
        render();
    });

    // — Save —
    document.getElementById('btn-save').addEventListener('click', saveFmrl);

    // — Load —
    document.getElementById('file-input').addEventListener('change', e => {
        const file = e.target.files[0];
        if (!file) return;
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
