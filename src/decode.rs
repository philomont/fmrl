use std::io::Read;
use std::ops::Range;

use flate2::read::ZlibDecoder;

use crate::error::FmrlError;
use crate::format::{
    AgeEntry, AGE_ENTRY_BYTES, CHUNK_AGE, CHUNK_DATA, CHUNK_IEND, CHUNK_IHDR, CHUNK_META,
    ColorMode, IhdrChunk, MAGIC, Palette, TILE_SIZE, parse_chunk, unpack_nibbles,
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
    pub fn indices(&self) -> &[u8] {
        assert_eq!(self.data.len(), TILE_SIZE * TILE_SIZE, "not indexed mode");
        &self.data
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
    if age_entries.len() != expected_tiles {
        return Err(FmrlError::MalformedChunk("AGE entry count mismatch"));
    }

    Ok(DecodedFmrl {
        ihdr,
        palette,
        tiles,
        age: age_entries,
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
    if data.len() < 12 {
        return Err(FmrlError::MalformedChunk("DATA chunk too short for palette"));
    }
    // Palette: 4 × RGB
    let mut palette_colors = [[0u8; 3]; 4];
    for i in 0..4 {
        palette_colors[i] = [data[i * 3], data[i * 3 + 1], data[i * 3 + 2]];
    }
    let palette = Palette(palette_colors);

    let w = ihdr.width as usize;
    let h = ihdr.height as usize;
    let tiles_x = w / TILE_SIZE;
    let tiles_y = h / TILE_SIZE;
    let pixel_count = TILE_SIZE * TILE_SIZE;

    let mut tiles = Vec::with_capacity(tiles_x * tiles_y);
    let mut pos = 12usize;

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

            let packed = zlib_decompress(compressed)?;
            let indices = unpack_nibbles(&packed, pixel_count);

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
    // Create a palette with paper as index 1, others default
    let mut palette = Palette::default();
    palette.0[1] = paper_color;

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
    if !data.len().is_multiple_of(AGE_ENTRY_BYTES) {
        return Err(FmrlError::MalformedChunk("AGE chunk size not a multiple of entry size"));
    }
    let count = data.len() / AGE_ENTRY_BYTES;
    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let slice = &data[i * AGE_ENTRY_BYTES..(i + 1) * AGE_ENTRY_BYTES];
        entries.push(AgeEntry::from_bytes(slice)?);
    }
    Ok(entries)
}

/// Re-serialize age entries into `file_bytes` at `age_chunk_range` and recompute CRC.
pub fn patch_age_chunk(file_bytes: &mut [u8], age_chunk_range: &Range<usize>, age: &[AgeEntry]) {
    // Serialize entries
    let mut payload = Vec::with_capacity(age.len() * AGE_ENTRY_BYTES);
    for entry in age {
        payload.extend_from_slice(&entry.to_bytes());
    }

    // Write payload into range
    file_bytes[age_chunk_range.start..age_chunk_range.end].copy_from_slice(&payload);

    // Recompute CRC: covers CHUNK_AGE name ++ payload
    let new_crc = crate::format::compute_crc(CHUNK_AGE, &payload);
    let crc_start = age_chunk_range.end;
    let crc_bytes = new_crc.to_be_bytes();
    file_bytes[crc_start..crc_start + 4].copy_from_slice(&crc_bytes);
}
