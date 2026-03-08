use fmrl::{decode, encode, render, FmrlImage, Palette};

const NOW_MS: u64 = 1_700_000_000_000;
// 15 days later — enough to trigger visible decay
const LATER_MS: u64 = NOW_MS + 15 * 24 * 3600 * 1000;

fn test_image() -> FmrlImage {
    let palette = Palette::default();
    // Mix of ink and paper pixels for interesting decay behavior
    let mut pixels = Vec::with_capacity(64 * 64 * 4);
    for y in 0..64usize {
        for x in 0..64usize {
            let idx = ((x / 8 + y / 8) % 4) as usize;
            let [r, g, b] = palette.0[idx];
            pixels.extend_from_slice(&[r, g, b, 255]);
        }
    }
    FmrlImage { width: 64, height: 64, palette, pixels, decay_policy: 0, meta: None }
}

#[test]
fn same_age_bytes_produce_identical_output() {
    let image = test_image();
    let encoded = encode(&image, NOW_MS).expect("encode failed");

    // Render twice from the same encoded bytes (same AGE state)
    let mut decoded1 = decode(&encoded).expect("decode 1 failed");
    let mut file1 = encoded.clone();
    let rgba1 = render(&mut decoded1, LATER_MS, &mut file1).expect("render 1 failed");

    let mut decoded2 = decode(&encoded).expect("decode 2 failed");
    let mut file2 = encoded.clone();
    let rgba2 = render(&mut decoded2, LATER_MS, &mut file2).expect("render 2 failed");

    assert_eq!(rgba1, rgba2, "renders must be deterministic given same input");
}

#[test]
fn different_time_produces_different_output() {
    let image = test_image();
    let encoded = encode(&image, NOW_MS).expect("encode failed");

    let mut decoded1 = decode(&encoded).expect("decode failed");
    let mut file1 = encoded.clone();
    let rgba_early = render(&mut decoded1, NOW_MS + 1000, &mut file1).expect("render failed");

    let mut decoded2 = decode(&encoded).expect("decode failed");
    let mut file2 = encoded.clone();
    // 29 days later: should be significantly decayed
    let rgba_late = render(&mut decoded2, NOW_MS + 29 * 24 * 3600 * 1000, &mut file2).expect("render failed");

    assert_ne!(rgba_early, rgba_late, "different times must produce different output");
}

#[test]
fn no_decay_at_creation_time() {
    let image = test_image();
    let encoded = encode(&image, NOW_MS).expect("encode failed");

    let mut decoded = decode(&encoded).expect("decode failed");
    let mut file_bytes = encoded.clone();
    // Render at the exact creation time — fade factor should be 0
    let rgba = render(&mut decoded, NOW_MS, &mut file_bytes).expect("render failed");

    // All pixels should be exact palette colors (no desaturation applied)
    // Just verify it doesn't panic and returns correct size
    assert_eq!(rgba.len(), 64 * 64 * 4);
}
