use fmrl::{decode, encode, render, FmrlImage, Palette};
use fmrl::format::{AGE_ENTRY_BYTES, CHUNK_AGE, compute_crc};

const NOW_MS: u64 = 1_700_000_000_000;
const VIEW_MS: u64 = NOW_MS + 5 * 24 * 3600 * 1000; // 5 days later

fn simple_image() -> FmrlImage {
    let palette = Palette::default();
    let pixels = vec![0u8, 0, 0, 255].repeat(64 * 64);
    let mut image = FmrlImage::new(64, 64, pixels);
    image.palette = palette;
    image
}

#[test]
fn last_view_updated_after_render() {
    let image = simple_image();
    let encoded = encode(&image, NOW_MS).expect("encode failed");

    let mut decoded = decode(&encoded).expect("decode failed");
    let mut file_bytes = encoded.clone();

    // All age entries should start with last_view = NOW_MS
    for entry in &decoded.age {
        assert_eq!(entry.last_view, NOW_MS);
    }

    render(&mut decoded, VIEW_MS, &mut file_bytes).expect("render failed");

    // After render, all entries should have last_view updated
    for entry in &decoded.age {
        assert_eq!(entry.last_view, VIEW_MS, "last_view not updated");
    }
}

#[test]
fn fade_level_incremented_after_render() {
    let image = simple_image();
    let encoded = encode(&image, NOW_MS).expect("encode failed");

    let mut decoded = decode(&encoded).expect("decode failed");
    let mut file_bytes = encoded.clone();

    render(&mut decoded, VIEW_MS, &mut file_bytes).expect("render failed");

    for entry in &decoded.age {
        assert_eq!(entry.fade_level, 1, "fade_level should be 1 after first render");
    }
}

#[test]
fn age_crc_valid_after_mutation() {
    let image = simple_image();
    let encoded = encode(&image, NOW_MS).expect("encode failed");

    let mut decoded = decode(&encoded).expect("decode failed");
    let mut file_bytes = encoded.clone();

    render(&mut decoded, VIEW_MS, &mut file_bytes).expect("render failed");

    // Re-decode the mutated bytes — this will verify the CRC
    let decoded2 = decode(&file_bytes).expect("re-decode after mutation failed (CRC invalid)");
    assert_eq!(decoded2.ihdr.width, 64);

    // Age entries should reflect the mutation
    for entry in &decoded2.age {
        assert_eq!(entry.last_view, VIEW_MS);
        assert_eq!(entry.fade_level, 1);
    }
}

#[test]
fn noise_seed_not_modified() {
    let image = simple_image();
    let encoded = encode(&image, NOW_MS).expect("encode failed");

    let original = decode(&encoded).expect("decode failed");
    let original_seeds: Vec<[u8; 4]> = original.age.iter().map(|a| a.noise_seed).collect();

    let mut decoded = decode(&encoded).expect("decode failed");
    let mut file_bytes = encoded.clone();
    render(&mut decoded, VIEW_MS, &mut file_bytes).expect("render failed");

    for (entry, &original_seed) in decoded.age.iter().zip(original_seeds.iter()) {
        assert_eq!(entry.noise_seed, original_seed, "noise_seed must not be modified by render");
    }
}

#[test]
fn age_chunk_range_correct() {
    let image = simple_image();
    let encoded = encode(&image, NOW_MS).expect("encode failed");
    let decoded = decode(&encoded).expect("decode failed");

    let range = &decoded.age_chunk_range;
    let tile_count = 4; // 2x2 tiles
    assert_eq!(range.end - range.start, tile_count * AGE_ENTRY_BYTES);

    // Verify the CRC stored right after the range is valid
    let payload = &encoded[range.start..range.end];
    let expected_crc = compute_crc(CHUNK_AGE, payload);
    let stored_crc = u32::from_be_bytes([
        encoded[range.end],
        encoded[range.end + 1],
        encoded[range.end + 2],
        encoded[range.end + 3],
    ]);
    assert_eq!(expected_crc, stored_crc, "initial AGE CRC must be valid");
}
