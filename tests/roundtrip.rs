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
            // v0.4+: 0=paper (transparent), 1=ink (black)
            // Checkerboard: even positions get ink (1), odd get paper (0)
            let is_ink = (x + y) % 2 == 0;
            if is_ink {
                let [r, g, b] = palette.0[1]; // ink = black
                pixels.extend_from_slice(&[r, g, b, 255]); // opaque
            } else {
                // Paper is transparent (alpha < 128)
                pixels.extend_from_slice(&[255, 255, 255, 0]); // transparent white
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
        let ink_count = indices.iter().filter(|&i| *i == 1).count();
        assert!(ink_count > 10000, "most pixels should remain ink, found {}", ink_count);
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
    assert!(indices.iter().all(|&i| i == 0), "checkerboard should erode to all paper");
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
    let image = rgba_image(128, 128);
    let encoded = encode(&image, NOW_MS).expect("encode failed");
    let decoded = decode(&encoded).expect("decode failed");

    assert_eq!(decoded.ihdr.width, 128);
    assert_eq!(decoded.ihdr.height, 128);
    assert_eq!(decoded.tiles.len(), 1);
    assert_eq!(decoded.ihdr.color_mode, ColorMode::Rgba); // RGBA mode

    // Check that tiles contain RGBA data
    for tile in &decoded.tiles {
        assert!(tile.is_rgba(), "tile should be RGBA mode");
        assert_eq!(tile.rgba().len(), 128 * 128 * 4);
    }
}

#[test]
fn rgba_preserves_colors() {
    let width = 128u16;
    let height = 128u16;
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
    let mut indexed_pixels = Vec::with_capacity(128 * 128 * 4);
    for _ in 0..128 * 128 {
        let [r, g, b] = palette.0[1]; // all ink (index 1 in v0.4+)
        indexed_pixels.extend_from_slice(&[r, g, b, 255]);
    }
    let mut indexed_image = FmrlImage::new(128, 128, indexed_pixels.clone());
    indexed_image.palette = palette.clone();

    let mut rgba_image = FmrlImage::new_rgba(128, 128, indexed_pixels);
    rgba_image.palette = palette;

    let indexed_encoded = encode(&indexed_image, NOW_MS).expect("encode failed");
    let rgba_encoded = encode(&rgba_image, NOW_MS).expect("encode failed");

    // Decode and verify color modes
    let indexed_decoded = decode(&indexed_encoded).expect("decode failed");
    let rgba_decoded = decode(&rgba_encoded).expect("decode failed");

    assert_eq!(indexed_decoded.ihdr.color_mode, ColorMode::Indexed);
    assert_eq!(rgba_decoded.ihdr.color_mode, ColorMode::Rgba);
}
