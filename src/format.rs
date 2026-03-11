use crate::error::FmrlError;

// Magic bytes: "FMRL" followed by PNG-style sentinel bytes
pub const MAGIC: [u8; 8] = [b'F', b'M', b'R', b'L', 0x0D, 0x0A, 0x1A, 0x0A];

pub const TILE_SIZE: usize = 32;

pub const CHUNK_IHDR: &[u8; 4] = b"IHDR";
pub const CHUNK_DATA: &[u8; 4] = b"DATA";
pub const CHUNK_AGE: &[u8; 4] = b"AGE ";
pub const CHUNK_ORIG: &[u8; 4] = b"ORIG";
pub const CHUNK_META: &[u8; 4] = b"META";
pub const CHUNK_IEND: &[u8; 4] = b"IEND";

// Color types (PNG-compatible where applicable)
pub const COLOR_TYPE_INDEXED: u8 = 3; // Palette-based, 4-color
pub const COLOR_TYPE_RGBA: u8 = 6;    // Full RGBA (8-bit per channel)

// Age types for different aging algorithms
pub const AGE_TYPE_EROSION: u8 = 0;   // Morphological erosion (default)
pub const AGE_TYPE_FADE: u8 = 1;      // Simple fade-to-paper
pub const AGE_TYPE_NOISE: u8 = 2;     // Perlin noise degradation

/// IHDR payload length: width(2) + height(2) + bit_depth(1) + color_type(1) +
/// compression(1) + filter(1) + interlace(1) + decay_policy(1) + age_type(1) = 11 bytes
pub const IHDR_LEN: usize = 11;

/// AGE entry: tx(2) + ty(2) + last_view(8) + fade_level(1) + noise_seed(4) +
/// edge_damage(1) + reserved(2) + _pad(2) = 22 bytes
pub const AGE_ENTRY_BYTES: usize = 22;

/// Color mode for FMRL images
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorMode {
    /// 4-color indexed palette (classic FMRL)
    Indexed,
    /// Full 8-bit RGBA per pixel
    Rgba,
}

impl ColorMode {
    /// Convert to PNG-compatible color type value
    pub fn as_u8(self) -> u8 {
        match self {
            ColorMode::Indexed => COLOR_TYPE_INDEXED,
            ColorMode::Rgba => COLOR_TYPE_RGBA,
        }
    }

    /// Parse from PNG-compatible color type value
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            COLOR_TYPE_INDEXED => Some(ColorMode::Indexed),
            COLOR_TYPE_RGBA => Some(ColorMode::Rgba),
            _ => None,
        }
    }
}

/// Age type for different aging algorithms
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AgeType {
    /// Morphological erosion - erodes edges of strokes
    Erosion,
    /// Simple fade to paper color
    Fade,
    /// Perlin noise-based degradation
    Noise,
}

impl AgeType {
    /// Convert to u8 for storage
    pub fn as_u8(self) -> u8 {
        match self {
            AgeType::Erosion => AGE_TYPE_EROSION,
            AgeType::Fade => AGE_TYPE_FADE,
            AgeType::Noise => AGE_TYPE_NOISE,
        }
    }

    /// Parse from u8
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            AGE_TYPE_EROSION => Some(AgeType::Erosion),
            AGE_TYPE_FADE => Some(AgeType::Fade),
            AGE_TYPE_NOISE => Some(AgeType::Noise),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IhdrChunk {
    pub width: u16,
    pub height: u16,
    pub bit_depth: u8,
    pub color_mode: ColorMode,
    pub compression: u8,
    pub filter: u8,
    pub interlace: u8,
    pub decay_policy: u8,
    pub age_type: AgeType,
}

impl IhdrChunk {
    pub fn new(width: u16, height: u16, color_mode: ColorMode, decay_policy: u8, age_type: AgeType) -> Self {
        IhdrChunk {
            width,
            height,
            bit_depth: 8,
            color_mode,
            compression: 0,
            filter: 0,
            interlace: 0,
            decay_policy,
            age_type,
        }
    }

    /// Create with default indexed color mode (backward compatible)
    pub fn new_indexed(width: u16, height: u16, decay_policy: u8) -> Self {
        Self::new(width, height, ColorMode::Indexed, decay_policy, AgeType::Erosion)
    }

    pub fn to_bytes(&self) -> [u8; IHDR_LEN] {
        let mut buf = [0u8; IHDR_LEN];
        buf[0..2].copy_from_slice(&self.width.to_be_bytes());
        buf[2..4].copy_from_slice(&self.height.to_be_bytes());
        buf[4] = self.bit_depth;
        buf[5] = self.color_mode.as_u8();
        buf[6] = self.compression;
        buf[7] = self.filter;
        buf[8] = self.interlace;
        buf[9] = self.decay_policy;
        buf[10] = self.age_type.as_u8();
        buf
    }

