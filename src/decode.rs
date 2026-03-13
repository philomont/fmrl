use std::io::Read;
use std::ops::Range;

use flate2::read::ZlibDecoder;

use crate::error::FmrlError;
use crate::format::{
    AgeEntry, AGE_ENTRY_BYTES, CHUNK_AGE, CHUNK_DATA, CHUNK_IEND, CHUNK_IHDR, CHUNK_META,
    ColorMode, IhdrChunk, MAGIC, Palette, TILE_SIZE, parse_chunk,
};

#[derive(Debug)]
pub struct DecodedFmrl {
    pub ihdr: IhdrChunk,
    pub palette: Palette,
    pub tiles: Vec<TileData>,
    pub age: Vec<AgeEntry>,
    pub meta: Option<serde_json::Value>,
    /// Byte offsets of the AGE chunk's *data payload* in the original buffer.
    /// Used for in-place mutation after rendering.
    pub age_chunk_range: Range<usize>,
}

/// Tile data - stores either palette indices (indexed mode) or RGBA pixels (RGBA mode)
#[derive(Debug, Clone)]
pub struct TileData {
    pub tx: u16,
    pub ty: u16,
    pub flags: u8,
    /// For indexed mode: palette indices (0-3), length = TILE_SIZE * TILE_SIZE
    /// For RGBA mode: raw RGBA pixels, length = TILE_SIZE * TILE_SIZE * 4
    pub data: Vec<u8>,
}

impl TileData {
    /// Check if this tile is in indexed mode
    pub fn is_indexed(&self) -> bool {
        self.data.len() == TILE_SIZE * TILE_SIZE
    }

    /// Check if this tile is in RGBA mode
    pub fn is_rgba(&self) -> bool {
        self.data.len() == TILE_SIZE * TILE_SIZE * 4
    }

    /// Get data as palette indices (panics if not indexed)
    /// Unpacks from high nibble of packed format.
    pub fn indices(&self) -> Vec<u8> {
        assert_eq!(self.data.len(), TILE_SIZE * TILE_SIZE, "not indexed mode");
        self.data.iter().map(|&packed| packed >> 4).collect()
    }

    /// Get ages from packed data (low nibble)
    pub fn pixel_ages(&self) -> Vec<u8> {
        assert_eq!(self.data.len(), TILE_SIZE * TILE_SIZE, "not indexed mode");
        self.data.iter().map(|&packed| packed & 0x0F).collect()
    }

    /// Get data as RGBA pixels (panics if not RGBA)
    pub fn rgba(&self) -> &[u8] {
        assert_eq!(self.data.len(), TILE_SIZE * TILE_SIZE * 4, "not RGBA mode");
        &self.data
    }
}

fn zlib_decompress(data: &[u8]) -> Result<Vec<u8>, FmrlError> {
    let mut decoder = ZlibDecoder::new(data);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|e| FmrlError::CompressionError(e.to_string()))?;
    Ok(out)
}

