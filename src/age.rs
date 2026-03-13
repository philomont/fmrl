use crate::format::TILE_SIZE;

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

/// Gradual 16-step consolidation using hierarchical per-pixel aging.
///
/// Each pixel ages through 16 levels (0-15) with gradual fading:
/// - Ages 0-3: Fade through grayscale at current resolution
/// - Ages 4-7: 2×2 consolidation + fade through grayscale
/// - Ages 8-11: 4×4 consolidation + fade through grayscale
/// - Ages 12-14: 8×8 consolidation + fade through grayscale
/// - Age 15: 16×16 consolidation → paper
///
/// The grayscale fade uses palette indices: 1→4→8→12→0 (paper)
/// This creates 16 gradual visual steps instead of 5 abrupt jumps.
pub fn consolidation_step_with_pixel_ages(
    indices: &[u8],
    pixel_ages: &[u8],
    width: usize,
    height: usize,
) -> (Vec<u8>, Vec<u8>) {
    let mut result = indices.to_vec();
    let mut new_ages = pixel_ages.to_vec();

    // Define fade levels within each consolidation stage
    // Age ranges: 0-3 (full res), 4-7 (2x2), 8-11 (4x4), 12-14 (8x8), 15 (16x16)
    const FADE_LEVELS: [u8; 4] = [1, 4, 8, 12]; // Palette indices to fade through

    // First pass: fade colors based on age within current consolidation level
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let age = pixel_ages[idx];
            let current_idx = indices[idx];

            if current_idx == PAPER_INDEX {
                continue; // Paper stays paper
            }

            // Determine consolidation level and fade sub-step
            let (consolidation_level, fade_step) = if age < 4 {
                (0, age as usize) // Full resolution, fade step 0-3
            } else if age < 8 {
                (1, (age - 4) as usize) // 2x2 consolidated, fade step 0-3
            } else if age < 12 {
                (2, (age - 8) as usize) // 4x4 consolidated, fade step 0-3
            } else if age < 15 {
                (3, (age - 12) as usize) // 8x8 consolidated, fade step 0-3
            } else {
                (4, 0) // 16x16 → paper
            };

            // Apply fade: reduce color index toward paper
            // Map any non-paper index to fade through FADE_LEVELS
            if current_idx > 0 && fade_step < 4 {
                let target_idx = FADE_LEVELS[fade_step];
                if current_idx > target_idx {
                    // Fade toward lighter grayscale
                    result[idx] = target_idx;
                }
            }

            // Track the consolidation level in new_ages
            new_ages[idx] = age;
        }
    }

    // Second pass: apply consolidation based on age thresholds
    // - Age 4+: apply 2×2 consolidation
    // - Age 8+: apply 4×4 consolidation
    // - Age 12+: apply 8×8 consolidation
    // - Age 15: apply 16×16 consolidation → paper

    let mut should_consolidate_2x2 = vec![false; (width / 2) * (height / 2)];
    let mut should_consolidate_4x4 = vec![false; (width / 4) * (height / 4)];
    let mut should_consolidate_8x8 = vec![false; (width / 8) * (height / 8)];
    let mut should_consolidate_16x16 = vec![false; (width / 16) * (height / 16)];

    // Check 2×2 blocks: consolidate pixels with age >= 4
    for y in (0..height).step_by(2) {
        for x in (0..width).step_by(2) {
            let y_end = (y + 2).min(height);
            let x_end = (x + 2).min(width);

            let min_pixel_age = min_age_in_region(
                &pixel_ages, width, x, y, x_end - x, y_end - y
            );

            // Consolidate if at least one pixel is age >= 4
            if min_pixel_age >= 4 {
                should_consolidate_2x2[(y / 2) * (width / 2) + (x / 2)] = true;
            }
        }
    }

    // Check 4×4 blocks: consolidate pixels with age >= 8
    for y in (0..height).step_by(4) {
        for x in (0..width).step_by(4) {
            let y_end = (y + 4).min(height);
            let x_end = (x + 4).min(width);

            let min_pixel_age = min_age_in_region(
                &pixel_ages, width, x, y, x_end - x, y_end - y
            );

            if min_pixel_age >= 8 {
                should_consolidate_4x4[(y / 4) * (width / 4) + (x / 4)] = true;
            }
        }
    }

    // Check 8×8 blocks: consolidate pixels with age >= 12
    for y in (0..height).step_by(8) {
        for x in (0..width).step_by(8) {
            let y_end = (y + 8).min(height);
            let x_end = (x + 8).min(width);

            let min_pixel_age = min_age_in_region(
                &pixel_ages, width, x, y, x_end - x, y_end - y
            );

            if min_pixel_age >= 12 {
                should_consolidate_8x8[(y / 8) * (width / 8) + (x / 8)] = true;
            }
        }
    }

    // Check 16×16 blocks: consolidate pixels with age >= 15
    for y in (0..height).step_by(16) {
        for x in (0..width).step_by(16) {
            let y_end = (y + 16).min(height);
            let x_end = (x + 16).min(width);

            let min_pixel_age = min_age_in_region(
                &pixel_ages, width, x, y, x_end - x, y_end - y
            );

            if min_pixel_age >= 15 {
                should_consolidate_16x16[(y / 16) * (width / 16) + (x / 16)] = true;
            }
        }
    }

    // Apply consolidation in order from smallest to largest
    // This preserves the hierarchical nature while allowing gradual fading

    // Create mutable copies for consolidation
    let mut final_result = result.clone();

    // Apply 2×2 consolidation (age >= 4)
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
                        // Keep the age for continued fading
                        if new_ages[idx] < 4 {
                            new_ages[idx] = 4; // Bump to 2x2 consolidation level
                        }
                    }
                }
            }
        }
    }

    // Apply 4×4 consolidation (age >= 8)
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
                        if new_ages[idx] < 8 {
                            new_ages[idx] = 8; // Bump to 4x4 level
                        }
                    }
                }
            }
        }
    }

    // Apply 8×8 consolidation (age >= 12)
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
                        if new_ages[idx] < 12 {
                            new_ages[idx] = 12; // Bump to 8x8 level
                        }
                    }
                }
            }
        }
    }

    // Apply 16×16 consolidation (age >= 15) → becomes paper
    for y in (0..height).step_by(16) {
        for x in (0..width).step_by(16) {
            if should_consolidate_16x16[(y / 16) * (width / 16) + (x / 16)] {
                let y_end = (y + 16).min(height);
                let x_end = (x + 16).min(width);

                for by in y..y_end {
                    for bx in x..x_end {
                        let idx = by * width + bx;
                        final_result[idx] = PAPER_INDEX;
                        new_ages[idx] = 0; // Reset age for paper
                    }
                }
            }
        }
    }

    (final_result, new_ages)
}

