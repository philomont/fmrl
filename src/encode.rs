use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;

// use crate::age::age_step;
use crate::error::FmrlError;
use crate::format::{
    AgeEntry, AGE_ENTRY_BYTES, CHUNK_AGE, CHUNK_DATA, CHUNK_IEND, CHUNK_IHDR, CHUNK_META,
    ColorMode, IhdrChunk, MAGIC, Palette, TILE_SIZE, pack_nibbles, write_chunk,
};

/// Input image to encode
pub struct FmrlImage {
    pub width: u16,
    pub height: u16,
    pub color_mode: ColorMode,
    pub palette: Palette,
    /// RGBA row-major pixels, width*height*4 bytes
    pub pixels: Vec<u8>,
    pub decay_policy: u8,
    pub meta: Option<serde_json::Value>,
}

impl FmrlImage {
    /// Create with the default aged-paper palette (indexed mode)
    pub fn new(width: u16, height: u16, pixels: Vec<u8>) -> Self {
        FmrlImage {
            width,
            height,
            color_mode: ColorMode::Indexed,
            palette: Palette::default(),
            pixels,
            decay_policy: 0,
            meta: None,
        }
    }

    /// Create in RGBA mode (full color, no palette quantization)
    pub fn new_rgba(width: u16, height: u16, pixels: Vec<u8>) -> Self {
        FmrlImage {
            width,
            height,
            color_mode: ColorMode::Rgba,
            palette: Palette::default(), // Still used for paper color reference
            pixels,
            decay_policy: 0,
            meta: None,
        }
    }
}

/// Quantize an RGBA pixel to a palette index using alpha + grayscale mapping.
///
/// Storage format (theme-agnostic):
/// 0 = ink (black [0,0,0], alpha=255) → renders as theme --ink
/// 1 = paper (transparent, alpha=0) → renders as theme --paper
/// 2 = accent (white [255,255,255], alpha=255) → renders as theme --accent
/// 3 = highlight (gray [128,128,128], alpha=255) → renders as theme --highlight
///
/// Alpha is checked first to distinguish paper (transparent) from accent (white).
fn quantize_pixel(r: u8, g: u8, b: u8, a: u8) -> u8 {
    // Transparent pixels are paper (index 1)
    // This distinguishes paper (alpha=0) from accent (white, alpha=255)
    if a < 128 {
        return 1;
    }

    // For opaque pixels, use brightness for grayscale mapping
    let brightness = (r as u16 + g as u16 + b as u16) / 3;

    // Direct mapping based on brightness thresholds:
    // 0-63   → ink (black)
    // 64-191 → highlight (gray)
    // 192+   → accent (white)
    if brightness < 64 {
        0 // ink - black
    } else if brightness > 191 {
        2 // accent - white
    } else {
        3 // highlight - gray
    }
}

/// Compress bytes with zlib (not raw DEFLATE).
fn zlib_compress(data: &[u8]) -> Result<Vec<u8>, FmrlError> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).map_err(|e| FmrlError::CompressionError(e.to_string()))?;
    encoder.finish().map_err(|e| FmrlError::CompressionError(e.to_string()))
}

/// Encode an `FmrlImage` to `.fmrl` bytes.
pub fn encode(image: &FmrlImage, now_ms: u64) -> Result<Vec<u8>, FmrlError> {
    let w = image.width as usize;
    let h = image.height as usize;

    if w == 0 || h == 0 {
        return Err(FmrlError::MalformedChunk("image dimensions must be non-zero"));
    }
    if w > 65504 || h > 65504 {
        return Err(FmrlError::MalformedChunk("image dimensions exceed maximum (65504)"));
    }
    if !w.is_multiple_of(TILE_SIZE) || !h.is_multiple_of(TILE_SIZE) {
        return Err(FmrlError::MalformedChunk("dimensions must be multiples of 32"));
    }
    if image.pixels.len() != w * h * 4 {
        return Err(FmrlError::MalformedChunk("pixel buffer size mismatch"));
    }

    let tiles_x = w / TILE_SIZE;
    let tiles_y = h / TILE_SIZE;

    let mut out = Vec::new();

    // Magic
    out.extend_from_slice(&MAGIC);

    // IHDR chunk
    let ihdr = IhdrChunk::new(image.width, image.height, image.color_mode, image.decay_policy);
    write_chunk(&mut out, CHUNK_IHDR, &ihdr.to_bytes());

    // DATA chunk: mode-dependent
    match image.color_mode {
        ColorMode::Indexed => encode_indexed(&mut out, image, w, h, tiles_x, tiles_y)?,
        ColorMode::Rgba => encode_rgba(&mut out, image, w, h, tiles_x, tiles_y)?,
    }

    // AGE chunk: one entry per tile (row-major)
    let mut age_payload = Vec::with_capacity(tiles_x * tiles_y * AGE_ENTRY_BYTES);
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let entry = AgeEntry {
                tx: tx as u16,
                ty: ty as u16,
                last_view: now_ms,
                fade_level: 0,
                noise_seed: [tx as u8, (tx >> 8) as u8, ty as u8, (ty >> 8) as u8],
                edge_damage: 0,
                reserved: 0,
            };
            age_payload.extend_from_slice(&entry.to_bytes());
        }
    }
    write_chunk(&mut out, CHUNK_AGE, &age_payload);

    // META chunk (optional): JSON → UTF-8 → zlib
    if let Some(meta) = &image.meta {
        let json_str = serde_json::to_string(meta)
            .map_err(|_| FmrlError::MalformedChunk("failed to serialize META JSON"))?;
        let compressed = zlib_compress(json_str.as_bytes())?;
        write_chunk(&mut out, CHUNK_META, &compressed);
    }

    // IEND
    write_chunk(&mut out, CHUNK_IEND, &[]);

    Ok(out)
}

