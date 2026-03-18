use std::io::Write;

use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::age::{
    age_by_bleaching, age_by_consolidation, age_by_erosion, consolidation_step_with_age,
};
use crate::error::FmrlError;
use crate::format::{
    write_chunk, AgeType, IhdrChunk, Palette, CHUNK_AGE, CHUNK_DATA, CHUNK_IEND, CHUNK_IHDR,
    CHUNK_META, MAGIC, TILE_SIZE,
};

/// Input image to encode
pub struct FmrlImage {
    pub width: u16,
    pub height: u16,
    pub palette: Palette,
    /// RGB row-major pixels, width*height*3 bytes
    /// Format: R = index × 16, G = contrast (0x00 for paper, 0xFF otherwise), B = age × 16
    pub pixels: Vec<u8>,
    pub decay_policy: u8,
    /// Stack of aging algorithms to apply (up to 8)
    pub age_types: Vec<AgeType>,
    /// Optional per-tile consolidation levels (for re-saving existing files)
    pub age_levels: Option<Vec<u8>>,
    /// Optional per-pixel ages (width*height bytes) for independent pixel aging
    /// If None, tile-level ages are used
    pub pixel_ages: Option<Vec<u8>>,
    pub meta: Option<serde_json::Value>,
}

impl FmrlImage {
    /// Create with the default aged-paper palette (indexed mode)
    pub fn new(width: u16, height: u16, pixels: Vec<u8>) -> Self {
        FmrlImage {
            width,
            height,
            palette: Palette::default(),
            pixels,
            decay_policy: 0,
            age_types: vec![AgeType::Erosion],
            age_levels: None,
            pixel_ages: None,
            meta: None,
        }
    }
}

/// Extract index and age from RGB pixel format.
///
/// Format: R = index × 16, G = contrast (ignored), B = age × 16
/// Returns (index, age) where both are in range 0-15
fn extract_index_and_age(r: u8, _g: u8, b: u8) -> (u8, u8) {
    // Extract index from red channel (divide by 16, clamp to 0-15)
    let index = (r >> 4).min(15);
    // Extract age from blue channel (divide by 16, clamp to 0-15)
    let age = (b >> 4).min(15);
    (index, age)
}

/// Compress bytes with zlib (not raw DEFLATE).
/// Uses best compression for smallest file size.
pub fn zlib_compress(data: &[u8]) -> Result<Vec<u8>, FmrlError> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(data)
        .map_err(|e| FmrlError::CompressionError(e.to_string()))?;
    encoder
        .finish()
        .map_err(|e| FmrlError::CompressionError(e.to_string()))
}

/// Encode an `FmrlImage` to `.fmrl` bytes.
pub fn encode(image: &FmrlImage, now_ms: u64) -> Result<Vec<u8>, FmrlError> {
    let w = image.width as usize;
    let h = image.height as usize;

    if w == 0 || h == 0 {
        return Err(FmrlError::MalformedChunk(
            "image dimensions must be non-zero",
        ));
    }
    if w > 65504 || h > 65504 {
        return Err(FmrlError::MalformedChunk(
            "image dimensions exceed maximum (65504)",
        ));
    }
    if !w.is_multiple_of(TILE_SIZE) || !h.is_multiple_of(TILE_SIZE) {
        return Err(FmrlError::MalformedChunk(
            "dimensions must be multiples of TILE_SIZE",
        ));
    }
    if image.pixels.len() != w * h * 3 {
        return Err(FmrlError::MalformedChunk("pixel buffer size mismatch"));
    }

    let tiles_x = w / TILE_SIZE;
    let tiles_y = h / TILE_SIZE;

    let mut out = Vec::new();

    // Magic
    out.extend_from_slice(&MAGIC);

    // IHDR chunk
    let ihdr = IhdrChunk::new(
        image.width,
        image.height,
        image.decay_policy,
        &image.age_types,
    );
    write_chunk(&mut out, CHUNK_IHDR, &ihdr.to_bytes());

    // DATA chunk: encode indexed mode
    // Get age levels from encoding (for consolidation tracking)
    let age_levels = encode_indexed(&mut out, image, w, h, tiles_x, tiles_y)?;

    // AGE chunk: compressed storage for all tiles
    // Format: [u16 entry_count] followed by compressed entries
    // For mostly-uniform data, zlib compression provides significant savings
    let mut age_entries: Vec<(u16, u16, u8)> = Vec::new();

    // Store entries for all tiles (compression will optimize uniform data)
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let tile_idx = ty * tiles_x + tx;
            let tile_age = age_levels.get(tile_idx).copied().unwrap_or(0);
            age_entries.push((tx as u16, ty as u16, tile_age));
        }
    }

    // Build AGE payload: count + compressed entries
    // Each entry: tx(2) + ty(2) + last_view(8) + fade_level(1) + noise_seed(4) + edge_damage(1) = 18 bytes
    let mut age_payload = Vec::new();
    age_payload.extend_from_slice(&(age_entries.len() as u16).to_le_bytes());

    let mut age_data = Vec::with_capacity(age_entries.len() * 18);
    for (tx, ty, level) in age_entries {
        age_data.extend_from_slice(&tx.to_le_bytes());
        age_data.extend_from_slice(&ty.to_le_bytes());
        age_data.extend_from_slice(&now_ms.to_le_bytes());
        age_data.push(level);
        age_data.extend_from_slice(&[tx as u8, (tx >> 8) as u8, ty as u8, (ty >> 8) as u8]); // noise_seed
        age_data.push(0); // edge_damage
    }

    // Compress age data (empty for blank images = just zlib overhead)
    let compressed_age = zlib_compress(&age_data)?;
    age_payload.extend_from_slice(&compressed_age);
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

