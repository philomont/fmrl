use fmrl::error::FmrlError;
use fmrl::format::write_chunk;
use fmrl::{decode, encode, FmrlImage, Palette};

const NOW_MS: u64 = 1_700_000_000_000;

fn simple_image() -> FmrlImage {
    let palette = Palette::default();
    // RGB format: 3 bytes per pixel (R=index×16, G=contrast, B=age×16)
    let pixels = vec![0u8; 128 * 128 * 3];
    let mut image = FmrlImage::new(128, 128, pixels);
    image.palette = palette;
    image
}

#[test]
fn crc_corruption_detected() {
    let image = simple_image();
    let mut encoded = encode(&image, NOW_MS).expect("encode failed");

    // Find the IHDR chunk (starts at offset 8, after magic)
    // Layout: magic(8) + len(4) + name(4) + data(variable) + crc(4)
    // With 1 age type: data = width(2) + height(2) + bit_depth(1) + color_type(1) +
    //                       compression(1) + filter(1) + interlace(1) + decay_policy(1) +
    //                       age_count(1) + age_type(1) = 12 bytes
    // CRC is at offset 8 + 4 + 4 + 12 = 28
    let ihdr_data_len =
        u32::from_be_bytes([encoded[8], encoded[9], encoded[10], encoded[11]]) as usize;
    let crc_pos = 8 + 4 + 4 + ihdr_data_len; // after magic + len + name + data
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
    data[0] = b'X';
    data[1] = b'X';
    data[2] = b'X';
    data[3] = b'X';
    let result = decode(&data);
    assert!(result.is_err());
    match result.unwrap_err() {
        FmrlError::InvalidMagic(_) => {}
        e => panic!("expected InvalidMagic, got {:?}", e),
    }
}