pub fn decode(data: &[u8]) -> Result<DecodedFmrl, FmrlError> {
    // Check magic
    if data.len() < MAGIC.len() {
        return Err(FmrlError::UnexpectedEof);
    }
    if data[..MAGIC.len()] != MAGIC {
        let mut magic = [0u8; 4];
        magic.copy_from_slice(&data[..4]);
        return Err(FmrlError::InvalidMagic(magic));
    }

    // First pass: collect all chunk positions without CRC verification
    // so we can locate the AGE data range before verifying
    struct RawChunk {
        name: [u8; 4],
        data_start: usize,
        data_end: usize,
    }

    let mut raw_chunks: Vec<RawChunk> = Vec::new();

    // We need the byte offsets of AGE data BEFORE we call parse_chunk (which verifies CRC).
    // So we do a lightweight scan first to collect offsets, then parse properly.
    {
        let mut scan_offset = MAGIC.len();
        while scan_offset + 8 <= data.len() {
            let length = u32::from_be_bytes([
                data[scan_offset],
                data[scan_offset + 1],
                data[scan_offset + 2],
                data[scan_offset + 3],
            ]) as usize;
            let name_start = scan_offset + 4;
            let data_start = scan_offset + 8;
            let data_end = data_start + length;
            let next = data_end + 4;

            if next > data.len() {
                return Err(FmrlError::UnexpectedEof);
            }

            let mut name = [0u8; 4];
            name.copy_from_slice(&data[name_start..name_start + 4]);
            raw_chunks.push(RawChunk { name, data_start, data_end });
            scan_offset = next;
        }
    }

    // Find AGE data range
    let age_chunk_range = raw_chunks
        .iter()
        .find(|c| &c.name == CHUNK_AGE)
        .map(|c| c.data_start..c.data_end)
        .ok_or(FmrlError::MalformedChunk("missing AGE chunk"))?;

    // Second pass: parse and validate chunks in order
    let mut ihdr: Option<IhdrChunk> = None;
    let mut palette = Palette::default();
    let mut tiles: Vec<TileData> = Vec::new();
    let mut age_entries: Vec<AgeEntry> = Vec::new();
    let mut meta: Option<serde_json::Value> = None;

    let mut offset = MAGIC.len();
    loop {
        if offset >= data.len() {
            break;
        }
        // Peek at chunk name to check for IEND
        if offset + 8 > data.len() {
            return Err(FmrlError::UnexpectedEof);
        }

        let (chunk, next_offset) = parse_chunk(data, offset)?;
        offset = next_offset;

        if chunk.name == CHUNK_IHDR {
            ihdr = Some(IhdrChunk::from_bytes(chunk.data)?);
        } else if chunk.name == CHUNK_DATA {
            let ihdr_ref = ihdr.as_ref().ok_or(FmrlError::MalformedChunk("DATA before IHDR"))?;
            let (parsed_palette, parsed_tiles) = parse_data_chunk(chunk.data, ihdr_ref)?;
            palette = parsed_palette;
            tiles = parsed_tiles;
        } else if chunk.name == CHUNK_AGE {
            age_entries = parse_age_chunk(chunk.data)?;
        } else if chunk.name == CHUNK_META {
            // zlib-compressed JSON
            let decompressed = zlib_decompress(chunk.data)?;
            let value: serde_json::Value = serde_json::from_slice(&decompressed)
                .map_err(|_| FmrlError::MalformedChunk("invalid META JSON"))?;
            meta = Some(value);
        } else if chunk.name == CHUNK_IEND {
            break;
        }
        // Unknown chunks are silently skipped
    }

    let ihdr = ihdr.ok_or(FmrlError::MalformedChunk("missing IHDR chunk"))?;
    let w = ihdr.width as usize;
    let h = ihdr.height as usize;
    let tiles_x = w / TILE_SIZE;
    let tiles_y = h / TILE_SIZE;
    let expected_tiles = tiles_x * tiles_y;

    if tiles.len() != expected_tiles {
        return Err(FmrlError::MalformedChunk("tile count mismatch"));
    }

    // Build full age array from sparse entries
    // Tiles without AGE entries get default values (age 0)
    let mut full_age = Vec::with_capacity(expected_tiles);
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            // Find entry for this tile, or use default
            let entry = age_entries.iter()
                .find(|e| e.tx as usize == tx && e.ty as usize == ty)
                .cloned()
                .unwrap_or_else(|| AgeEntry {
                    tx: tx as u16,
                    ty: ty as u16,
                    last_view: 0,
                    fade_level: 0,
                    noise_seed: [tx as u8, (tx >> 8) as u8, ty as u8, (ty >> 8) as u8],
                    edge_damage: 0,
                    reserved: 0,
                });
            full_age.push(entry);
        }
    }

    Ok(DecodedFmrl {
        ihdr,
        palette,
        tiles,
        age: full_age,
        meta,
        age_chunk_range,
    })
}

fn parse_data_chunk(data: &[u8], ihdr: &IhdrChunk) -> Result<(Palette, Vec<TileData>), FmrlError> {
    match ihdr.color_mode {
        ColorMode::Indexed => parse_data_chunk_indexed(data, ihdr),
        ColorMode::Rgba => parse_data_chunk_rgba(data, ihdr),
    }
}

