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

/// Apply one consolidation step using hierarchical per-pixel aging with gradual block sizes.
///
/// Each pixel has its own age (0-6). On each step, pixels advance ONE level
/// based on their current age. Uses intermediate block sizes for more gradual aging:
///
/// - Age 0 pixels → 2×2 blocks consolidate (become age 1)
/// - Age 1 pixels → 3×3 blocks consolidate (become age 2)
/// - Age 2 pixels → 4×4 blocks consolidate (become age 3)
/// - Age 3 pixels → 6×6 blocks consolidate (become age 4)
/// - Age 4 pixels → 8×8 blocks consolidate (become age 5)
/// - Age 5 pixels → 12×12 blocks consolidate (become age 6)
/// - Age 6 pixels → 16×16 blocks consolidate (become age 7)
/// - Age 7+ → paper
pub fn consolidation_step_with_pixel_ages(
    indices: &[u8],
    pixel_ages: &[u8],
    width: usize,
    height: usize,
) -> (Vec<u8>, Vec<u8>) {
    // Debug: log input
    #[cfg(all(target_arch = "wasm32", feature = "debug-logging"))]
    {
        use web_sys::console;
        console::log_1(&format!("consolidation_step: width={}, height={}, pixel_ages[0..5]={:?}",
            width, height, &pixel_ages.get(0..5.min(pixel_ages.len())).unwrap_or(&[])).into());
    }

    let result = indices.to_vec();
    let new_ages = pixel_ages.to_vec();

    // Block sizes for each age level: 2, 3, 4, 6, 8, 12, 16
    const BLOCK_SIZES: [usize; 7] = [2, 3, 4, 6, 8, 12, 16];

    // First pass: determine consolidation based on ORIGINAL ages
    // A block consolidates if min_age == age_level (youngest pixel drives it)
    // Use ceiling division to account for partial blocks at edges
    let mut should_consolidate: Vec<Vec<bool>> = BLOCK_SIZES.iter().map(|size| {
        let blocks_x = (width + size - 1) / size;
        let blocks_y = (height + size - 1) / size;
        vec![false; blocks_x * blocks_y]
    }).collect();

    // Check blocks for each age level
    let mut total_marked = 0;
    let mut debug_sample = 0;
    for (age_level, &block_size) in BLOCK_SIZES.iter().enumerate() {
        let blocks_x = (width + block_size - 1) / block_size;
        for y in (0..height).step_by(block_size) {
            for x in (0..width).step_by(block_size) {
                let y_end = (y + block_size).min(height);
                let x_end = (x + block_size).min(width);

                let min_age = min_age_in_region(
                    &new_ages, width, x, y, x_end - x, y_end - y
                );

                // Consolidate if youngest pixel matches this age level
                if min_age == age_level as u8 {
                    let index = (y / block_size) * blocks_x + (x / block_size);
                    should_consolidate[age_level][index] = true;
                    total_marked += 1;
                }

                // Debug: log first few blocks
                #[cfg(all(target_arch = "wasm32", feature = "debug-logging"))]
                if debug_sample < 3 {
                    use web_sys::console;
                    console::log_1(&format!("  Block ({},{}) size={} min_age={} age_level={} match={}",
                        x, y, block_size, min_age, age_level, min_age == age_level as u8).into());
                    debug_sample += 1;
                }
            }
        }
    }

    #[cfg(all(target_arch = "wasm32", feature = "debug-logging"))]
    {
        use web_sys::console;
        console::log_1(&format!("First pass: {} blocks marked for consolidation", total_marked).into());
        for (i, sizes) in should_consolidate.iter().enumerate() {
            let count = sizes.iter().filter(|&&b| b).count();
            if count > 0 {
                console::log_1(&format!("  Age level {} ({}x{}): {} blocks", i, BLOCK_SIZES[i], BLOCK_SIZES[i], count).into());
            }
        }
    }

    // Second pass: apply consolidation from largest to smallest
    #[cfg(all(target_arch = "wasm32", feature = "debug-logging"))]
    {
        use web_sys::console;
        console::log_1(&format!("Starting second pass, should_consolidate len={}", should_consolidate.len()).into());
    }

    let mut final_result = result.clone();
    let mut final_ages = new_ages.clone();
    // Track which pixels were consolidated (so we don't double-increment their age)
    let mut was_consolidated = vec![false; width * height];
    let mut consolidated_count = 0;

    // Apply consolidation in reverse order (largest blocks first)
    for (age_level, &block_size) in BLOCK_SIZES.iter().enumerate().rev() {
        let next_age = (age_level + 1) as u8;

        #[cfg(all(target_arch = "wasm32", feature = "debug-logging"))]
        {
            use web_sys::console;
            console::log_1(&format!("  Age level {} block_size={} array_len={}", age_level, block_size, should_consolidate[age_level].len()).into());
        }

        let blocks_x = (width + block_size - 1) / block_size;
        for y in (0..height).step_by(block_size) {
            for x in (0..width).step_by(block_size) {
                let index = (y / block_size) * blocks_x + (x / block_size);
                if index >= should_consolidate[age_level].len() {
                    #[cfg(all(target_arch = "wasm32", feature = "debug-logging"))]
                    {
                        use web_sys::console;
                        console::log_1(&format!("    INDEX OUT OF BOUNDS: index={} array_len={}", index, should_consolidate[age_level].len()).into());
                    }
                    continue;
                }
                if should_consolidate[age_level][index] {
                    let y_end = (y + block_size).min(height);
                    let x_end = (x + block_size).min(width);

                    let min_idx = min_index_in_region(
                        &result, width, x, y,
                        x_end - x, y_end - y
                    );

                    for by in y..y_end {
                        for bx in x..x_end {
                            let idx = by * width + bx;
                            final_result[idx] = min_idx;
                            final_ages[idx] = next_age;
                            was_consolidated[idx] = true;
                            consolidated_count += 1;
                        }
                    }
                }
            }
        }
    }

    #[cfg(all(target_arch = "wasm32", feature = "debug-logging"))]
    {
        use web_sys::console;
        console::log_1(&format!("Second pass: {} pixels consolidated", consolidated_count).into());
    }

    // Age advancement: pixels that were NOT consolidated increment their age by 1
    // This ensures they can advance to larger block sizes
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if final_result[idx] != PAPER_INDEX
                && !was_consolidated[idx]
                && final_ages[idx] < 7
            {
                final_ages[idx] = final_ages[idx].saturating_add(1);
            }
        }
    }

    // Final pass: pixels with age 7 become paper
    let mut paper_count = 0;
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if final_ages[idx] >= 7 {
                final_result[idx] = PAPER_INDEX;
                final_ages[idx] = 0;
                paper_count += 1;
            }
        }
    }

    // Debug: log summary
    #[cfg(all(target_arch = "wasm32", feature = "debug-logging"))]
    {
        use web_sys::console;
        let non_paper_before = indices.iter().filter(|&&i| i != PAPER_INDEX).count();
        let non_paper_after = final_result.iter().filter(|&&i| i != PAPER_INDEX).count();
        console::log_1(&format!("RESULT: non-paper before={}, after={}, became_paper={}, final_ages[0..5]={:?}",
            non_paper_before, non_paper_after, paper_count,
            &final_ages.get(0..5.min(final_ages.len())).unwrap_or(&[])).into());
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
