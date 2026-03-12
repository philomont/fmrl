/// Number of consecutive non-paper pixels in a row or column that will be
/// erased.  Runs of this length or shorter become paper.
const RUN_THRESHOLD: usize = 2;

/// Paper index - pixels with this index don't age and are the erosion target.
const PAPER_INDEX: u8 = 0;

/// Apply one aging step to a flat, row-major array of palette indices.
///
/// `indices` is `width × height` bytes; each byte is a palette index where
/// `0` means paper.  Returns a new Vec with the aged indices.
///
/// Two passes, both of which can only convert pixels *to* paper:
///
/// 1. **Morphological erosion** — any non-paper pixel with ≥ 4 paper
///    8-neighbours (out-of-bounds treated as paper) becomes paper.
///    This gentler threshold ensures gradual erosion from edges while
///    preserving the core of larger ink regions for multiple save/load cycles.
///
/// 2. **Short-run elimination** — scan every row then every column.  Any
///    non-paper run whose length ≤ `RUN_THRESHOLD` becomes paper.  This
///    collapses isolated dots and thin bridges that erosion leaves behind,
///    and ensures the surviving non-paper regions form wide, regular blocks
///    that zlib can compress efficiently.
///
/// Because both passes only convert to paper, the information content of the
/// image is strictly non-increasing.  Repeated application eventually renders
/// all pixels paper (all indices equal 0).
pub fn age_step(indices: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut next = indices.to_vec();
    let w = width;
    let h = height;

    // ── Pass 1: morphological erosion ──────────────────────────────────────
    for y in 0..h {
        for x in 0..w {
            if indices[y * w + x] == PAPER_INDEX {
                continue; // paper is immune
            }
            let mut paper_count: u32 = 0;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    let is_paper = nx < 0
                        || nx >= w as i32
                        || ny < 0
                        || ny >= h as i32
                        || indices[ny as usize * w + nx as usize] == PAPER_INDEX;
                    if is_paper {
                        paper_count += 1;
                    }
                }
            }
            // Require 4+ paper neighbors (was 3) for gentler aging
            if paper_count >= 4 {
                next[y * w + x] = PAPER_INDEX;
            }
        }
    }

    // ── Pass 2a: short-run elimination — rows ──────────────────────────────
    // ── Pass 2a: short-run elimination — rows ──────────────────────────────
    for y in 0..h {
        let mut x = 0;
        while x < w {
            if next[y * w + x] != PAPER_INDEX {
                let start = x;
                while x < w && next[y * w + x] != PAPER_INDEX {
                    x += 1;
                }
                if x - start <= RUN_THRESHOLD {
                    for rx in start..x {
                        next[y * w + rx] = PAPER_INDEX;
                    }
                }
            } else {
                x += 1;
            }
        }
    }

    // ── Pass 2b: short-run elimination — columns ───────────────────────────
    for x in 0..w {
        let mut y = 0;
        while y < h {
            if next[y * w + x] != PAPER_INDEX {
                let start = y;
                while y < h && next[y * w + x] != PAPER_INDEX {
                    y += 1;
                }
                if y - start <= RUN_THRESHOLD {
                    for ry in start..y {
                        next[ry * w + x] = PAPER_INDEX;
                    }
                }
            } else {
                y += 1;
            }
        }
    }

    next
}


/// Find the minimum non-zero index in a region.
/// Returns 0 (paper) if all pixels are paper.
fn min_index_in_region(
    indices: &[u8],
    width: usize,
    x: usize,
    y: usize,
    block_w: usize,
    block_h: usize,
) -> u8 {
    let x_end = (x + block_w).min(width);
    let y_end = (y + block_h).min(indices.len() / width);

    let mut min_idx = 255u8;
    for by in y..y_end {
        for bx in x..x_end {
            let idx = indices[by * width + bx];
            if idx != 0 && idx < min_idx {
                min_idx = idx;
            }
        }
    }

    if min_idx == 255 {
        0 // All paper
    } else {
        min_idx
    }
}

/// Find the minimum age in a region.
fn min_age_in_region(
    ages: &[u8],
    width: usize,
    x: usize,
    y: usize,
    block_w: usize,
    block_h: usize,
) -> u8 {
    let x_end = (x + block_w).min(width);
    let y_end = (y + block_h).min(ages.len() / width);

    let mut min_age = 255u8;
    for by in y..y_end {
        for bx in x..x_end {
            let age = ages[by * width + bx];
            if age < min_age {
                min_age = age;
            }
        }
    }

    min_age
}

