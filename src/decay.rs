use crate::decode::TileData;
use crate::format::{AgeEntry, ColorMode, Palette, TILE_SIZE};
use crate::prng::TilePrng;

const THIRTY_DAYS_MS: f32 = 30.0 * 24.0 * 3600.0 * 1000.0;

/// Derive the effective fade factor for a tile given the global decay_policy.
/// policy=0: ink-heavy tiles get 1.5×, background tiles get 0.5×.
/// Only applies to indexed mode; RGBA mode uses uniform decay.
fn effective_fade(base_fade: f32, tile: &TileData, decay_policy: u8, color_mode: ColorMode) -> f32 {
    if color_mode == ColorMode::Rgba || decay_policy != 0 {
        return base_fade;
    }
    let indices = tile.indices();
    let ink_count = indices.iter().filter(|&&i| i == 0).count();
    let total = indices.len();
    let ink_ratio = ink_count as f32 / total as f32;

    if ink_ratio > 0.5 {
        (base_fade * 1.5).min(1.0)
    } else {
        base_fade * 0.5
    }
}

/// Lerp between a and b by t ∈ [0, 1].
#[inline]
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let a = a as f32;
    let b = b as f32;
    (a + (b - a) * t).round().clamp(0.0, 255.0) as u8
}

/// Render one tile into 32×32×4 RGBA bytes.
pub fn render_tile(tile: &TileData, age: &AgeEntry, palette: &Palette, now_ms: u64, color_mode: ColorMode) -> Vec<u8> {
    let base_fade = if now_ms > age.last_view {
        ((now_ms - age.last_view) as f32 / THIRTY_DAYS_MS).min(1.0)
    } else {
        0.0
    };
    render_tile_inner(tile, age, palette, base_fade, color_mode)
}

pub fn render_tile_with_policy(
    tile: &TileData,
    age: &AgeEntry,
    palette: &Palette,
    now_ms: u64,
    decay_policy: u8,
    color_mode: ColorMode,
) -> Vec<u8> {
    let base_fade = if now_ms > age.last_view {
        ((now_ms - age.last_view) as f32 / THIRTY_DAYS_MS).min(1.0)
    } else {
        0.0
    };
    let fade = effective_fade(base_fade, tile, decay_policy, color_mode);
    render_tile_inner(tile, age, palette, fade, color_mode)
}

fn render_tile_inner(tile: &TileData, age: &AgeEntry, palette: &Palette, fade: f32, color_mode: ColorMode) -> Vec<u8> {
    match color_mode {
        ColorMode::Indexed => render_tile_indexed(tile, age, palette, fade),
        ColorMode::Rgba => render_tile_rgba(tile, age, palette, fade),
    }
}

fn render_tile_indexed(tile: &TileData, age: &AgeEntry, palette: &Palette, fade: f32) -> Vec<u8> {
    let pixel_count = TILE_SIZE * TILE_SIZE;
    let mut output = vec![255u8; pixel_count * 4];
    let mut prng = TilePrng::from_tile(age);

    // Paper color is the target state — all non-paper pixels fade toward it.
    let [paper_r, paper_g, paper_b] = palette.0[1];
    let indices = tile.indices();

    for i in 0..pixel_count {
        let idx = indices[i] as usize;
        let [r, g, b] = palette.0[idx.min(3)];

        // 1. Fade toward paper color. Paper pixels (idx==1) are already at rest.
        let (mut pr, mut pg, mut pb) = if idx == 1 {
            (r, g, b)
        } else {
            (
                lerp_u8(r, paper_r, fade),
                lerp_u8(g, paper_g, fade),
                lerp_u8(b, paper_b, fade),
            )
        };

        // 2. Edge erosion: edge pixels stochastically convert to paper.
        //    Probability gates on edge_damage accumulated over views.
        //    Result is always paper — this reduces information, not adds it.
        if idx != 1 && is_stroke_edge_indexed(indices, i) {
            let edge_prob = (age.edge_damage as f32 / 100.0) * fade;
            if prng.next_f32() < edge_prob {
                pr = paper_r;
                pg = paper_g;
                pb = paper_b;
            }
        }

        let base = i * 4;
        output[base] = pr;
        output[base + 1] = pg;
        output[base + 2] = pb;
        // Paper (index 1) is transparent, all others are opaque
        output[base + 3] = if idx == 1 { 0 } else { 255 };
    }

    output
}

