use fmrl::{decode, encode, ColorMode, FmrlImage, Palette};

const NOW_MS: u64 = 1_700_000_000_000;

/// Create RGB pixels from palette index
/// Format: R = index × 16, G = contrast (0x00 for paper, 0xFF otherwise), B = age × 16
fn rgb_from_index(index: u8, age: u8) -> [u8; 3] {
    [
        index << 4,                           // R = index × 16
        if index == 0 { 0x00 } else { 0xFF }, // G = contrast
        age << 4,                             // B = age × 16
    ]
}

fn solid_image(color_idx: u8, width: u16, height: u16) -> FmrlImage {
    let palette = Palette::default();
    let rgb = rgb_from_index(color_idx, 0);
    let pixels: Vec<u8> = (0..width as usize * height as usize)
        .flat_map(|_| rgb)
        .collect();
    let mut image = FmrlImage::new(width, height, pixels);
    image.palette = palette;
    image
}

fn checkerboard_image(width: u16, height: u16) -> FmrlImage {
    let palette = Palette::default();
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 3);
    for y in 0..height as usize {
        for x in 0..width as usize {
            // v0.4+: 0=paper, 1=ink (black)
            // Checkerboard: even positions get ink (1), odd get paper (0)
            let is_ink = (x + y) % 2 == 0;
            if is_ink {
                pixels.extend_from_slice(&rgb_from_index(1, 0)); // ink
            } else {
                pixels.extend_from_slice(&rgb_from_index(0, 0)); // paper
            }
        }
    }
    let mut image = FmrlImage::new(width, height, pixels);
    image.palette = palette;
    image
}

#[test]
fn solid_roundtrip() {
    // Index 1 is ink (darkest) in v0.4+ format
    let image = solid_image(1, 128, 128);
    let encoded = encode(&image, NOW_MS).expect("encode failed");
    let decoded = decode(&encoded).expect("decode failed");

    assert_eq!(decoded.ihdr.width, 128);
    assert_eq!(decoded.ihdr.height, 128);
    assert_eq!(decoded.tiles.len(), 1); // 1x1 tiles of 128x128
    assert_eq!(decoded.ihdr.color_mode, ColorMode::Indexed); // indexed mode

    // With aging applied during encode, edge pixels erode.
    // For a solid 128x128 tile, inner pixels remain ink (1).
    // Check that the center of each tile is still ink.
    for tile in &decoded.tiles {
        let indices = tile.indices();
        // Check center pixel (64,64) in tile
        let center_idx = 64 * 128 + 64;
        assert_eq!(indices[center_idx], 1, "center pixel should be ink");
        // Most pixels should still be ink (not all eroded)
        let ink_count = indices.iter().filter(|i| **i == 1).count();
        assert!(
            ink_count > 10000,
            "most pixels should remain ink, found {}",
            ink_count
        );
    }
}

#[test]
fn checkerboard_roundtrip() {
    let image = checkerboard_image(128, 128);
    let encoded = encode(&image, NOW_MS).expect("encode failed");
    let decoded = decode(&encoded).expect("decode failed");

    assert_eq!(decoded.tiles.len(), 1);

    // Checkerboard pattern is maximally vulnerable to erosion.
    // Every non-paper pixel has 4 paper neighbors, so after one
    // erosion step, the entire checkerboard becomes paper (all 0s).
    // This is expected behavior for FMRL aging.
    let tile0 = &decoded.tiles[0];
    let indices = tile0.indices();
    // After erosion, checkerboard should be all paper
    assert!(
        indices.iter().all(|i| *i == 0),
        "checkerboard should erode to all paper"
    );
}

#[test]
fn meta_roundtrip() {
    // Index 0 is paper (white) in v0.4+ format
    let mut image = solid_image(0, 128, 128);
    image.meta = Some(serde_json::json!({ "author": "test", "tags": ["decay", "art"] }));

    let encoded = encode(&image, NOW_MS).expect("encode failed");
    let decoded = decode(&encoded).expect("decode failed");

    let meta = decoded.meta.expect("missing meta");
    assert_eq!(meta["author"], "test");
    assert_eq!(meta["tags"][0], "decay");
}

#[test]
fn age_entries_initialized() {
    let image = solid_image(0, 128, 128);
    let encoded = encode(&image, NOW_MS).expect("encode failed");
    let decoded = decode(&encoded).expect("decode failed");

    assert_eq!(decoded.age.len(), 1);
    for entry in &decoded.age {
        assert_eq!(entry.last_view, NOW_MS);
        assert_eq!(entry.fade_level, 0);
        assert_eq!(entry.edge_damage, 0);
    }
}

#[test]
fn rgb_visualization_format() {
    // Test that decode_to_rgb produces the expected RGB format
    // Using paper (index 0) which won't be affected by aging
    let image = solid_image(0, 128, 128); // paper with age 0
    let encoded = encode(&image, NOW_MS).expect("encode failed");
    let decoded = decode(&encoded).expect("decode failed");

    let tile = &decoded.tiles[0];
    let rgb = tile.to_rgb();

    // Check first pixel
    // R = index × 16 = 0 × 16 = 0
    // G = 0x00 (paper)
    // B = age × 16 = 0
    assert_eq!(rgb[0], 0, "R should be index × 16");
    assert_eq!(rgb[1], 0x00, "G should be 0x00 for paper");
    assert_eq!(rgb[2], 0, "B should be age × 16");
}

#[test]
fn paper_pixel_format() {
    // Test paper pixel (index 0)
    let image = solid_image(0, 128, 128); // paper
    let encoded = encode(&image, NOW_MS).expect("encode failed");
    let decoded = decode(&encoded).expect("decode failed");

    let tile = &decoded.tiles[0];
    let rgb = tile.to_rgb();

    // Check first pixel
    // R = 0
    // G = 0x00 (paper)
    // B = 0
    assert_eq!(rgb[0], 0, "R should be 0 for paper");
    assert_eq!(rgb[1], 0x00, "G should be 0x00 for paper");
    assert_eq!(rgb[2], 0, "B should be 0");
}

#[test]
fn algorithm_stacking() {
    // Test that multiple algorithms can be applied
    let palette = Palette::default();
    let mut image = solid_image(1, 128, 128); // ink
    image.age_types = vec![
        fmrl::format::AgeType::Erosion,
        fmrl::format::AgeType::Consolidation,
    ];
    image.palette = palette;

    let encoded = encode(&image, NOW_MS).expect("encode failed");
    let decoded = decode(&encoded).expect("decode failed");

    // Should have 2 age types
    assert_eq!(decoded.ihdr.age_types.len(), 2);
    assert_eq!(decoded.ihdr.age_types[0], fmrl::format::AgeType::Erosion);
    assert_eq!(
        decoded.ihdr.age_types[1],
        fmrl::format::AgeType::Consolidation
    );
}