/// Apply one consolidation step using hierarchical per-pixel aging.
///
/// Each pixel has its own age (0-4). On each step, pixels advance ONE level
/// based on their current age. New drawings (age 0) age properly even when
/// drawn on top of older content.
///
/// A block consolidates when ALL pixels have age <= threshold AND at least one
/// pixel has age == threshold. This allows mixed-age blocks to consolidate
/// at the level of the newest (youngest) pixels.
///
/// - Age 0 pixels → 2×2 blocks consolidate (become age 1)
/// - Age 1 pixels → 4×4 blocks consolidate (become age 2)
/// - Age 2 pixels → 8×8 blocks consolidate (become age 3)
/// - Age 3 pixels → 16×16 blocks consolidate (become age 4)
/// - Age 4+ → paper
pub fn consolidation_step_with_pixel_ages(
    indices: &[u8],
    pixel_ages: &[u8],
    width: usize,
    height: usize,
) -> (Vec<u8>, Vec<u8>) {
    let result = indices.to_vec();
    let new_ages = pixel_ages.to_vec();

    // First pass: determine consolidation based on ORIGINAL ages
    // A block consolidates if:
    // - ALL pixels have age <= required_age (no pixel is too old)
    // - At least one pixel has age == required_age (some pixel is ready)

    let mut should_consolidate_2x2 = vec![false; (width / 2) * (height / 2)];
    let mut should_consolidate_4x4 = vec![false; (width / 4) * (height / 4)];
    let mut should_consolidate_8x8 = vec![false; (width / 8) * (height / 8)];
    let mut should_consolidate_16x16 = vec![false; (width / 16) * (height / 16)];

    // Check 2×2 blocks: consolidate if min_age == 0
    // (at least one pixel is age 0 and ready for consolidation)
    // Pixels with higher ages in the block get reset to participate
    for y in (0..height).step_by(2) {
        for x in (0..width).step_by(2) {
            let y_end = (y + 2).min(height);
            let x_end = (x + 2).min(width);

            let min_age = min_age_in_region(
                &new_ages, width, x, y, x_end - x, y_end - y
            );

            // Consolidate if any pixel is age 0 (youngest drives consolidation)
            if min_age == 0 {
                should_consolidate_2x2[(y / 2) * (width / 2) + (x / 2)] = true;
            }
        }
    }

    // Check 4×4 blocks: consolidate if min_age == 1
    // (at least one pixel is age 1 and ready)
    for y in (0..height).step_by(4) {
        for x in (0..width).step_by(4) {
            let y_end = (y + 4).min(height);
            let x_end = (x + 4).min(width);

            let min_age = min_age_in_region(
                &new_ages, width, x, y, x_end - x, y_end - y
            );

            if min_age == 1 {
                should_consolidate_4x4[(y / 4) * (width / 4) + (x / 4)] = true;
            }
        }
    }

    // Check 8×8 blocks: consolidate if min_age == 2
    for y in (0..height).step_by(8) {
        for x in (0..width).step_by(8) {
            let y_end = (y + 8).min(height);
            let x_end = (x + 8).min(width);

            let min_age = min_age_in_region(
                &new_ages, width, x, y, x_end - x, y_end - y
            );

            if min_age == 2 {
                should_consolidate_8x8[(y / 8) * (width / 8) + (x / 8)] = true;
            }
        }
    }

    // Check 16×16 blocks: consolidate if min_age == 3
    for y in (0..height).step_by(16) {
        for x in (0..width).step_by(16) {
            let y_end = (y + 16).min(height);
            let x_end = (x + 16).min(width);

            let min_age = min_age_in_region(
                &new_ages, width, x, y, x_end - x, y_end - y
            );

            if min_age == 3 {
                should_consolidate_16x16[(y / 16) * (width / 16) + (x / 16)] = true;
            }
        }
    }

    // Second pass: apply consolidation in order from largest to smallest
    let mut final_result = result.clone();
    let mut final_ages = new_ages.clone();

    // Apply 16×16 consolidation (age 3 → 4)
    for y in (0..height).step_by(16) {
        for x in (0..width).step_by(16) {
            if should_consolidate_16x16[(y / 16) * (width / 16) + (x / 16)] {
                let y_end = (y + 16).min(height);
                let x_end = (x + 16).min(width);

                let min_idx = min_index_in_region(
                    &result, width, x, y,
                    x_end - x, y_end - y
                );

                for by in y..y_end {
                    for bx in x..x_end {
                        let idx = by * width + bx;
                        final_result[idx] = min_idx;
                        final_ages[idx] = 4;
                    }
                }
            }
        }
    }

    // Apply 8×8 consolidation (age 2 → 3)
    for y in (0..height).step_by(8) {
        for x in (0..width).step_by(8) {
            if should_consolidate_8x8[(y / 8) * (width / 8) + (x / 8)] {
                let y_end = (y + 8).min(height);
                let x_end = (x + 8).min(width);

                let min_idx = min_index_in_region(
                    &result, width, x, y,
                    x_end - x, y_end - y
                );

                for by in y..y_end {
                    for bx in x..x_end {
                        let idx = by * width + bx;
                        final_result[idx] = min_idx;
                        final_ages[idx] = 3;
                    }
                }
            }
        }
    }

    // Apply 4×4 consolidation (age 1 → 2)
    for y in (0..height).step_by(4) {
        for x in (0..width).step_by(4) {
            if should_consolidate_4x4[(y / 4) * (width / 4) + (x / 4)] {
                let y_end = (y + 4).min(height);
                let x_end = (x + 4).min(width);

                let min_idx = min_index_in_region(
                    &result, width, x, y,
                    x_end - x, y_end - y
                );

                for by in y..y_end {
                    for bx in x..x_end {
                        let idx = by * width + bx;
                        final_result[idx] = min_idx;
                        final_ages[idx] = 2;
                    }
                }
            }
        }
    }

    // Apply 2×2 consolidation (age 0 → 1)
    for y in (0..height).step_by(2) {
        for x in (0..width).step_by(2) {
            if should_consolidate_2x2[(y / 2) * (width / 2) + (x / 2)] {
                let y_end = (y + 2).min(height);
                let x_end = (x + 2).min(width);

                let min_idx = min_index_in_region(
                    &result, width, x, y,
                    x_end - x, y_end - y
                );

                for by in y..y_end {
                    for bx in x..x_end {
                        let idx = by * width + bx;
                        final_result[idx] = min_idx;
                        final_ages[idx] = 1;
                    }
                }
            }
        }
    }

    // Final pass: pixels with age 4 become paper
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if final_ages[idx] >= 4 {
                final_result[idx] = PAPER_INDEX;
                final_ages[idx] = 0;
            }
        }
    }

    (final_result, final_ages)
}

