use std::fmt;

#[derive(Debug)]
pub enum FmrlError {
    InvalidMagic([u8; 4]),
    InvalidChunkCrc { chunk: [u8; 4], expected: u32, got: u32 },
    UnexpectedEof,
    MalformedChunk(&'static str),
    CompressionError(String),
    UnsupportedVersion(u8),
    IoError(std::io::Error),
}

impl fmt::Display for FmrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FmrlError::InvalidMagic(m) => write!(f, "invalid magic bytes: {:?}", m),
            FmrlError::InvalidChunkCrc { chunk, expected, got } => {
                write!(
                    f,
                    "CRC mismatch in chunk '{}': expected {:#010x}, got {:#010x}",
                    std::str::from_utf8(chunk).unwrap_or("????"),
                    expected,
                    got
                )
            }
            FmrlError::UnexpectedEof => write!(f, "unexpected end of file"),
            FmrlError::MalformedChunk(msg) => write!(f, "malformed chunk: {}", msg),
            FmrlError::CompressionError(msg) => write!(f, "compression error: {}", msg),
            FmrlError::UnsupportedVersion(v) => write!(f, "unsupported version: {}", v),
            FmrlError::IoError(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl std::error::Error for FmrlError {}

impl From<std::io::Error> for FmrlError {
    fn from(e: std::io::Error) -> Self {
        FmrlError::IoError(e)
    }
}