    pub fn from_bytes(b: &[u8]) -> Result<Self, FmrlError> {
        if b.len() < IHDR_LEN {
            return Err(FmrlError::MalformedChunk("IHDR too short"));
        }
        let color_mode = ColorMode::from_u8(b[5])
            .ok_or(FmrlError::MalformedChunk("unsupported color type"))?;
        let age_type = AgeType::from_u8(b[10])
            .ok_or(FmrlError::MalformedChunk("unsupported age type"))?;
        Ok(IhdrChunk {
            width: u16::from_be_bytes([b[0], b[1]]),
            height: u16::from_be_bytes([b[2], b[3]]),
            bit_depth: b[4],
            color_mode,
            compression: b[6],
            filter: b[7],
            interlace: b[8],
            decay_policy: b[9],
            age_type,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct AgeEntry {
    pub tx: u16,
    pub ty: u16,
    pub last_view: u64,
    pub fade_level: u8,
    pub noise_seed: [u8; 4],
    pub edge_damage: u8,
    pub reserved: u16,
}

impl AgeEntry {
    pub fn to_bytes(&self) -> [u8; AGE_ENTRY_BYTES] {
        let mut buf = [0u8; AGE_ENTRY_BYTES];
        buf[0..2].copy_from_slice(&self.tx.to_le_bytes());
        buf[2..4].copy_from_slice(&self.ty.to_le_bytes());
        buf[4..12].copy_from_slice(&self.last_view.to_le_bytes());
        buf[12] = self.fade_level;
        buf[13..17].copy_from_slice(&self.noise_seed);
        buf[17] = self.edge_damage;
        buf[18..20].copy_from_slice(&self.reserved.to_le_bytes());
        // bytes 20..22 are padding, stay zero
        buf
    }

    pub fn from_bytes(b: &[u8]) -> Result<Self, FmrlError> {
        if b.len() < AGE_ENTRY_BYTES {
            return Err(FmrlError::MalformedChunk("AGE entry too short"));
        }
        let mut noise_seed = [0u8; 4];
        noise_seed.copy_from_slice(&b[13..17]);
        Ok(AgeEntry {
            tx: u16::from_le_bytes([b[0], b[1]]),
            ty: u16::from_le_bytes([b[2], b[3]]),
            last_view: u64::from_le_bytes([b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11]]),
            fade_level: b[12],
            noise_seed,
            edge_damage: b[17],
            reserved: u16::from_le_bytes([b[18], b[19]]),
        })
    }
}

/// Number of palette entries (16 for v0.4+ format)
pub const PALETTE_SIZE: usize = 16;

/// 16-color RGB palette (v0.4+ format)
/// Index 0 is paper (white, doesn't age)
/// Indices 1-15 are colors that age toward paper
#[derive(Debug, Clone)]
pub struct Palette(pub [[u8; 3]; PALETTE_SIZE]);

impl Default for Palette {
    fn default() -> Self {
        // Grayscale storage palette:
        // Index 0 = paper (white, transparent, doesn't age)
        // Index 1 = ink (black)
        // Indices 2-15 = grayscale steps toward white
        // Each step is 17 grayscale units (256/15 ≈ 17)
        let mut colors = [[0u8; 3]; PALETTE_SIZE];
        // Paper (index 0) - white, treated as transparent via alpha
        colors[0] = [255, 255, 255];
        // Color indices 1-15: black to almost-white
        for i in 1..PALETTE_SIZE {
            let gray = ((PALETTE_SIZE - i) * 17).min(255) as u8;
            colors[i] = [gray, gray, gray];
        }
        Palette(colors)
    }
}


/// Zero-copy borrowed chunk view
pub struct ChunkRef<'a> {
    pub name: &'a [u8; 4],
    pub data: &'a [u8],
}

/// Parse one chunk from `data` starting at `offset`.
/// Returns the chunk ref and the offset of the next chunk.
/// Layout: [length: u32 BE][name: 4 bytes][data: length bytes][crc: 4 bytes]
pub fn parse_chunk<'a>(data: &'a [u8], offset: usize) -> Result<(ChunkRef<'a>, usize), FmrlError> {
    if offset + 8 > data.len() {
        return Err(FmrlError::UnexpectedEof);
    }
    let length = u32::from_be_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]) as usize;
    let name_start = offset + 4;
    let data_start = offset + 8;
    let crc_start = data_start + length;
    let next_offset = crc_start + 4;

    if next_offset > data.len() {
        return Err(FmrlError::UnexpectedEof);
    }

    let name: &[u8; 4] = data[name_start..name_start+4].try_into()
        .map_err(|_| FmrlError::MalformedChunk("chunk name length"))?;
    let chunk_data = &data[data_start..crc_start];

    let stored_crc = u32::from_be_bytes([
        data[crc_start], data[crc_start+1], data[crc_start+2], data[crc_start+3],
    ]);

    // Verify CRC over name ++ data
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(name);
    hasher.update(chunk_data);
    let computed = hasher.finalize();

    if computed != stored_crc {
        return Err(FmrlError::InvalidChunkCrc {
            chunk: *name,
            expected: stored_crc,
            got: computed,
        });
    }

    Ok((ChunkRef { name, data: chunk_data }, next_offset))
}

/// Compute CRC over name ++ data (per PNG convention).
pub fn compute_crc(name: &[u8; 4], data: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(name);
    hasher.update(data);
    hasher.finalize()
}

/// Write a chunk to `out`: [length][name][data][crc]
pub fn write_chunk(out: &mut Vec<u8>, name: &[u8; 4], data: &[u8]) {
    let length = data.len() as u32;
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(name);
    out.extend_from_slice(data);
    let crc = compute_crc(name, data);
    out.extend_from_slice(&crc.to_be_bytes());
}