/// Find the maximum age in a region.
fn max_age_in_region(
    ages: &[u8],
    width: usize,
    x: usize,
    y: usize,
    block_w: usize,
    block_h: usize,
) -> u8 {
    let x_end = (x + block_w).min(width);
    let y_end = (y + block_h).min(ages.len() / width);

    let mut max_age = 0u8;
    for by in y..y_end {
        for bx in x..x_end {
            let age = ages[by * width + bx];
            if age > max_age {
                max_age = age;
            }
        }
    }

    max_age
}

/// Legacy function for backward compatibility.
/// Uses tile-level age approximation (all pixels in tile get same age).
pub fn consolidation_step_with_age(
    indices: &[u8],
    width: usize,
    height: usize,
    age_levels: &mut [u8],
) -> Vec<u8> {
    const TILE_SIZE: usize = 32;
    let tiles_x = width / TILE_SIZE;
    let tiles_y = height / TILE_SIZE;

    // age_levels should already be sized correctly by caller
    // We use it read-only to build per-pixel ages

    // Build per-pixel age array from tile ages
    let mut per_pixel_age = vec![0u8; width * height];
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let tile_idx = ty * tiles_x + tx;
            let tile_age = age_levels[tile_idx];
            let tx0 = tx * TILE_SIZE;
            let ty0 = ty * TILE_SIZE;
            for y in 0..TILE_SIZE {
                for x in 0..TILE_SIZE {
                    per_pixel_age[(ty0 + y) * width + (tx0 + x)] = tile_age;
                }
            }
        }
    }

    // Apply consolidation with per-pixel ages
    let (result, new_per_pixel_age) = consolidation_step_with_pixel_ages(
        indices,
        &per_pixel_age,
        width,
        height,
    );

    // Compute new tile ages from per-pixel ages
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let tile_idx = ty * tiles_x + tx;
            let tx0 = tx * TILE_SIZE;
            let ty0 = ty * TILE_SIZE;

            // Tile age = max of pixel ages in tile
            let mut max_age = 0u8;
            for y in 0..TILE_SIZE {
                for x in 0..TILE_SIZE {
                    let age = new_per_pixel_age[(ty0 + y) * width + (tx0 + x)];
                    if age > max_age {
                        max_age = age;
                    }
                }
            }
            age_levels[tile_idx] = max_age;
        }
    }

    result
}