/// Encode indexed mode: palette (48 bytes) + tiles with full-byte indices
/// Returns the updated age levels for saving to AGE chunk.
fn encode_indexed(
    out: &mut Vec<u8>,
    image: &FmrlImage,
    w: usize,
    h: usize,
    tiles_x: usize,
    tiles_y: usize,
) -> Result<Vec<u8>, FmrlError> {
    // Step 1: extract index and age from RGB pixels
    let mut indices = vec![0u8; w * h];
    let mut pixel_ages = image.pixel_ages.clone().unwrap_or_else(|| vec![0u8; w * h]);

    for y in 0..h {
        for x in 0..w {
            let base = (y * w + x) * 3;
            let r = image.pixels[base];
            let g = image.pixels[base + 1];
            let b = image.pixels[base + 2];
            let (index, age) = extract_index_and_age(r, g, b);
            indices[y * w + x] = index;
            pixel_ages[y * w + x] = age;
        }
    }

    // Step 2: apply aging steps sequentially based on age_types stack
    let mut age_levels = image
        .age_levels
        .clone()
        .unwrap_or_else(|| vec![0u8; tiles_x * tiles_y]);

    for age_type in &image.age_types {
        indices = match age_type {
            AgeType::Erosion => age_by_erosion(&indices, w, h),
            AgeType::Consolidation => {
                // Use per-pixel ages if available
                if image.pixel_ages.is_some() {
                    let (new_indices, new_pixel_ages) =
                        age_by_consolidation(&indices, &pixel_ages, w, h);
                    // Compute tile-level ages from per-pixel ages (max age in tile)
                    for ty in 0..tiles_y {
                        for tx in 0..tiles_x {
                            let tile_idx = ty * tiles_x + tx;
                            let tx0 = tx * TILE_SIZE;
                            let ty0 = ty * TILE_SIZE;
                            let mut max_age = 0u8;
                            for y in 0..TILE_SIZE {
                                for x in 0..TILE_SIZE {
                                    let age = new_pixel_ages[(ty0 + y) * w + (tx0 + x)];
                                    if age > max_age {
                                        max_age = age;
                                    }
                                }
                            }
                            age_levels[tile_idx] = max_age;
                        }
                    }
                    pixel_ages = new_pixel_ages;
                    new_indices
                } else {
                    consolidation_step_with_age(&indices, w, h, &mut age_levels)
                }
            }
            AgeType::Bleach => {
                // Convolutional bleach: 2x2 blocks with mixed/diagonal patterns become paper
                age_by_bleaching(&indices, w, h)
            }
        };
    }

    // DATA chunk: palette (48 bytes) + tiles
    let mut data_payload: Vec<u8> = Vec::new();
    // Palette: PALETTE_SIZE colors × 3 bytes RGB
    for color in &image.palette.0 {
        data_payload.extend_from_slice(color);
    }

    // Per-tile: [u16 compressed_len LE][u8 flags][compressed packed data]
    // Packed format: high nibble = index (0-15), low nibble = age (0-15)
    // 1 byte per pixel instead of 2
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let tile_indices = extract_tile_indices(&indices, w, tx, ty);
            let tile_ages = extract_tile_ages(&pixel_ages, w, tx, ty);
            // Pack index + age into one byte per pixel
            let packed = pack_tile_data(&tile_indices, &tile_ages);
            let compressed = zlib_compress(&packed)?;
            let len = compressed.len() as u16;
            data_payload.extend_from_slice(&len.to_le_bytes());
            data_payload.push(0u8); // flags
            data_payload.extend_from_slice(&compressed);
        }
    }
    write_chunk(out, CHUNK_DATA, &data_payload);
    Ok(age_levels)
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

fn extract_tile_ages(ages: &[u8], width: usize, tx: usize, ty: usize) -> Vec<u8> {
    let mut tile = Vec::with_capacity(TILE_SIZE * TILE_SIZE);
    let x_start = tx * TILE_SIZE;
    let y_start = ty * TILE_SIZE;
    for y in y_start..y_start + TILE_SIZE {
        let row_start = y * width + x_start;
        tile.extend_from_slice(&ages[row_start..row_start + TILE_SIZE]);
    }
    tile
}

/// Pack tile indices and ages into one byte per pixel.
/// High nibble (4 bits) = index (0-15), low nibble (4 bits) = age (0-15).
fn pack_tile_data(indices: &[u8], ages: &[u8]) -> Vec<u8> {
    assert_eq!(indices.len(), ages.len());
    indices
        .iter()
        .zip(ages.iter())
        .map(|(&idx, &age)| (idx << 4) | (age & 0x0F))
        .collect()
}
