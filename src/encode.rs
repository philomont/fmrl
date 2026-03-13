use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;

use crate::age::{age_step, bleach_step, consolidation_step_with_age, consolidation_step_with_pixel_ages};
use crate::error::FmrlError;
use crate::format::{
    AgeEntry, AGE_ENTRY_BYTES, CHUNK_AGE, CHUNK_DATA, CHUNK_IEND, CHUNK_IHDR, CHUNK_META,
    AgeType, ColorMode, IhdrChunk, MAGIC, Palette, TILE_SIZE, write_chunk,
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
    pub age_type: AgeType,
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
            color_mode: ColorMode::Indexed,
            palette: Palette::default(),
            pixels,
            decay_policy: 0,
            age_type: AgeType::Erosion,
            age_levels: None,
            pixel_ages: None,
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
            age_type: AgeType::Erosion,
            age_levels: None,
            pixel_ages: None,
            meta: None,
        }
    }
}

/// Quantize an RGBA pixel to a palette index using alpha + grayscale mapping.
///
/// Storage format (v0.4+, theme-agnostic):
/// Index 0 = paper (white, alpha=0) → renders as theme --paper
/// Index 1 = ink (black [0,0,0], alpha=255) → renders as theme --ink
/// Index 2-15 = grayscale steps → map to theme colors
///
/// Alpha is checked first to distinguish paper (transparent) from colors.
fn quantize_pixel(r: u8, g: u8, b: u8, a: u8) -> u8 {
    use crate::format::PALETTE_SIZE;

    // Transparent pixels are paper (index 0)
    if a < 128 {
        return 0;
    }

    // For opaque pixels, use brightness for grayscale mapping
    let brightness = (r as u16 + g as u16 + b as u16) / 3;

    // Map brightness (0-255) to color indices 1-15
    // Index 1 = black (brightness 0-16)
    // Index 15 = almost-white (brightness 240-255)
    let color_count = PALETTE_SIZE - 1; // 15 colors (indices 1-15)
    let step = 256 / color_count as u16; // ~17 per step
    let color_idx = ((brightness / step).min(color_count as u16 - 1) + 1) as u8;
    color_idx
}

/// Compress bytes with zlib (not raw DEFLATE).
/// Uses best compression for smallest file size.
pub fn zlib_compress(data: &[u8]) -> Result<Vec<u8>, FmrlError> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
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
    let ihdr = IhdrChunk::new(image.width, image.height, image.color_mode, image.decay_policy, image.age_type);
    write_chunk(&mut out, CHUNK_IHDR, &ihdr.to_bytes());

    // DATA chunk: mode-dependent
    // Get age levels from encoding (for consolidation tracking)
    let age_levels = match image.color_mode {
        ColorMode::Indexed => encode_indexed(&mut out, image, w, h, tiles_x, tiles_y)?,
        ColorMode::Rgba => {
            encode_rgba(&mut out, image, w, h, tiles_x, tiles_y)?;
            vec![0u8; tiles_x * tiles_y] // RGBA doesn't use consolidation
        }
    };

    // AGE chunk: sparse storage - only for tiles with non-paper content
    // Format: [u16 tile_count] followed by compressed [tx, ty, age] entries
    // This reduces size significantly for sparse images
    let mut age_entries: Vec<(u16, u16, u8)> = Vec::new();
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let tile_idx = ty * tiles_x + tx;
            let consolidation_level = age_levels.get(tile_idx).copied().unwrap_or(0);
            // Only store ages for tiles with content (consolidation_level > 0 or has ink)
            // For now, store all to maintain compatibility; compression will help
            age_entries.push((tx as u16, ty as u16, consolidation_level));
        }
    }

    // Build AGE payload: count + compressed entries
    // Each entry: tx(2) + ty(2) + last_view(8) + fade_level(1) + noise_seed(4) + edge_damage(1) + reserved(2) = 20 bytes
    let mut age_payload = Vec::new();
    age_payload.extend_from_slice(&(age_entries.len() as u16).to_le_bytes());

    let mut age_data = Vec::with_capacity(age_entries.len() * 20);
    for (tx, ty, level) in age_entries {
        age_data.extend_from_slice(&tx.to_le_bytes());
        age_data.extend_from_slice(&ty.to_le_bytes());
        age_data.extend_from_slice(&now_ms.to_le_bytes());
        age_data.push(level);
        age_data.extend_from_slice(&[tx as u8, (tx >> 8) as u8, ty as u8, (ty >> 8) as u8]); // noise_seed
        age_data.push(0); // edge_damage
        age_data.extend_from_slice(&[0u8, 0]); // reserved
    }

    // Compress age data
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

    // Step 2: apply one aging step based on age_type
    let mut age_levels = image.age_levels.clone().unwrap_or_else(|| vec![0u8; tiles_x * tiles_y]);

    // Get or initialize per-pixel ages
    let mut pixel_ages = image.pixel_ages.clone().unwrap_or_else(|| vec![0u8; w * h]);

    indices = match image.age_type {
        AgeType::Erosion => {
            age_step(&indices, w, h)
        }
        AgeType::Consolidation => {
            // Use per-pixel ages if available
            if image.pixel_ages.is_some() {
                let (new_indices, new_pixel_ages) = consolidation_step_with_pixel_ages(
                    &indices, &pixel_ages, w, h
                );
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
            bleach_step(&indices, w, h)
        }
    };

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
    // Store paper color for fade target (index 0 is paper in v0.4+)
    data_payload.extend_from_slice(&image.palette.0[0]);

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
    indices.iter().zip(ages.iter())
        .map(|(&idx, &age)| (idx << 4) | (age & 0x0F))
        .collect()
}