fn parse_data_chunk_indexed(data: &[u8], ihdr: &IhdrChunk) -> Result<(Palette, Vec<TileData>), FmrlError> {
    use crate::format::PALETTE_SIZE;

    let palette_bytes = PALETTE_SIZE * 3;
    if data.len() < palette_bytes {
        return Err(FmrlError::MalformedChunk("DATA chunk too short for palette"));
    }
    // Palette: PALETTE_SIZE × RGB
    let mut palette_colors = [[0u8; 3]; PALETTE_SIZE];
    for i in 0..PALETTE_SIZE {
        palette_colors[i] = [data[i * 3], data[i * 3 + 1], data[i * 3 + 2]];
    }
    let palette = Palette(palette_colors);

    let w = ihdr.width as usize;
    let h = ihdr.height as usize;
    let tiles_x = w / TILE_SIZE;
    let tiles_y = h / TILE_SIZE;
    let pixel_count = TILE_SIZE * TILE_SIZE;

    let mut tiles = Vec::with_capacity(tiles_x * tiles_y);
    let mut pos = palette_bytes;

    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            if pos + 3 > data.len() {
                return Err(FmrlError::UnexpectedEof);
            }
            let comp_len = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
            let flags = data[pos + 2];
            pos += 3;

            if pos + comp_len > data.len() {
                return Err(FmrlError::UnexpectedEof);
            }
            let compressed = &data[pos..pos + comp_len];
            pos += comp_len;

            let indices = zlib_decompress(compressed)?;
            if indices.len() != pixel_count {
                return Err(FmrlError::MalformedChunk("tile indices size mismatch"));
            }

            tiles.push(TileData {
                tx: tx as u16,
                ty: ty as u16,
                flags,
                data: indices,
            });
        }
    }

    Ok((palette, tiles))
}

fn parse_data_chunk_rgba(data: &[u8], ihdr: &IhdrChunk) -> Result<(Palette, Vec<TileData>), FmrlError> {
    if data.len() < 3 {
        return Err(FmrlError::MalformedChunk("DATA chunk too short for paper color"));
    }
    // Paper color: 3 bytes RGB (used as fade target)
    let paper_color = [data[0], data[1], data[2]];
    // Create a palette with paper as index 0, others default
    let mut palette = Palette::default();
    palette.0[0] = paper_color;

    let w = ihdr.width as usize;
    let h = ihdr.height as usize;
    let tiles_x = w / TILE_SIZE;
    let tiles_y = h / TILE_SIZE;
    let rgba_count = TILE_SIZE * TILE_SIZE * 4;

    let mut tiles = Vec::with_capacity(tiles_x * tiles_y);
    let mut pos = 3usize;

    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            if pos + 3 > data.len() {
                return Err(FmrlError::UnexpectedEof);
            }
            let comp_len = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
            let flags = data[pos + 2];
            pos += 3;

            if pos + comp_len > data.len() {
                return Err(FmrlError::UnexpectedEof);
            }
            let compressed = &data[pos..pos + comp_len];
            pos += comp_len;

            let rgba = zlib_decompress(compressed)?;
            if rgba.len() != rgba_count {
                return Err(FmrlError::MalformedChunk("RGBA tile size mismatch"));
            }

            tiles.push(TileData {
                tx: tx as u16,
                ty: ty as u16,
                flags,
                data: rgba,
            });
        }
    }

    Ok((palette, tiles))
}

