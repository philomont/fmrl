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

/// Apply one consolidation step.
///
/// Divides the image into N×N blocks. Each block becomes a single color:
/// - The most common color in the block wins
/// - Ties go to the lowest index (so paper wins ties)
/// - Block size N grows with age: 2→4→8→16→32...
///
/// This creates larger uniform areas with each step, genuinely reducing information.
/// Features gradually merge and eventually become paper when surrounded.
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

    // Calculate block size from max tile age
    // Each age level doubles the block size: 0=2x2, 1=4x4, 2=8x8, etc.
    let max_age = age_levels.iter().copied().max().unwrap_or(0);
    let shift = (max_age + 1).min(5);  // Cap at 32x32 blocks (shift 5 = 32)
    let block_size = 1usize << shift;  // 2, 4, 8, 16, 32

    // Process the entire image in fixed blocks
    let mut result = indices.to_vec();

    for y in (0..height).step_by(block_size) {
        for x in (0..width).step_by(block_size) {
            let y_end = (y + block_size).min(height);
            let x_end = (x + block_size).min(width);

            // Count colors in this block
            let mut counts = [0u16; 16];
            for by in y..y_end {
                for bx in x..x_end {
                    let idx = result[by * width + bx];
                    counts[idx as usize] += 1;
                }
            }

            // Find most common (lowest index wins ties)
            let mut best_idx = 0u8;
            let mut best_count = counts[0];
            for i in 1..16 {
                if counts[i] > best_count {
                    best_count = counts[i];
                    best_idx = i as u8;
                }
            }

            // Fill block with consolidated value
            for by in y..y_end {
                for bx in x..x_end {
                    result[by * width + bx] = best_idx;
                }
            }
        }
    }

    // Update tile ages
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let tile_idx = ty * tiles_x + tx;
            let tx0 = tx * TILE_SIZE;
            let ty0 = ty * TILE_SIZE;

            // Check if tile is all paper
            let mut all_paper = true;
            let mut has_content = false;
            for y in 0..TILE_SIZE {
                for x in 0..TILE_SIZE {
                    let val = result[(ty0 + y) * width + (tx0 + x)];
                    if val != 0 {
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
