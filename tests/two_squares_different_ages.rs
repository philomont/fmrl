use fmrl::encode::{encode, FmrlImage};
use fmrl::format::{AgeType, Palette};

/// Test: Two squares at different ages in one image
///
/// Setup:
/// - 256x256 canvas
/// - Square A (upper right): age 2, 64x64 pixels at (128-192, 0-64)
/// - Square B (lower left): age 0, 64x64 pixels at (0-64, 128-192)
///
/// Expected behavior after 2 encoding steps:
/// - Step 1: Square A (age 2→3), Square B (age 0→1)
/// - Step 2: Square A (age 3→4, disappears), Square B (age 1→2, still visible)
///
/// This tests that consolidation properly handles mixed-age regions
#[test]
fn test_two_squares_different_ages() {
    const WIDTH: u16 = 256;
    const HEIGHT: u16 = 256;
    const SQUARE_SIZE: usize = 64;

    println!("\n=== Test: Two squares at different ages ===");

    // Create initial image with age 2 square (upper right) and age 0 square (lower left)
    let mut pixels = vec![0u8; WIDTH as usize * HEIGHT as usize * 3];

    // Square A: Age 2 in upper right (x: 128-192, y: 0-64)
    for y in 0..SQUARE_SIZE {
        for x in 128..128 + SQUARE_SIZE {
            let idx = y * WIDTH as usize + x;
            pixels[idx * 3] = 1 * 16; // R = index 1
            pixels[idx * 3 + 1] = 0xFF; // G = non-paper
            pixels[idx * 3 + 2] = 2 * 16; // B = age 2
        }
    }

    // Square B: Age 0 in lower left (x: 0-64, y: 128-192)
    for y in 128..128 + SQUARE_SIZE {
        for x in 0..SQUARE_SIZE {
            let idx = y * WIDTH as usize + x;
            pixels[idx * 3] = 1 * 16; // R = index 1
            pixels[idx * 3 + 1] = 0xFF; // G = non-paper
            pixels[idx * 3 + 2] = 0 * 16; // B = age 0
        }
    }

    // Initial state
    let mut image = FmrlImage::new(WIDTH, HEIGHT, pixels);
    image.palette = Palette::default();
    image.age_types = vec![AgeType::Consolidation];

    println!("Initial state:");
    println!("  Square A (upper right): age 2");
    println!("  Square B (lower left): age 0");

    // First encode
    println!("\n--- After 1st encode ---");
    let bytes1 = encode(&image, 0).expect("First encode failed");
    let decoded1 = fmrl::decode(&bytes1).expect("First decode failed");

    // Check ages after first encode
    let mut square_a_exists_1 = false;
    let mut square_b_exists_1 = false;
    let mut square_a_age_sum_1 = 0u32;
    let mut square_b_age_sum_1 = 0u32;
    let mut square_a_count_1 = 0u32;
    let mut square_b_count_1 = 0u32;

    for tile in &decoded1.tiles {
        let tile_ages = tile.pixel_ages();
        let tile_indices = tile.indices();

        // Check upper right tiles (Square A)
        if tile.tx >= 1 && tile.ty == 0 {
            for py in 0..fmrl::format::TILE_SIZE {
                for px in 0..fmrl::format::TILE_SIZE {
                    let global_x = tile.tx as usize * fmrl::format::TILE_SIZE + px;
                    let global_y = tile.ty as usize * fmrl::format::TILE_SIZE + py;

                    if global_x >= 128 && global_x < 192 && global_y < 64 {
                        let tile_idx = py * fmrl::format::TILE_SIZE + px;
                        if tile_indices[tile_idx] != 0 {
                            square_a_exists_1 = true;
                            square_a_age_sum_1 += tile_ages[tile_idx] as u32;
                            square_a_count_1 += 1;
                        }
                    }
                }
            }
        }

        // Check lower left tiles (Square B)
        if tile.tx == 0 && tile.ty >= 1 {
            for py in 0..fmrl::format::TILE_SIZE {
                for px in 0..fmrl::format::TILE_SIZE {
                    let global_x = tile.tx as usize * fmrl::format::TILE_SIZE + px;
                    let global_y = tile.ty as usize * fmrl::format::TILE_SIZE + py;

                    if global_x < 64 && global_y >= 128 && global_y < 192 {
                        let tile_idx = py * fmrl::format::TILE_SIZE + px;
                        if tile_indices[tile_idx] != 0 {
                            square_b_exists_1 = true;
                            square_b_age_sum_1 += tile_ages[tile_idx] as u32;
                            square_b_count_1 += 1;
                        }
                    }
                }
            }
        }
    }

    let square_a_avg_age_1 = if square_a_count_1 > 0 {
        square_a_age_sum_1 / square_a_count_1
    } else {
        0
    };
    let square_b_avg_age_1 = if square_b_count_1 > 0 {
        square_b_age_sum_1 / square_b_count_1
    } else {
        0
    };

    println!(
        "  Square A: exists={}, avg_age={}, pixel_count={}",
        square_a_exists_1, square_a_avg_age_1, square_a_count_1
    );
    println!(
        "  Square B: exists={}, avg_age={}, pixel_count={}",
        square_b_exists_1, square_b_avg_age_1, square_b_count_1
    );

    // Prepare for second encode
    let mut pixels2 = vec![0u8; WIDTH as usize * HEIGHT as usize * 3];
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

    // Second encode
    println!("\n--- After 2nd encode ---");
    let bytes2 = encode(&image2, 0).expect("Second encode failed");
    let decoded2 = fmrl::decode(&bytes2).expect("Second decode failed");

    // Check final state
    let mut square_a_exists_2 = false;
    let mut square_b_exists_2 = false;
    let mut square_a_age_sum_2 = 0u32;
    let mut square_b_age_sum_2 = 0u32;
    let mut square_a_count_2 = 0u32;
    let mut square_b_count_2 = 0u32;

    for tile in &decoded2.tiles {
        let tile_ages = tile.pixel_ages();
        let tile_indices = tile.indices();

        // Check upper right tiles (Square A)
        if tile.tx >= 1 && tile.ty == 0 {
            for py in 0..fmrl::format::TILE_SIZE {
                for px in 0..fmrl::format::TILE_SIZE {
                    let global_x = tile.tx as usize * fmrl::format::TILE_SIZE + px;
                    let global_y = tile.ty as usize * fmrl::format::TILE_SIZE + py;

                    if global_x >= 128 && global_x < 192 && global_y < 64 {
                        let tile_idx = py * fmrl::format::TILE_SIZE + px;
                        if tile_indices[tile_idx] != 0 {
                            square_a_exists_2 = true;
                            square_a_age_sum_2 += tile_ages[tile_idx] as u32;
                            square_a_count_2 += 1;
                        }
                    }
                }
            }
        }

        // Check lower left tiles (Square B)
        if tile.tx == 0 && tile.ty >= 1 {
            for py in 0..fmrl::format::TILE_SIZE {
                for px in 0..fmrl::format::TILE_SIZE {
                    let global_x = tile.tx as usize * fmrl::format::TILE_SIZE + px;
                    let global_y = tile.ty as usize * fmrl::format::TILE_SIZE + py;

                    if global_x < 64 && global_y >= 128 && global_y < 192 {
                        let tile_idx = py * fmrl::format::TILE_SIZE + px;
                        if tile_indices[tile_idx] != 0 {
                            square_b_exists_2 = true;
                            square_b_age_sum_2 += tile_ages[tile_idx] as u32;
                            square_b_count_2 += 1;
                        }
                    }
                }
            }
        }
    }

    let square_a_avg_age_2 = if square_a_count_2 > 0 {
        square_a_age_sum_2 / square_a_count_2
    } else {
        0
    };
    let square_b_avg_age_2 = if square_b_count_2 > 0 {
        square_b_age_sum_2 / square_b_count_2
    } else {
        0
    };

    println!(
        "  Square A: exists={}, avg_age={}, pixel_count={}",
        square_a_exists_2, square_a_avg_age_2, square_a_count_2
    );
    println!(
        "  Square B: exists={}, avg_age={}, pixel_count={}",
        square_b_exists_2, square_b_avg_age_2, square_b_count_2
    );

    println!("\n=== Verification ===");
    println!("Expected after 2 encodes:");
    println!("  Square A (started age 2): should disappear (age 4+)");
    println!("  Square B (started age 0): should exist at age 2");
    println!("\nActual:");
    println!("  Square A disappeared: {}", !square_a_exists_2);
    println!("  Square B still exists: {}", square_b_exists_2);

    // Final assertions
    assert!(
        !square_a_exists_2,
        "Square A (started age 2) should have disappeared after 2 encodes"
    );
    assert!(
        square_b_exists_2,
        "Square B (started age 0) should still exist after 2 encodes"
    );
    assert!(
        square_b_avg_age_2 >= 1 && square_b_avg_age_2 <= 3,
        "Square B should be at age 1-3, got {}",
        square_b_avg_age_2
    );

    println!("\n✓ Test passed!");
}
