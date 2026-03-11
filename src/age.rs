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

/// Apply one consolidation step using per-tile AGE data.
///
/// Block size increases with consolidation_level: 2×2 → 4×4 → 8×8 → 16×16 → etc.
/// Tiles with non-paper content are prioritized (start aging from there).
/// When block size exceeds tile boundaries, step back to largest aligned block.
///
/// `age_levels` maps each tile to its consolidation level (0=initial, 1=2x2 done, etc.)
/// Returns the consolidated indices and updated age levels.
pub fn consolidation_step_with_age(
    indices: &[u8],
    width: usize,
    height: usize,
    age_levels: &mut [u8], // per-tile consolidation levels
) -> Vec<u8> {
    const TILE_SIZE: usize = 32;
    let tiles_x = width / TILE_SIZE;
    let tiles_y = height / TILE_SIZE;

    // Initialize age_levels if not already set
    if age_levels.len() != tiles_x * tiles_y {
        age_levels.fill(0);
    }

    let mut result = indices.to_vec();

    // Build list of tiles with content, sorted by content ratio (most first)
    let mut tiles_with_content: Vec<(usize, f32)> = Vec::new();
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let tile_idx = ty * tiles_x + tx;
            let tx0 = tx * TILE_SIZE;
            let ty0 = ty * TILE_SIZE;

            let mut non_paper_count = 0;
            for y in 0..TILE_SIZE {
                for x in 0..TILE_SIZE {
                    if indices[(ty0 + y) * width + (tx0 + x)] != 0 {
                        non_paper_count += 1;
                    }
                }
            }

            let ratio = non_paper_count as f32 / (TILE_SIZE * TILE_SIZE) as f32;
            if non_paper_count > 0 {
                tiles_with_content.push((tile_idx, ratio));
            }
        }
    }

    // Sort by content ratio descending (tiles with more content age first)
    tiles_with_content.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    // Process tiles in order of content
    for (tile_idx, _ratio) in tiles_with_content {
        let tx = tile_idx % tiles_x;
        let ty = tile_idx / tiles_x;
        let level = age_levels[tile_idx];

        // Calculate block size: 2^(level+1)
        // level 0 -> 2x2, level 1 -> 4x4, level 2 -> 8x8, etc.
        let mut block_size = 1usize << (level + 1);

        // Step back if block size doesn't align with tile boundaries
        // or exceeds image dimensions
        while block_size > TILE_SIZE {
            // Check if this block would align with tile grid
            let tile_aligned = (tx * TILE_SIZE) % block_size == 0 &&
                              (ty * TILE_SIZE) % block_size == 0;
            if !tile_aligned || tx * TILE_SIZE + block_size > width ||
               ty * TILE_SIZE + block_size > height {
                // Step back to smaller block
                block_size >>= 1;
            } else {
                break;
            }
        }

        // Also ensure we don't exceed image bounds
        while tx * TILE_SIZE + block_size > width ||
              ty * TILE_SIZE + block_size > height {
            block_size >>= 1;
        }

        // Minimum block size is 2
        if block_size < 2 {
            block_size = 2;
        }

        // Apply consolidation to this tile with calculated block size
        let tx0 = tx * TILE_SIZE;
        let ty0 = ty * TILE_SIZE;
        let blocks_per_tile = TILE_SIZE / block_size;

        for by in 0..blocks_per_tile {
            for bx in 0..blocks_per_tile {
                let x0 = tx0 + bx * block_size;
                let y0 = ty0 + by * block_size;

                // Collect all pixels in this block
                let mut counts = [0u16; 16];
                for y in 0..block_size {
                    for x in 0..block_size {
                        let idx = result[(y0 + y) * width + (x0 + x)];
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
                for y in 0..block_size {
                    for x in 0..block_size {
                        result[(y0 + y) * width + (x0 + x)] = best_idx;
                    }
                }
            }
        }

        // Increment age level for this tile
        age_levels[tile_idx] = age_levels[tile_idx].saturating_add(1);
    }

    result
}

/// Simple consolidation step without AGE tracking (for backward compatibility).
/// Uses 2×2 blocks throughout the image.
pub fn consolidation_step(indices: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut dummy_age = vec![0u8; (width / 32) * (height / 32)];
    consolidation_step_with_age(indices, width, height, &mut dummy_age)
}
