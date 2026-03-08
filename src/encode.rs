use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;

use crate::error::FmrlError;
use crate::format::{
    AgeEntry, AGE_ENTRY_BYTES, CHUNK_AGE, CHUNK_DATA, CHUNK_IEND, CHUNK_IHDR, CHUNK_META,
    IhdrChunk, MAGIC, Palette, TILE_SIZE, pack_nibbles, write_chunk,
};

/// Input image to encode
pub struct FmrlImage {
    pub width: u16,
    pub height: u16,
    pub palette: Palette,
    /// RGBA row-major pixels, width*height*4 bytes
    pub pixels: Vec<u8>,
    pub decay_policy: u8,
    pub meta: Option<serde_json::Value>,
}

impl FmrlImage {
    /// Create with the default aged-paper palette
    pub fn new(width: u16, height: u16, pixels: Vec<u8>) -> Self {
        FmrlImage {
            width,
            height,
            palette: Palette::default(),
            pixels,
            decay_policy: 0,
            meta: None,
        }
    }
}

/// Quantize an RGBA pixel to the nearest palette index (Euclidean distance on RGB).
fn quantize_pixel(r: u8, g: u8, b: u8, palette: &Palette) -> u8 {
    let mut best_idx = 0u8;
    let mut best_dist = u32::MAX;
    for (i, color) in palette.0.iter().enumerate() {
        let dr = (r as i32) - (color[0] as i32);
        let dg = (g as i32) - (color[1] as i32);
        let db = (b as i32) - (color[2] as i32);
        let dist = (dr * dr + dg * dg + db * db) as u32;
        if dist < best_dist {
            best_dist = dist;
            best_idx = i as u8;
        }
    }
    best_idx
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

    // Step 1: quantize all pixels to palette indices
    let mut indices = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let base = (y * w + x) * 4;
            let r = image.pixels[base];
            let g = image.pixels[base + 1];
            let b = image.pixels[base + 2];
            // alpha ignored for palette quantization
            indices[y * w + x] = quantize_pixel(r, g, b, &image.palette);
        }
    }

    let mut out = Vec::new();

    // Magic
    out.extend_from_slice(&MAGIC);

    // IHDR chunk
    let ihdr = IhdrChunk::new(image.width, image.height, image.decay_policy);
    write_chunk(&mut out, CHUNK_IHDR, &ihdr.to_bytes());

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
    write_chunk(&mut out, CHUNK_DATA, &data_payload);

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
