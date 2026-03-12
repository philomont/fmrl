use fmrl::{decode, encode, ColorMode, FmrlImage, Palette};

const NOW_MS: u64 = 1_700_000_000_000;

fn solid_image(color_idx: u8, width: u16, height: u16) -> FmrlImage {
    let palette = Palette::default();
    let [r, g, b] = palette.0[color_idx as usize];
    let pixels: Vec<u8> = (0..width as usize * height as usize)
        .flat_map(|_| [r, g, b, 255u8])
        .collect();
    let mut image = FmrlImage::new(width, height, pixels);
    image.palette = palette;
    image
}

fn checkerboard_image(width: u16, height: u16) -> FmrlImage {
    let palette = Palette::default();
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height as usize {
        for x in 0..width as usize {
            // v0.4+: 0=paper (white), 1=ink (black)
            // Checkerboard: even positions get ink (1), odd get paper (0)
            let idx = if (x + y) % 2 == 0 { 1 } else { 0 };
            let [r, g, b] = palette.0[idx];
            pixels.extend_from_slice(&[r, g, b, 255]);
        }
    }
    let mut image = FmrlImage::new(width, height, pixels);
    image.palette = palette;
    image
}

#[test]
fn solid_roundtrip() {
    // Index 1 is ink (darkest) in v0.4+ format
    let image = solid_image(1, 64, 64);
    let encoded = encode(&image, NOW_MS).expect("encode failed");
    let decoded = decode(&encoded).expect("decode failed");

    assert_eq!(decoded.ihdr.width, 64);
    assert_eq!(decoded.ihdr.height, 64);
    assert_eq!(decoded.tiles.len(), 4); // 2x2 tiles of 32x32
    assert_eq!(decoded.ihdr.color_mode, ColorMode::Indexed); // indexed mode

    // All indices should be 1 (ink) in v0.4+ format
    for tile in &decoded.tiles {
        assert!(tile.indices().iter().all(|&i| i == 1), "expected all ink pixels");
    }
}

#[test]
fn checkerboard_roundtrip() {
    let image = checkerboard_image(64, 64);
    let encoded = encode(&image, NOW_MS).expect("encode failed");
    let decoded = decode(&encoded).expect("decode failed");

    assert_eq!(decoded.tiles.len(), 4);

    // Check first tile: alternating 0 (paper) and 1 (ink) in v0.4+ format
    let tile0 = &decoded.tiles[0];
    let indices = tile0.indices();
    for (i, &idx) in indices.iter().enumerate() {
        let x = i % 32;
        let y = i / 32;
        // Even positions should be ink (1), odd should be paper (0)
        let expected = if (x + y) % 2 == 0 { 1 } else { 0 };
        assert_eq!(idx, expected, "pixel mismatch at ({}, {})", x, y);
    }
}

#[test]
fn meta_roundtrip() {
    // Index 0 is paper (white) in v0.4+ format
    let mut image = solid_image(0, 64, 64);
    image.meta = Some(serde_json::json!({ "author": "test", "tags": ["decay", "art"] }));

    let encoded = encode(&image, NOW_MS).expect("encode failed");
    let decoded = decode(&encoded).expect("decode failed");

    let meta = decoded.meta.expect("missing meta");
    assert_eq!(meta["author"], "test");
    assert_eq!(meta["tags"][0], "decay");
}

#[test]
fn age_entries_initialized() {
    let image = solid_image(0, 64, 64);
    let encoded = encode(&image, NOW_MS).expect("encode failed");
    let decoded = decode(&encoded).expect("decode failed");

    assert_eq!(decoded.age.len(), 4);
    for entry in &decoded.age {
        assert_eq!(entry.last_view, NOW_MS);
        assert_eq!(entry.fade_level, 0);
        assert_eq!(entry.edge_damage, 0);
    }
}

// ─── RGBA Mode Tests ───────────────────────────────────────────────────────

fn rgba_image(width: u16, height: u16) -> FmrlImage {
    // Create a gradient image with full RGBA
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height as usize {
        for x in 0..width as usize {
            let r = ((x * 255) / width as usize) as u8;
            let g = ((y * 255) / height as usize) as u8;
            let b = 128u8;
            let a = 255u8;
            pixels.extend_from_slice(&[r, g, b, a]);
        }
    }
    FmrlImage::new_rgba(width, height, pixels)
}

#[test]
fn rgba_roundtrip() {
    let image = rgba_image(64, 64);
    let encoded = encode(&image, NOW_MS).expect("encode failed");
    let decoded = decode(&encoded).expect("decode failed");

    assert_eq!(decoded.ihdr.width, 64);
    assert_eq!(decoded.ihdr.height, 64);
    assert_eq!(decoded.tiles.len(), 4);
    assert_eq!(decoded.ihdr.color_mode, ColorMode::Rgba); // RGBA mode

    // Check that tiles contain RGBA data
    for tile in &decoded.tiles {
        assert!(tile.is_rgba(), "tile should be RGBA mode");
        assert_eq!(tile.rgba().len(), 32 * 32 * 4);
    }
}

#[test]
fn rgba_preserves_colors() {
    let width = 64u16;
    let height = 64u16;
    let image = rgba_image(width, height);
    let encoded = encode(&image, NOW_MS).expect("encode failed");
    let decoded = decode(&encoded).expect("decode failed");

    // Check first pixel of first tile
    let tile = &decoded.tiles[0];
    let rgba = tile.rgba();
    let expected_r = 0u8; // x=0
    let expected_g = 0u8; // y=0
    assert_eq!(rgba[0], expected_r);
    assert_eq!(rgba[1], expected_g);
    assert_eq!(rgba[2], 128);
    assert_eq!(rgba[3], 255);
}

#[test]
fn indexed_vs_rgba_produces_different_files() {
    // Same visual content but different encoding
    let palette = Palette::default();
    let mut indexed_pixels = Vec::with_capacity(64 * 64 * 4);
    for _ in 0..64 * 64 {
        let [r, g, b] = palette.0[1]; // all ink (index 1 in v0.4+)
        indexed_pixels.extend_from_slice(&[r, g, b, 255]);
    }
    let mut indexed_image = FmrlImage::new(64, 64, indexed_pixels.clone());
    indexed_image.palette = palette.clone();

    let mut rgba_image = FmrlImage::new_rgba(64, 64, indexed_pixels);
    rgba_image.palette = palette;

    let indexed_encoded = encode(&indexed_image, NOW_MS).expect("encode failed");
    let rgba_encoded = encode(&rgba_image, NOW_MS).expect("encode failed");

    // Decode and verify color modes
    let indexed_decoded = decode(&indexed_encoded).expect("decode failed");
    let rgba_decoded = decode(&rgba_encoded).expect("decode failed");

    assert_eq!(indexed_decoded.ihdr.color_mode, ColorMode::Indexed);
    assert_eq!(rgba_decoded.ihdr.color_mode, ColorMode::Rgba);
}
