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

/// Apply one consolidation step using per-tile AGE data with per-pixel "newness" detection.
///
/// Block size increases with consolidation_level: 2×2 → 3×3 → 4×4 → 6×6 → 8×8 → ...
/// Pixels surrounded by paper are treated as "new" (age 0) regardless of tile age.
/// This allows new drawings to start fresh with small blocks.
///
/// `age_levels` maps each tile to its consolidation level (0=initial, 1=next size, etc.)
/// Returns the consolidated indices and updated age levels.
pub fn consolidation_step_with_age(
    indices: &[u8],
    width: usize,
    height: usize,
    age_levels: &mut [u8],
) -> Vec<u8> {
    const TILE_SIZE: usize = 32;
    let tiles_x = width / TILE_SIZE;
    let tiles_y = height / TILE_SIZE;

    // Initialize age_levels if not already set
    if age_levels.len() != tiles_x * tiles_y {
        age_levels.fill(0);
    }

    let mut result = indices.to_vec();

    // First pass: identify "new" pixels (surrounded by paper)
    // These get age 0 treatment
    let mut is_new_pixel = vec![false; width * height];
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let idx = y * width + x;
            if indices[idx] != 0 {
                // Check if surrounded by paper (at least 6 of 8 neighbors are paper)
                let mut paper_neighbors = 0;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let nx = x as isize + dx;
                        let ny = y as isize + dy;
                        if result[ny as usize * width + nx as usize] == 0 {
                            paper_neighbors += 1;
                        }
                    }
                }
                if paper_neighbors >= 6 {
                    is_new_pixel[idx] = true;
                }
            }
        }
    }

    // Build list of pixels to process with their effective age and edge score
    // "New" pixels use age 0, others use their tile's age
    // Edge score: how many paper neighbors (pixels on edges of features consolidate first)
    #[derive(Clone, Copy)]
    struct PixelInfo {
        idx: usize,
        age: u8,
        x: usize,
        y: usize,
        paper_neighbors: u8,  // 0-8, higher = more likely to become paper
    }
    let mut pixels_to_process: Vec<PixelInfo> = Vec::new();
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let idx = y * width + x;
            if indices[idx] != 0 {
                // Count paper neighbors
                let mut paper_count = 0u8;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 { continue; }
                        let nx = x as isize + dx;
                        let ny = y as isize + dy;
                        if result[ny as usize * width + nx as usize] == 0 {
                            paper_count += 1;
                        }
                    }
                }

                let tx = x / TILE_SIZE;
                let ty = y / TILE_SIZE;
                let tile_idx = ty * tiles_x + tx;
                let effective_age = if is_new_pixel[idx] { 0 } else { age_levels[tile_idx] };
                pixels_to_process.push(PixelInfo { idx, age: effective_age, x, y, paper_neighbors: paper_count });
            }
        }
    }

    // Sort by: (age, -paper_neighbors) - youngest pixels with most paper neighbors first
    // This makes edges erode before centers, giving gradual disappearance
    pixels_to_process.sort_by(|a, b| {
        a.age.cmp(&b.age)
            .then_with(|| b.paper_neighbors.cmp(&a.paper_neighbors))
    });

    // Process pixels - for gentler aging, use edge-first processing
    // where edge pixels (surrounded by paper) consolidate first
    for PixelInfo { age, x: px, y: py, .. } in pixels_to_process {
        // Calculate block size based on age
        // age 0 -> 2x2, age 1 -> 2x2, age 2 -> 3x3, age 3 -> 3x3, age 4 -> 4x4...
        // Much more gradual progression
        let block_size = match age {
            0 | 1 => 2,
            2 | 3 => 3,
            4 | 5 => 4,
            6 | 7 => 6,
            8 | 9 => 8,
            10 | 11 => 12,
            _ => 16,
        };

        // Calculate block bounds centered on this pixel
        let half_block = block_size / 2;
        let x_start = px.saturating_sub(half_block);
        let y_start = py.saturating_sub(half_block);
        let x_end = (px + half_block + block_size % 2).min(width);
        let y_end = (py + half_block + block_size % 2).min(height);

        // Count colors in the block
        let mut counts = [0u16; 16];
        let block_area = (y_end - y_start) * (x_end - x_start);
        for y in y_start..y_end {
            for x in x_start..x_end {
                let idx = result[y * width + x];
                counts[idx as usize] += 1;
            }
        }

        // Find most common color (lowest index wins ties)
        let mut best_idx = 0u8;
        let mut best_count = counts[0];
        for i in 1..16 {
            if counts[i] > best_count {
                best_count = counts[i];
                best_idx = i as u8;
            }
        }

        // Fill block with consolidated value
        for y in y_start..y_end {
            for x in x_start..x_end {
                result[y * width + x] = best_idx;
            }
        }
    }

    // Update tile ages
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let tile_idx = ty * tiles_x + tx;
            let tx0 = tx * TILE_SIZE;
            let ty0 = ty * TILE_SIZE;

            // Check if tile became all paper
            let mut all_paper = true;
            let mut has_content = false;
            for y in 0..TILE_SIZE {
                for x in 0..TILE_SIZE {
                    if result[(ty0 + y) * width + (tx0 + x)] != 0 {
                        all_paper = false;
                        has_content = true;
                    }
                }
            }

            if all_paper {
                age_levels[tile_idx] = 0;
            } else if has_content {
                age_levels[tile_idx] = age_levels[tile_idx].saturating_add(1);
            }
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
