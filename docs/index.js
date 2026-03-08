import init, { FmrlView, create_demo_fmrl } from './pkg/fmrl.js';

const STORAGE_KEY = 'fmrl_demo_v1';
const SCALE = 4;

let view = null;

// ── Base64 helpers (avoid spread operator limit for large arrays) ──────────

function bytesToBase64(bytes) {
    let str = '';
    for (let i = 0; i < bytes.length; i++) str += String.fromCharCode(bytes[i]);
    return btoa(str);
}

function base64ToBytes(b64) {
    const str = atob(b64);
    const arr = new Uint8Array(str.length);
    for (let i = 0; i < str.length; i++) arr[i] = str.charCodeAt(i);
    return arr;
}

// ── Rendering ──────────────────────────────────────────────────────────────

function renderRgba(rgba, w, h) {
    const canvas = document.getElementById('canvas');
    canvas.width = w * SCALE;
    canvas.height = h * SCALE;

    // Draw native-size into a temp canvas, then scale up with pixelated rendering
    const tmp = document.createElement('canvas');
    tmp.width = w;
    tmp.height = h;
    tmp.getContext('2d').putImageData(new ImageData(new Uint8ClampedArray(rgba), w, h), 0, 0);

    const ctx = canvas.getContext('2d');
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(tmp, 0, 0, w * SCALE, h * SCALE);

    document.getElementById('overlay').classList.add('hidden');
}

function updateStats() {
    const now = Date.now();
    const lastViewMs = view.last_view_ms();
    const daysSince = ((now - lastViewMs) / 86_400_000).toFixed(1);
    const fade = view.avg_fade_level();
    const views = view.view_count();

    document.getElementById('stat-views').textContent = views;
    document.getElementById('stat-age').textContent = `${daysSince}d`;
    document.getElementById('stat-fade').textContent = `${fade}/255`;
}

// ── Core view action ───────────────────────────────────────────────────────

function viewImage() {
    try {
        const rgba = view.decode_and_decay();
        renderRgba(rgba, view.width(), view.height());
        // Persist mutated bytes
        localStorage.setItem(STORAGE_KEY, bytesToBase64(view.get_mutated_bytes()));
        updateStats();
    } catch (e) {
        console.error('decode_and_decay failed:', e);
    }
}

// ── Load bytes into a FmrlView and immediately view ───────────────────────

function loadBytes(bytes) {
    try {
        view = FmrlView.new(bytes);
    } catch (e) {
        alert(`Failed to load .fmrl: ${e}`);
        return false;
    }
    viewImage();
    return true;
}

// ── Entry point ────────────────────────────────────────────────────────────

async function main() {
    try {
        await init();
    } catch (e) {
        document.getElementById('overlay-text').textContent = 'Failed to load WASM.';
        console.error(e);
        return;
    }

    // Canvas click → view (ages the image)
    document.getElementById('canvas').addEventListener('click', () => {
        if (view) viewImage();
    });

    // Open custom .fmrl file
    document.getElementById('file-input').addEventListener('change', e => {
        const file = e.target.files[0];
        if (!file) return;
        const reader = new FileReader();
        reader.onload = ev => {
            localStorage.removeItem(STORAGE_KEY);
            loadBytes(new Uint8Array(ev.target.result));
        };
        reader.readAsArrayBuffer(file);
        // Reset the input so the same file can be opened again
        e.target.value = '';
    });

    // Reset to a fresh demo
    document.getElementById('btn-reset').addEventListener('click', () => {
        localStorage.removeItem(STORAGE_KEY);
        loadBytes(create_demo_fmrl());
    });

    // Try to restore from localStorage, otherwise create a fresh demo
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) {
        try {
            if (loadBytes(base64ToBytes(stored))) return;
        } catch (_) {
            localStorage.removeItem(STORAGE_KEY);
        }
    }
    loadBytes(create_demo_fmrl());
}

main();
