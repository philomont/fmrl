use crate::decode::TileData;
use crate::format::{AgeEntry, Palette, TILE_SIZE};
use crate::prng::TilePrng;

const THIRTY_DAYS_MS: f32 = 30.0 * 24.0 * 3600.0 * 1000.0;

/// Derive the effective fade factor for a tile given the global decay_policy.
/// policy=0: ink-heavy tiles get 1.5×, background tiles get 0.5×.
fn effective_fade(base_fade: f32, tile: &TileData, decay_policy: u8) -> f32 {
    if decay_policy != 0 {
        return base_fade;
    }
    let ink_count = tile.indices.iter().filter(|&&i| i == 0).count();
    let total = tile.indices.len();
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
pub fn render_tile(tile: &TileData, age: &AgeEntry, palette: &Palette, now_ms: u64) -> Vec<u8> {
    let base_fade = if now_ms > age.last_view {
        ((now_ms - age.last_view) as f32 / THIRTY_DAYS_MS).min(1.0)
    } else {
        0.0
    };
    render_tile_inner(tile, age, palette, base_fade)
}

pub fn render_tile_with_policy(
    tile: &TileData,
    age: &AgeEntry,
    palette: &Palette,
    now_ms: u64,
    decay_policy: u8,
) -> Vec<u8> {
    let base_fade = if now_ms > age.last_view {
        ((now_ms - age.last_view) as f32 / THIRTY_DAYS_MS).min(1.0)
    } else {
        0.0
    };
    let fade = effective_fade(base_fade, tile, decay_policy);
    render_tile_inner(tile, age, palette, fade)
}

fn render_tile_inner(tile: &TileData, age: &AgeEntry, palette: &Palette, fade: f32) -> Vec<u8> {
    let pixel_count = TILE_SIZE * TILE_SIZE;
    let mut output = vec![255u8; pixel_count * 4];
    let mut prng = TilePrng::from_tile(age);

    // Paper color is the target state — all non-paper pixels fade toward it.
    let [paper_r, paper_g, paper_b] = palette.0[1];

    for i in 0..pixel_count {
        let idx = tile.indices[i] as usize;
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
        if idx != 1 && is_stroke_edge(tile, i) {
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
        output[base + 3] = 255;
    }

    output
}

/// A pixel is at a stroke edge if any of its 4-connected neighbors has a different palette index.
fn is_stroke_edge(tile: &TileData, pixel_idx: usize) -> bool {
    let x = pixel_idx % TILE_SIZE;
    let y = pixel_idx / TILE_SIZE;
    let idx = tile.indices[pixel_idx];

    let neighbors = [
        if x > 0 { Some(pixel_idx - 1) } else { None },
        if x + 1 < TILE_SIZE { Some(pixel_idx + 1) } else { None },
        if y > 0 { Some(pixel_idx - TILE_SIZE) } else { None },
        if y + 1 < TILE_SIZE { Some(pixel_idx + TILE_SIZE) } else { None },
    ];

    for n in neighbors.iter().flatten() {
        if tile.indices[*n] != idx {
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
