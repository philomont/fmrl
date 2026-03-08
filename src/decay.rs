use crate::decode::TileData;
use crate::format::{AgeEntry, Palette, TILE_SIZE};
use crate::prng::TilePrng;

const THIRTY_DAYS_MS: f32 = 30.0 * 24.0 * 3600.0 * 1000.0;

/// Desaturation weight per palette slot.
/// 0=ink→1.0, 1=paper→0.3, 2=accent→0.7, 3=white→0.2
const DESAT_WEIGHT: [f32; 4] = [1.0, 0.3, 0.7, 0.2];

/// Derive the effective fade factor for a tile given the global decay_policy.
/// policy=0: ink-heavy tiles get 1.5×, background tiles get 0.5×.
fn effective_fade(base_fade: f32, tile: &TileData, decay_policy: u8) -> f32 {
    if decay_policy != 0 {
        return base_fade;
    }
    // Count ink pixels (palette index 0)
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

    // decay_policy is not passed here; caller integrates it via effective_fade externally
    // For this function we use base_fade directly (caller should pre-compute effective fade)
    // We expose a separate render_tile_with_policy function for the orchestration layer.
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

    for i in 0..pixel_count {
        let idx = tile.indices[i] as usize;
        let [r, g, b] = palette.0[idx.min(3)];
        let weight = DESAT_WEIGHT[idx.min(3)];

        // 1. Desaturate: lerp toward grayscale
        let luma = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) as u8;
        let desat_strength = fade * weight;
        let mut pr = lerp_u8(r, luma, desat_strength);
        let mut pg = lerp_u8(g, luma, desat_strength);
        let mut pb = lerp_u8(b, luma, desat_strength);

        // 2. Noise injection: with probability fade*0.15, flip to adjacent palette color
        if prng.next_f32() < fade * 0.15 {
            // adjacent palette color: cycle index
            let alt_idx = (idx + 1) % 4;
            let [ar, ag, ab] = palette.0[alt_idx];
            pr = lerp_u8(pr, ar, 0.5);
            pg = lerp_u8(pg, ag, 0.5);
            pb = lerp_u8(pb, ab, 0.5);
        }

        // 3. Edge damage: check if pixel is at a stroke edge
        let is_edge = is_stroke_edge(tile, i);
        if is_edge {
            let edge_prob = (age.edge_damage as f32 / 100.0) * fade;
            if prng.next_f32() < edge_prob {
                // salt-and-pepper: randomly go white or black
                if prng.next_f32() < 0.5 {
                    pr = 255; pg = 255; pb = 255;
                } else {
                    pr = 0; pg = 0; pb = 0;
                }
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
    // Increment fade_level at a slow rate (saturating)
    age.fade_level = age.fade_level.saturating_add(1);
    // Increment edge_damage at a slow rate (saturating at 100)
    if age.edge_damage < 100 {
        age.edge_damage = age.edge_damage.saturating_add(1);
    }
    // noise_seed is intentionally NOT modified
}