/// Encode indexed mode: palette (12 bytes) + packed nibble tiles
fn encode_indexed(
    out: &mut Vec<u8>,
    image: &FmrlImage,
    w: usize,
    h: usize,
    tiles_x: usize,
    tiles_y: usize,
) -> Result<(), FmrlError> {
    // Step 1: quantize all pixels to palette indices
    let mut indices = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let base = (y * w + x) * 4;
            let r = image.pixels[base];
            let g = image.pixels[base + 1];
            let b = image.pixels[base + 2];
            let a = image.pixels[base + 3];
            indices[y * w + x] = quantize_pixel(r, g, b, a);
        }
    }

    // Step 2: apply one aging step (morphological erosion + short-run elimination)
    // This is the FMRL protocol: each encode->decode cycle ages the image
    // DISABLED: Aging during save is too aggressive for drawing app
    // indices = age_step(&indices, w, h);

    // DATA chunk: palette (12 bytes) + tiles
    let mut data_payload: Vec<u8> = Vec::new();
    // Palette: 4 colors × 3 bytes RGB
    for color in &image.palette.0 {
        data_payload.extend_from_slice(color);
    }

    // Per-tile: [u16 compressed_len LE][u8 flags][compressed nibble data]
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let tile_indices = extract_tile_indices(&indices, w, tx, ty);
            let packed = pack_nibbles(&tile_indices);
            let compressed = zlib_compress(&packed)?;
            let len = compressed.len() as u16;
            data_payload.extend_from_slice(&len.to_le_bytes());
            data_payload.push(0u8); // flags
            data_payload.extend_from_slice(&compressed);
        }
    }
    write_chunk(out, CHUNK_DATA, &data_payload);
    Ok(())
}

/// Encode RGBA mode: paper color (3 bytes) + raw RGBA tiles
fn encode_rgba(
    out: &mut Vec<u8>,
    image: &FmrlImage,
    w: usize,
    h: usize,
    tiles_x: usize,
    tiles_y: usize,
) -> Result<(), FmrlError> {
    // DATA chunk: paper color RGB (3 bytes) + tiles
    let mut data_payload: Vec<u8> = Vec::new();
    // Store paper color for fade target
    data_payload.extend_from_slice(&image.palette.0[1]);

    // Per-tile: [u16 compressed_len LE][u8 flags][compressed RGBA data]
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let tile_rgba = extract_tile_rgba(&image.pixels, w, h, tx, ty);
            let compressed = zlib_compress(&tile_rgba)?;
            let len = compressed.len() as u16;
            data_payload.extend_from_slice(&len.to_le_bytes());
            data_payload.push(0u8); // flags
            data_payload.extend_from_slice(&compressed);
        }
    }
    write_chunk(out, CHUNK_DATA, &data_payload);
    Ok(())
}

fn extract_tile_rgba(pixels: &[u8], width: usize, _height: usize, tx: usize, ty: usize) -> Vec<u8> {
    let mut tile = Vec::with_capacity(TILE_SIZE * TILE_SIZE * 4);
    let x_start = tx * TILE_SIZE;
    let y_start = ty * TILE_SIZE;
    for y in y_start..y_start + TILE_SIZE {
        let row_start = (y * width + x_start) * 4;
        tile.extend_from_slice(&pixels[row_start..row_start + TILE_SIZE * 4]);
    }
    tile
}

fn extract_tile_indices(indices: &[u8], width: usize, tx: usize, ty: usize) -> Vec<u8> {
    let mut tile = Vec::with_capacity(TILE_SIZE * TILE_SIZE);
    let x_start = tx * TILE_SIZE;
    let y_start = ty * TILE_SIZE;
    for y in y_start..y_start + TILE_SIZE {
        let row_start = y * width + x_start;
        tile.extend_from_slice(&indices[row_start..row_start + TILE_SIZE]);
    }
    tile
}
