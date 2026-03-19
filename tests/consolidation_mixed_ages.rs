use fmrl::encode::{encode, FmrlImage};
use fmrl::format::{AgeType, Palette};

/// Test consolidation aging with mixed-age content
///
/// Scenario:
/// - 256x256 canvas
/// - Upper right quadrant (x: 128-192, y: 0-64): 64x64 square at age 2
/// - Lower left quadrant (x: 0-64, y: 128-192): 64x64 square at age 0
/// - Encode with consolidation twice
/// - Verify the age 0 square still exists after the age 2 square disappears
#[test]
fn test_consolidation_mixed_ages() {
    const WIDTH: u16 = 256;
    const HEIGHT: u16 = 256;
    const SQUARE_SIZE: usize = 64;

    // Create RGB buffer: R=index×16, G=contrast, B=age×16
    // Index 1 = ink (non-paper), Index 0 = paper
    // Age 2 = 32 in blue channel, Age 0 = 0 in blue channel
    let mut pixels = vec![0u8; WIDTH as usize * HEIGHT as usize * 3];

    // Draw age 2 square in upper right quadrant (x: 128-192, y: 0-64)
    for y in 0..SQUARE_SIZE {
        for x in 128..128 + SQUARE_SIZE {
            let idx = y * WIDTH as usize + x;
            pixels[idx * 3] = 1 * 16; // R = index 1 × 16
            pixels[idx * 3 + 1] = 0xFF; // G = non-paper contrast
            pixels[idx * 3 + 2] = 2 * 16; // B = age 2 × 16
        }
    }

    // Draw age 0 square in lower left quadrant (x: 0-64, y: 128-192)
    for y in 128..128 + SQUARE_SIZE {
        for x in 0..SQUARE_SIZE {
            let idx = y * WIDTH as usize + x;
            pixels[idx * 3] = 1 * 16; // R = index 1 × 16
            pixels[idx * 3 + 1] = 0xFF; // G = non-paper contrast
            pixels[idx * 3 + 2] = 0 * 16; // B = age 0 × 16
        }
    }

    // Create image with consolidation as the only aging algorithm
    let mut image = FmrlImage::new(WIDTH, HEIGHT, pixels);
    image.palette = Palette::default();
    image.age_types = vec![AgeType::Consolidation];

    // First encode (age 2 square should advance to age 3, age 0 to age 1)
    let bytes1 = encode(&image, 0).expect("First encode failed");

    // Decode to check state after first aging
    let decoded1 = fmrl::decode(&bytes1).expect("First decode failed");

    // Extract pixel ages from first decode
    let mut age2_square_exists = false;
    let mut age0_square_exists = false;

    for tile in &decoded1.tiles {
        let tile_ages = tile.pixel_ages();
        let tile_indices = tile.indices();

        // Check upper right quadrant tiles
        if tile.tx >= 1 && tile.ty == 0 {
            // Tiles in upper right
            for (i, (&idx, &age)) in tile_indices.iter().zip(tile_ages.iter()).enumerate() {
                if idx != 0 {
                    // Non-paper pixel
                    println!("Upper right pixel {}: index={}, age={}", i, idx, age);
                    if age >= 2 {
                        age2_square_exists = true;
                    }
                }
            }
        }

        // Check lower left quadrant tiles
        if tile.tx == 0 && tile.ty >= 1 {
            // Tiles in lower left
            for (i, (&idx, &age)) in tile_indices.iter().zip(tile_ages.iter()).enumerate() {
                if idx != 0 {
                    // Non-paper pixel
                    println!("Lower left pixel {}: index={}, age={}", i, idx, age);
                    if age < 2 {
                        age0_square_exists = true;
                    }
                }
            }
        }
    }

    println!("After first encode:");
    println!("  Age 2 square exists: {}", age2_square_exists);
    println!("  Age 0 square exists: {}", age0_square_exists);

    // Create second image from decoded data
    let mut pixels2 = vec![0u8; WIDTH as usize * HEIGHT as usize * 3];

    // Reconstruct pixels from decoded tiles
    for tile in &decoded1.tiles {
        let tile_ages = tile.pixel_ages();
        let tile_indices = tile.indices();

        for py in 0..fmrl::format::TILE_SIZE {
            for px in 0..fmrl::format::TILE_SIZE {
                let global_x = tile.tx as usize * fmrl::format::TILE_SIZE + px;
                let global_y = tile.ty as usize * fmrl::format::TILE_SIZE + py;

                if global_x < WIDTH as usize && global_y < HEIGHT as usize {
                    let tile_idx = py * fmrl::format::TILE_SIZE + px;
                    let pixel_idx = global_y * WIDTH as usize + global_x;

                    pixels2[pixel_idx * 3] = tile_indices[tile_idx] * 16;
                    pixels2[pixel_idx * 3 + 1] = if tile_indices[tile_idx] == 0 {
                        0x00
                    } else {
                        0xFF
                    };
                    pixels2[pixel_idx * 3 + 2] = tile_ages[tile_idx] * 16;
                }
            }
        }
    }

    let mut image2 = FmrlImage::new(WIDTH, HEIGHT, pixels2);
    image2.palette = Palette::default();
    image2.age_types = vec![AgeType::Consolidation];

    // Second encode (age 2 square should be at age 4+ and disappear, age 0 square should be at age 2)
    let bytes2 = encode(&image2, 0).expect("Second encode failed");

    // Decode to check final state
    let decoded2 = fmrl::decode(&bytes2).expect("Second decode failed");

    // Check final state
    let mut age2_square_gone = true;
    let mut age0_square_still_exists = false;

    for tile in &decoded2.tiles {
        let tile_ages = tile.pixel_ages();
        let tile_indices = tile.indices();

        // Check upper right quadrant tiles
        if tile.tx >= 1 && tile.ty == 0 {
            for (&idx, &age) in tile_indices.iter().zip(tile_ages.iter()) {
                if idx != 0 && age < 4 {
                    age2_square_gone = false;
                }
            }
        }

        // Check lower left quadrant tiles
        if tile.tx == 0 && tile.ty >= 1 {
            for (&idx, &age) in tile_indices.iter().zip(tile_ages.iter()) {
                if idx != 0 && age < 4 {
                    age0_square_still_exists = true;
                }
            }
        }
    }

    println!("After second encode:");
    println!("  Age 2 square gone: {}", age2_square_gone);
    println!("  Age 0 square still exists: {}", age0_square_still_exists);

    // Assertions
    assert!(
        age2_square_gone,
        "Age 2 square should have disappeared (reached age 4+)"
    );
    assert!(
        age0_square_still_exists,
        "Age 0 square should still exist (at age 2)"
    );
}