/// Simple consolidation step without AGE tracking (for backward compatibility).
/// Uses 2×2 blocks throughout the image.
pub fn consolidation_step(indices: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut dummy_age = vec![0u8; (width / 32) * (height / 32)];
    consolidation_step_with_age(indices, width, height, &mut dummy_age)
}

/// Apply one convolutional bleach step.
///
/// Uses 2×2 convolution to detect and bleach "noisy" blocks:
/// - If 3 or 4 different indices in 2×2 block → becomes paper
/// - If 2 indices with unequal counts → becomes paper
/// - If 2 indices with equal counts (2 each) AND diagonal pattern → becomes paper
/// - All other arrangements remain unchanged
///
/// Examples that become paper:
/// - [[1,2],[3,4]] (4 different indices)
/// - [[1,2],[3,1]] (3 different indices)
/// - [[1,1],[2,2]] (2 indices, equal, not diagonal)
/// - [[1,2],[2,1]] (2 indices, equal, diagonal)
///
/// Examples that remain unchanged:
/// - [[1,1],[1,1]] (all same)
/// - [[1,1],[1,2]] (3 same, 1 different - not paper)
pub fn bleach_step(indices: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut result = indices.to_vec();

    // Process 2×2 blocks
    for y in (0..height).step_by(2) {
        for x in (0..width).step_by(2) {
            // Collect the 4 pixels in this block
            let mut block = [0u8; 4];
            let mut i = 0;

            for dy in 0..2 {
                for dx in 0..2 {
                    let py = (y + dy).min(height - 1);
                    let px = (x + dx).min(width - 1);
                    block[i] = indices[py * width + px];
                    i += 1;
                }
            }

            // Count unique indices (excluding paper which is already 0)
            let mut unique = [0u8; 4];
            let mut unique_count = 0;
            for &idx in &block {
                if idx != PAPER_INDEX {
                    let mut found = false;
                    for j in 0..unique_count {
                        if unique[j] == idx {
                            found = true;
                            break;
                        }
                    }
                    if !found && unique_count < 4 {
                        unique[unique_count] = idx;
                        unique_count += 1;
                    }
                }
            }

            // Count non-paper pixels
            let non_paper_count = block.iter().filter(|&&x| x != PAPER_INDEX).count();

            // Case 1: 3 or 4 different non-paper indices → paper
            if unique_count >= 3 {
                for dy in 0..2 {
                    for dx in 0..2 {
                        let py = (y + dy).min(height - 1);
                        let px = (x + dx).min(width - 1);
                        result[py * width + px] = PAPER_INDEX;
                    }
                }
                continue;
            }

            // Case 2: Exactly 2 different indices
            if unique_count == 2 {
                // Count occurrences of each
                let idx1 = unique[0];
                let idx2 = unique[1];
                let count1 = block.iter().filter(|&&x| x == idx1).count();
                let count2 = block.iter().filter(|&&x| x == idx2).count();

                // If unequal counts → paper
                if count1 != count2 {
                    for dy in 0..2 {
                        for dx in 0..2 {
                            let py = (y + dy).min(height - 1);
                            let px = (x + dx).min(width - 1);
                            result[py * width + px] = PAPER_INDEX;
                        }
                    }
                    continue;
                }

                // Equal counts (2 each) - check if diagonal
                // Diagonal patterns: [[a,b],[b,a]]
                let is_diagonal =
                    block[0] == block[3] && block[1] == block[2] && block[0] != block[1];

                if is_diagonal {
                    for dy in 0..2 {
                        for dx in 0..2 {
                            let py = (y + dy).min(height - 1);
                            let px = (x + dx).min(width - 1);
                            result[py * width + px] = PAPER_INDEX;
                        }
                    }
                }
            }
        }
    }

    result
}