fn parse_age_chunk(data: &[u8]) -> Result<Vec<AgeEntry>, FmrlError> {
    // New format: compressed AGE data
    // First 2 bytes: entry count (u16 LE)
    // Rest: zlib compressed age entries (20 bytes each)
    if data.len() < 2 {
        return Err(FmrlError::MalformedChunk("AGE chunk too short"));
    }

    let count = u16::from_le_bytes([data[0], data[1]]) as usize;

    // Decompress the rest
    let compressed = &data[2..];
    let decompressed = zlib_decompress(compressed)?;

    // Each entry is 20 bytes: tx(2) + ty(2) + last_view(8) + fade_level(1) + noise_seed(4) + edge_damage(1) + reserved(2)
    const NEW_ENTRY_BYTES: usize = 20;
    if decompressed.len() != count * NEW_ENTRY_BYTES {
        return Err(FmrlError::MalformedChunk("AGE decompressed size mismatch"));
    }

    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let offset = i * NEW_ENTRY_BYTES;
        let tx = u16::from_le_bytes([decompressed[offset], decompressed[offset + 1]]);
        let ty = u16::from_le_bytes([decompressed[offset + 2], decompressed[offset + 3]]);
        let last_view = u64::from_le_bytes([
            decompressed[offset + 4], decompressed[offset + 5], decompressed[offset + 6], decompressed[offset + 7],
            decompressed[offset + 8], decompressed[offset + 9], decompressed[offset + 10], decompressed[offset + 11],
        ]);
        let fade_level = decompressed[offset + 12];
        let noise_seed = [decompressed[offset + 13], decompressed[offset + 14], decompressed[offset + 15], decompressed[offset + 16]];
        let edge_damage = decompressed[offset + 17];
        let reserved = u16::from_le_bytes([decompressed[offset + 18], decompressed[offset + 19]]);

        entries.push(AgeEntry {
            tx,
            ty,
            last_view,
            fade_level,
            noise_seed,
            edge_damage,
            reserved,
        });
    }

    Ok(entries)
}

/// Re-serialize age entries into `file_bytes` at `age_chunk_range` and recompute CRC.
/// Uses new compressed format: [count: u16 LE] + [zlib compressed entries].
pub fn patch_age_chunk(file_bytes: &mut [u8], age_chunk_range: &Range<usize>, age: &[AgeEntry]) {
    use crate::encode::zlib_compress;

    // Serialize entries in new format (20 bytes each)
    const NEW_ENTRY_BYTES: usize = 20;
    let mut age_data = Vec::with_capacity(age.len() * NEW_ENTRY_BYTES);
    for entry in age {
        age_data.extend_from_slice(&entry.tx.to_le_bytes());
        age_data.extend_from_slice(&entry.ty.to_le_bytes());
        age_data.extend_from_slice(&entry.last_view.to_le_bytes());
        age_data.push(entry.fade_level);
        age_data.extend_from_slice(&entry.noise_seed);
        age_data.push(entry.edge_damage);
        age_data.extend_from_slice(&entry.reserved.to_le_bytes());
    }

    // Compress age data
    let compressed = zlib_compress(&age_data).expect("zlib compress failed");

    // Build payload: count + compressed data
    let mut payload = Vec::with_capacity(2 + compressed.len());
    payload.extend_from_slice(&(age.len() as u16).to_le_bytes());
    payload.extend_from_slice(&compressed);

    // Update age_chunk_range to match new payload size
    // Note: This is a simplification - in practice the chunk might need resizing
    // For now, we assume the range is large enough
    let payload_len = payload.len();
    let range_len = age_chunk_range.end - age_chunk_range.start;
    if payload_len > range_len {
        // Truncate or handle error - for now just use what fits
        eprintln!("Warning: new AGE payload {} bytes > old {} bytes", payload_len, range_len);
    }

    // Write payload into range
    let write_len = payload_len.min(range_len);
    file_bytes[age_chunk_range.start..age_chunk_range.start + write_len].copy_from_slice(&payload[..write_len]);

    // Recompute CRC: covers CHUNK_AGE name ++ payload
    let new_crc = crate::format::compute_crc(CHUNK_AGE, &payload[..write_len]);
    let crc_start = age_chunk_range.start + write_len;
    let crc_bytes = new_crc.to_be_bytes();
    if crc_start + 4 <= file_bytes.len() {
        file_bytes[crc_start..crc_start + 4].copy_from_slice(&crc_bytes);
    }
}
