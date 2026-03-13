use fmrl::{decode, encode, FmrlImage, Palette};
use fmrl::error::FmrlError;
use fmrl::format::write_chunk;

const NOW_MS: u64 = 1_700_000_000_000;

fn simple_image() -> FmrlImage {
    let palette = Palette::default();
    let pixels = vec![0u8; 128 * 128 * 4];
    let mut image = FmrlImage::new(128, 128, pixels);
    image.palette = palette;
    image
}

#[test]
fn crc_corruption_detected() {
    let image = simple_image();
    let mut encoded = encode(&image, NOW_MS).expect("encode failed");

    // Find the IHDR chunk (starts at offset 8, after magic)
    // Layout: magic(8) + len(4) + name(4) + data(10) + crc(4)
    // CRC is at offset 8 + 4 + 4 + 10 = 26
    let crc_pos = 8 + 4 + 4 + 10;
    encoded[crc_pos] ^= 0xFF; // corrupt CRC

    let result = decode(&encoded);
    assert!(result.is_err(), "expected CRC error");
    match result.unwrap_err() {
        FmrlError::InvalidChunkCrc { .. } => {}
        e => panic!("expected InvalidChunkCrc, got {:?}", e),
    }
}

#[test]
fn unknown_chunk_skipped() {
    let image = simple_image();
    let encoded = encode(&image, NOW_MS).expect("encode failed");

    // Insert an unknown "XUNK" chunk before IEND by rebuilding the file
    // Find IEND position: scan from end
    // IEND = length(4) + name(4) + no data + crc(4) = 12 bytes at end
    let iend_pos = encoded.len() - 12;

    let mut modified = encoded[..iend_pos].to_vec();
    write_chunk(&mut modified, b"XUNK", b"some unknown data");
    modified.extend_from_slice(&encoded[iend_pos..]);

    // Should parse successfully, ignoring XUNK
    let decoded = decode(&modified).expect("should tolerate unknown chunks");
    assert_eq!(decoded.ihdr.width, 128);
}

#[test]
fn unexpected_eof_detected() {
    let image = simple_image();
    let encoded = encode(&image, NOW_MS).expect("encode failed");

    // Truncate the file drastically
    let truncated = &encoded[..16]; // only magic + partial chunk
    let result = decode(truncated);
    assert!(result.is_err(), "expected UnexpectedEof");
}

#[test]
fn invalid_magic_detected() {
    let mut data = vec![0u8; 100];
    data[0] = b'X'; data[1] = b'X'; data[2] = b'X'; data[3] = b'X';
    let result = decode(&data);
    assert!(result.is_err());
    match result.unwrap_err() {
        FmrlError::InvalidMagic(_) => {}
        e => panic!("expected InvalidMagic, got {:?}", e),
    }
}
