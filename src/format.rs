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

/// IHDR payload length: width(2) + height(2) + bit_depth(1) + color_type(1) +
/// compression(1) + filter(1) + interlace(1) + decay_policy(1) = 10 bytes
pub const IHDR_LEN: usize = 10;

/// AGE entry: tx(2) + ty(2) + last_view(8) + fade_level(1) + noise_seed(4) +
/// edge_damage(1) + reserved(2) = 20 bytes... wait:
/// tx(2) + ty(2) + last_view(8) + fade_level(1) + noise_seed(4) + edge_damage(1) + reserved(2) = 20
/// Plan says 22 bytes. Let me count: tx(2)+ty(2)+last_view(8)+fade_level(1)+noise_seed(4)+edge_damage(1)+reserved(2) = 20
/// Adding pad(2) to reach 22:
/// tx(u16)=2, ty(u16)=2, last_view(u64)=8, fade_level(u8)=1, noise_seed([u8;4])=4, edge_damage(u8)=1, reserved(u16)=2, _pad(u16)=2 = 22
pub const AGE_ENTRY_BYTES: usize = 22;

#[derive(Debug, Clone)]
pub struct IhdrChunk {
    pub width: u16,
    pub height: u16,
    pub bit_depth: u8,
    pub color_type: u8,
    pub compression: u8,
    pub filter: u8,
    pub interlace: u8,
    pub decay_policy: u8,
}

impl IhdrChunk {
    pub fn new(width: u16, height: u16, decay_policy: u8) -> Self {
        IhdrChunk {
            width,
            height,
            bit_depth: 8,
            color_type: 3, // indexed color
            compression: 0,
            filter: 0,
            interlace: 0,
            decay_policy,
        }
    }

    pub fn to_bytes(&self) -> [u8; IHDR_LEN] {
        let mut buf = [0u8; IHDR_LEN];
        buf[0..2].copy_from_slice(&self.width.to_be_bytes());
        buf[2..4].copy_from_slice(&self.height.to_be_bytes());
        buf[4] = self.bit_depth;
        buf[5] = self.color_type;
        buf[6] = self.compression;
        buf[7] = self.filter;
        buf[8] = self.interlace;
        buf[9] = self.decay_policy;
        buf
    }

    pub fn from_bytes(b: &[u8]) -> Result<Self, FmrlError> {
        if b.len() < IHDR_LEN {
            return Err(FmrlError::MalformedChunk("IHDR too short"));
        }
        Ok(IhdrChunk {
            width: u16::from_be_bytes([b[0], b[1]]),
            height: u16::from_be_bytes([b[2], b[3]]),
            bit_depth: b[4],
            color_type: b[5],
            compression: b[6],
            filter: b[7],
            interlace: b[8],
            decay_policy: b[9],
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

/// 4-color RGB palette
#[derive(Debug, Clone)]
pub struct Palette(pub [[u8; 3]; 4]);

impl Default for Palette {
    fn default() -> Self {
        Palette([
            [0, 0, 0],         // 0: ink (black)
            [230, 220, 195],   // 1: aged paper
            [180, 30, 30],     // 2: crimson accent
            [255, 255, 255],   // 3: white
        ])
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

/// Pack palette indices into 4-bit nibbles (high=even pixel, low=odd pixel).
/// Input length must be even; output is input.len()/2.
pub fn pack_nibbles(indices: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(indices.len().div_ceil(2));
    let mut i = 0;
    while i < indices.len() {
        let hi = if i < indices.len() { indices[i] & 0x0F } else { 0 };
        let lo = if i + 1 < indices.len() { indices[i + 1] & 0x0F } else { 0 };
        out.push((hi << 4) | lo);
        i += 2;
    }
    out
}

/// Unpack 4-bit nibbles back into palette indices.
pub fn unpack_nibbles(packed: &[u8], pixel_count: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(pixel_count);
    for &byte in packed {
        out.push((byte >> 4) & 0x0F);
        out.push(byte & 0x0F);
        if out.len() >= pixel_count {
            break;
        }
    }
    out.truncate(pixel_count);
    out
}