/// Legacy function kept for backward compatibility.
/// Maps the old 5-step consolidation to the new 16-step system.
pub fn consolidation_step_with_pixel_ages_legacy(
    indices: &[u8],
    pixel_ages: &[u8],
    width: usize,
    height: usize,
) -> (Vec<u8>, Vec<u8>) {
    consolidation_step_with_pixel_ages(indices, pixel_ages, width, height)
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

/// Apply one convolutional bleach step using SLIDING 2×2 windows.
///
/// Unlike tile-based processing, this slides a 2×2 window across EVERY pixel
/// position, checking all overlapping windows. Any pixel that is part of
/// at least one "bleachable" window becomes paper.
///
/// A 2×2 window is "bleachable" if:
/// - 3 or 4 different indices in window → becomes paper
/// - 2 indices with unequal counts → becomes paper
/// - 2 indices with equal counts (2 each) AND diagonal pattern → becomes paper
/// - All other arrangements remain unchanged
///
/// Examples that become paper:
/// - [[1,2],[3,4]] (4 different indices)
/// - [[1,2],[3,1]] (3 different indices)
/// - [[1,1],[2,2]] (2 indices, equal, not diagonal - becomes paper!)
/// - [[1,2],[2,1]] (2 indices, equal, diagonal)
///
/// Examples that remain unchanged:
/// - [[1,1],[1,1]] (all same)
/// - [[1,1],[1,2]] (3 same, 1 different)
pub fn bleach_step(indices: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut result = indices.to_vec();

    // Track which pixels should be bleached (part of any bleachable window)
    let mut bleach_mask = vec![false; width * height];

    // Slide 2×2 window across every pixel position
    // Window covers positions (x,y), (x+1,y), (x,y+1), (x+1,y+1)
    for y in 0..height.saturating_sub(1) {
        for x in 0..width.saturating_sub(1) {
            // Collect the 4 pixels in this window
            let block = [
                indices[y * width + x],
                indices[y * width + (x + 1)],
                indices[(y + 1) * width + x],
                indices[(y + 1) * width + (x + 1)],
            ];

            let should_bleach = is_block_bleachable(&block);

            if should_bleach {
                // Mark all 4 pixels in this window for bleaching
                bleach_mask[y * width + x] = true;
                bleach_mask[y * width + (x + 1)] = true;
                bleach_mask[(y + 1) * width + x] = true;
                bleach_mask[(y + 1) * width + (x + 1)] = true;
            }
        }
    }

    // Apply bleaching to all marked pixels
    for y in 0..height {
        for x in 0..width {
            if bleach_mask[y * width + x] {
                result[y * width + x] = PAPER_INDEX;
            }
        }
    }

    result
}

/// Check if a 2×2 block should be bleached according to the rules.
///
/// Bleach cases:
/// 1. Information rich/noisy: 3 or 4 different indices
///    Examples: [[0,1],[2,0]], [[0,1],[2,3]], [[1,2],[3,4]]
///
/// 2. Imbalanced: 3 of one index, 1 of another
///    Examples: [[0,1],[1,1]], [[0,0],[0,1]], [[2,2],[2,3]]
///
/// 3. Anti-diagonal: 2 of each in [[a,b],[b,a]] pattern
///    Examples: [[0,1],[1,0]], [[2,3],[3,2]]
fn is_block_bleachable(block: &[u8; 4]) -> bool {
    // Count occurrences of each index (0-15, including paper/0)
    let mut counts = [0u8; 16];
    for &idx in block {
        if (idx as usize) < 16 {
            counts[idx as usize] += 1;
        }
    }

    // Count how many different indices are present
    let unique_indices: Vec<u8> = (0..16).filter(|&i| counts[i] > 0).map(|i| i as u8).collect();
    let unique_count = unique_indices.len();

    // Case 1: 3 or 4 different indices -> bleach (information rich/noisy)
    // Examples: [[0,1],[2,0]], [[0,1],[2,3]], [[1,2],[3,4]]
    if unique_count >= 3 {
        return true;
    }

    // Case 2: Exactly 2 different indices
    if unique_count == 2 {
        let idx1 = unique_indices[0] as usize;
        let idx2 = unique_indices[1] as usize;
        let count1 = counts[idx1];
        let count2 = counts[idx2];

        // Imbalanced: 3 of one, 1 of other -> bleach
        // Examples: [[0,1],[1,1]], [[0,0],[0,1]]
        if (count1 == 3 && count2 == 1) || (count1 == 1 && count2 == 3) {
            return true;
        }

        // Balanced: 2 of each -> check for anti-diagonal
        // Anti-diagonal: [[a,b],[b,a]] where block[0] == block[3] and block[1] == block[2]
        // Examples: [[0,1],[1,0]], [[2,3],[3,2]]
        if count1 == 2 && count2 == 2 {
            let is_antidiagonal = block[0] == block[3] && block[1] == block[2];
            if is_antidiagonal {
                return true;
            }
        }
    }

    // Case 3: 0 or 1 unique indices -> don't bleach (uniform)
    false
}

