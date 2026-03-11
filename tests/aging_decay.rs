use fmrl::age_step;
use fmrl::encode::{FmrlImage, encode};
use fmrl::format::Palette;

// ── Helpers ───────────────────────────────────────────────────────────────

fn indices_to_rgba(indices: &[u8], palette: &Palette) -> Vec<u8> {
    let mut rgba = vec![0u8; indices.len() * 4];
    for (i, &idx) in indices.iter().enumerate() {
        let [r, g, b] = palette.0[idx as usize];
        rgba[i * 4]     = r;
        rgba[i * 4 + 1] = g;
        rgba[i * 4 + 2] = b;
        rgba[i * 4 + 3] = 255;
    }
    rgba
}

fn encoded_size(indices: &[u8], width: usize, height: usize, palette: &Palette) -> usize {
    let rgba = indices_to_rgba(indices, palette);
    let mut img = FmrlImage::new(width as u16, height as u16, rgba);
    img.palette = palette.clone();
    encode(&img, 0).expect("encode failed").len()
}

fn all_paper(width: usize, height: usize) -> Vec<u8> {
    // Index 0 = paper in v0.4+ format
    vec![0u8; width * height]
}

// ── Tests ─────────────────────────────────────────────────────────────────

/// Aging must eventually converge to all-paper for any starting canvas.
/// When all pixels are paper the encoded size must equal the minimum (the
/// size of an all-paper file of the same dimensions).
#[test]
fn aging_converges_to_all_paper() {
    let w = 128usize;
    let h = 128usize;
    let palette = Palette::default();

    // Varied canvas: solid block, lines, diagonal, checkerboard patch.
    let mut indices = all_paper(w, h);

    // 32×32 solid ink block near centre (ink = index 1 in v0.4+)
    for y in 48..80 {
        for x in 48..80 {
            indices[y * w + x] = 1;
        }
    }
    // Horizontal line across the upper third
    for x in 0..w {
        indices[32 * w + x] = 1;
    }
    // 2-pixel-wide diagonal
    for i in 0..64usize {
        indices[i * w + i] = 1;
        if i + 1 < w {
            indices[i * w + (i + 1)] = 1;
        }
    }
    // 16×16 checkerboard patch (worst case for per-pixel compression)
    for y in 8..24 {
        for x in 96..112 {
            if (x + y) % 2 == 0 {
                indices[y * w + x] = 1;
            }
        }
    }

    let all_paper_size = encoded_size(&all_paper(w, h), w, h, &palette);
    // Count non-paper pixels (index 0 is paper in v0.4+)
    let initial_non_paper: usize = indices.iter().filter(|&&p| p != 0).count();

    let max_steps = 300;
    for step in 1..=max_steps {
        indices = age_step(&indices, w, h);

        // Check for all paper (index 0 in v0.4+)
        if indices.iter().all(|&p| p == 0) {
            let size = encoded_size(&indices, w, h, &palette);
            assert_eq!(
                size, all_paper_size,
                "all-paper reached at step {step} but encoded size {size} != \
                 expected minimum {all_paper_size}"
            );
            return; // test passed
        }
    }

    // Count non-paper pixels remaining (index 0 is paper)
    let remaining: usize = indices.iter().filter(|&&p| p != 0).count();
    panic!(
        "aging did not converge to all-paper within {max_steps} steps; \
         {remaining}/{initial_non_paper} non-paper pixels remain"
    );
}

/// After enough steps the encoded file must be strictly smaller than it
/// started.  zlib compressed sizes can blip up slightly on intermediate
/// shapes, but the overall trend must be downward.
#[test]
fn aging_reduces_file_size_over_many_steps() {
    let w = 128usize;
    let h = 128usize;
    let palette = Palette::default();

    // Dense canvas with several thick strokes (ink = index 1 in v0.4+)
    let mut indices = all_paper(w, h);
    for y in 16..112 {
        for x in 16..112 {
            if x < 48 || y < 48 || x > 80 || y > 80 {
                indices[y * w + x] = 1; // ink border region
            }
        }
    }

    let initial_size = encoded_size(&indices, w, h, &palette);

    for _ in 0..30 {
        indices = age_step(&indices, w, h);
    }

    let reduced_size = encoded_size(&indices, w, h, &palette);
    assert!(
        reduced_size < initial_size,
        "file size should be smaller after 30 age steps: {reduced_size} >= {initial_size}"
    );
}

/// A checkerboard (maximally compressed-size-hostile) must also converge.
#[test]
fn checkerboard_converges_to_all_paper() {
    let w = 64usize;
    let h = 64usize;
    let palette = Palette::default();

    // Checkerboard with ink (1) and paper (0) in v0.4+ format
    let mut indices: Vec<u8> = (0..w * h)
        .map(|i| if (i % w + i / w) % 2 == 0 { 1 } else { 0 })
        .collect();

    for step in 1..=200 {
        indices = age_step(&indices, w, h);
        // Check for all paper (index 0 in v0.4+)
        if indices.iter().all(|&p| p == 0) {
            let size = encoded_size(&indices, w, h, &palette);
            let ap   = encoded_size(&all_paper(w, h), w, h, &palette);
            assert_eq!(size, ap, "checkerboard converged at step {step} but size wrong");
            return;
        }
    }
    panic!("checkerboard did not converge within 200 steps");
}

/// A single isolated pixel must be erased in exactly one step.
#[test]
fn single_pixel_erased_in_one_step() {
    let w = 32usize;
    let h = 32usize;
    let mut indices = all_paper(w, h);
    // Single ink pixel (index 1) in v0.4+ format
    indices[16 * w + 16] = 1;

    let aged = age_step(&indices, w, h);
    assert!(
        aged.iter().all(|&p| p == 0),
        "a single isolated pixel should be erased in one step"
    );
}

/// age_step must never introduce new non-paper pixels.
#[test]
fn aging_never_introduces_non_paper() {
    let w = 64usize;
    let h = 64usize;
    let mut indices = all_paper(w, h);
    // Sparse ink marks (index 1) in v0.4+ format
    for i in (0..w * h).step_by(7) {
        indices[i] = 1;
    }

    for _ in 0..20 {
        // Count non-paper pixels (anything other than index 0)
        let before_non_paper: usize = indices.iter().filter(|&&p| p != 0).count();
        indices = age_step(&indices, w, h);
        let after_non_paper: usize = indices.iter().filter(|&&p| p != 0).count();
        assert!(
            after_non_paper <= before_non_paper,
            "age_step introduced non-paper pixels: {after_non_paper} > {before_non_paper}"
        );
    }
}
