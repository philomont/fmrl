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

/// Apply one consolidation step: reduce effective resolution by 2×.
///
/// Divides the image into 2×2 blocks. Each block becomes a single pixel
/// with the most common index from the 4 pixels. On ties, the lowest
/// index wins. The result is then upscaled back to original size by
/// duplicating each consolidated pixel 2×2.
///
/// This creates large uniform areas that zlib compresses efficiently,
/// genuinely reducing information content (file size decreases).
///
/// Returns a new Vec with the consolidated indices at original dimensions.
pub fn consolidation_step(indices: &[u8], width: usize, height: usize) -> Vec<u8> {
    assert!(width % 2 == 0 && height % 2 == 0, "dimensions must be even for consolidation");

    let half_w = width / 2;
    let half_h = height / 2;

    // Step 1: consolidate 2×2 blocks into 1 pixel each
    let mut consolidated = vec![0u8; half_w * half_h];

    for by in 0..half_h {
        for bx in 0..half_w {
            // Collect the 4 pixels in this 2×2 block
            let x0 = bx * 2;
            let y0 = by * 2;

            let p0 = indices[y0 * width + x0];
            let p1 = indices[y0 * width + (x0 + 1)];
            let p2 = indices[(y0 + 1) * width + x0];
            let p3 = indices[(y0 + 1) * width + (x0 + 1)];

            // Find most common index, with lowest index winning ties
            consolidated[by * half_w + bx] = most_common_index([p0, p1, p2, p3]);
        }
    }

    // Step 2: upscale back to original dimensions by 2× duplication
    let mut result = vec![0u8; width * height];
    for by in 0..half_h {
        for bx in 0..half_w {
            let c = consolidated[by * half_w + bx];
            let x0 = bx * 2;
            let y0 = by * 2;

            // Write 2×2 block
            result[y0 * width + x0] = c;
            result[y0 * width + (x0 + 1)] = c;
            result[(y0 + 1) * width + x0] = c;
            result[(y0 + 1) * width + (x0 + 1)] = c;
        }
    }

    result
}

/// Find the most common index in 4 values.
/// Returns the lowest index on ties.
fn most_common_index([a, b, c, d]: [u8; 4]) -> u8 {
    // Count occurrences (max index is 15 for 16-color palette)
    let mut counts = [0u8; 16];
    counts[a as usize] += 1;
    counts[b as usize] += 1;
    counts[c as usize] += 1;
    counts[d as usize] += 1;

    // Find index with highest count, lowest index on ties
    let mut best_idx = 0u8;
    let mut best_count = counts[0];
    for i in 1..16 {
        if counts[i] > best_count {
            best_count = counts[i];
            best_idx = i as u8;
        }
    }
    best_idx
}
