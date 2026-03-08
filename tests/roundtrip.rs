use fmrl::{decode, encode, FmrlImage, Palette};

const NOW_MS: u64 = 1_700_000_000_000;

fn solid_image(color_idx: u8, width: u16, height: u16) -> FmrlImage {
    let palette = Palette::default();
    let [r, g, b] = palette.0[color_idx as usize];
    let pixels: Vec<u8> = (0..width as usize * height as usize)
        .flat_map(|_| [r, g, b, 255u8])
        .collect();
    FmrlImage {
        width,
        height,
        palette,
        pixels,
        decay_policy: 0,
        meta: None,
    }
}

fn checkerboard_image(width: u16, height: u16) -> FmrlImage {
    let palette = Palette::default();
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height as usize {
        for x in 0..width as usize {
            let idx = ((x + y) % 2) as usize;
            let [r, g, b] = palette.0[idx];
            pixels.extend_from_slice(&[r, g, b, 255]);
        }
    }
    FmrlImage {
        width,
        height,
        palette,
        pixels,
        decay_policy: 0,
        meta: None,
    }
}

#[test]
fn solid_roundtrip() {
    let image = solid_image(0, 64, 64);
    let encoded = encode(&image, NOW_MS).expect("encode failed");
    let decoded = decode(&encoded).expect("decode failed");

    assert_eq!(decoded.ihdr.width, 64);
    assert_eq!(decoded.ihdr.height, 64);
    assert_eq!(decoded.tiles.len(), 4); // 2x2 tiles of 32x32

    // All indices should be 0 (ink)
    for tile in &decoded.tiles {
        assert!(tile.indices.iter().all(|&i| i == 0), "expected all ink pixels");
    }
}

#[test]
fn checkerboard_roundtrip() {
    let image = checkerboard_image(64, 64);
    let encoded = encode(&image, NOW_MS).expect("encode failed");
    let decoded = decode(&encoded).expect("decode failed");

    assert_eq!(decoded.tiles.len(), 4);

    // Check first tile: alternating 0 and 1
    let tile0 = &decoded.tiles[0];
    for (i, &idx) in tile0.indices.iter().enumerate() {
        let x = i % 32;
        let y = i / 32;
        let expected = ((x + y) % 2) as u8;
        assert_eq!(idx, expected, "pixel mismatch at ({}, {})", x, y);
    }
}

#[test]
fn meta_roundtrip() {
    let mut image = solid_image(1, 64, 64);
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