fn render_tile_rgba(tile: &TileData, age: &AgeEntry, palette: &Palette, fade: f32) -> Vec<u8> {
    let pixel_count = TILE_SIZE * TILE_SIZE;
    let mut output = vec![0u8; pixel_count * 4];
    let mut prng = TilePrng::from_tile(age);

    // Paper color is the target state
    let [paper_r, paper_g, paper_b] = palette.0[1];
    let rgba = tile.rgba();

    for i in 0..pixel_count {
        let base = i * 4;
        let r = rgba[base];
        let g = rgba[base + 1];
        let b = rgba[base + 2];
        let a = rgba[base + 3];

        // Check if this pixel is "paper" (close to paper color)
        let is_paper = is_close_to_paper(r, g, b, paper_r, paper_g, paper_b);

        // Fade toward paper color
        let (mut pr, mut pg, mut pb) = if is_paper {
            (r, g, b)
        } else {
            (
                lerp_u8(r, paper_r, fade),
                lerp_u8(g, paper_g, fade),
                lerp_u8(b, paper_b, fade),
            )
        };

        // Edge erosion for non-paper pixels
        if !is_paper && is_stroke_edge_rgba(rgba, i) {
            let edge_prob = (age.edge_damage as f32 / 100.0) * fade;
            if prng.next_f32() < edge_prob {
                pr = paper_r;
                pg = paper_g;
                pb = paper_b;
            }
        }

        output[base] = pr;
        output[base + 1] = pg;
        output[base + 2] = pb;
        output[base + 3] = a; // Preserve original alpha
    }

    output
}

/// Check if a color is close to paper (within threshold)
fn is_close_to_paper(r: u8, g: u8, b: u8, paper_r: u8, paper_g: u8, paper_b: u8) -> bool {
    let dr = (r as i16 - paper_r as i16).abs();
    let dg = (g as i16 - paper_g as i16).abs();
    let db = (b as i16 - paper_b as i16).abs();
    dr < 10 && dg < 10 && db < 10
}

/// A pixel is at a stroke edge if any of its 4-connected neighbors has a different palette index.
fn is_stroke_edge_indexed(indices: &[u8], pixel_idx: usize) -> bool {
    let x = pixel_idx % TILE_SIZE;
    let y = pixel_idx / TILE_SIZE;
    let idx = indices[pixel_idx];

    let neighbors = [
        if x > 0 { Some(pixel_idx - 1) } else { None },
        if x + 1 < TILE_SIZE { Some(pixel_idx + 1) } else { None },
        if y > 0 { Some(pixel_idx - TILE_SIZE) } else { None },
        if y + 1 < TILE_SIZE { Some(pixel_idx + TILE_SIZE) } else { None },
    ];

    for n in neighbors.iter().flatten() {
        if indices[*n] != idx {
            return true;
        }
    }
    false
}

/// A pixel is at a stroke edge if any neighbor has significantly different color
fn is_stroke_edge_rgba(rgba: &[u8], pixel_idx: usize) -> bool {
    let x = pixel_idx % TILE_SIZE;
    let y = pixel_idx / TILE_SIZE;
    let base = pixel_idx * 4;
    let (r, g, b) = (rgba[base], rgba[base + 1], rgba[base + 2]);

    let neighbors = [
        if x > 0 { Some(pixel_idx - 1) } else { None },
        if x + 1 < TILE_SIZE { Some(pixel_idx + 1) } else { None },
        if y > 0 { Some(pixel_idx - TILE_SIZE) } else { None },
        if y + 1 < TILE_SIZE { Some(pixel_idx + TILE_SIZE) } else { None },
    ];

    for n in neighbors.iter().flatten() {
        let nb = n * 4;
        let (nr, ng, nb_) = (rgba[nb], rgba[nb + 1], rgba[nb + 2]);
        // Significant color difference indicates an edge
        let diff = (r as i16 - nr as i16).abs()
            + (g as i16 - ng as i16).abs()
            + (b as i16 - nb_ as i16).abs();
        if diff > 30 {
            return true;
        }
    }
    false
}

/// Update age state for a tile after viewing. Never modifies noise_seed.
pub fn mutate_age(age: &mut AgeEntry, now_ms: u64) {
    age.last_view = now_ms;
    age.fade_level = age.fade_level.saturating_add(1);
    if age.edge_damage < 100 {
        age.edge_damage = age.edge_damage.saturating_add(1);
    }
}
