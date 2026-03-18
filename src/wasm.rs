#![cfg(feature = "wasm")]

use wasm_bindgen::prelude::*;

use crate::age::{
    age_by_bleaching, age_by_consolidation, age_by_erosion, consolidation_step_with_age,
};
use crate::decode::{decode, DecodedFmrl};
use crate::encode::{encode, FmrlImage};
use crate::format::{AgeType, Palette, TILE_SIZE};
use crate::render;

#[wasm_bindgen]
pub struct FmrlView {
    file_bytes: Vec<u8>,
    decoded: DecodedFmrl,
}

#[wasm_bindgen]
impl FmrlView {
    pub fn new(data: &[u8]) -> Result<FmrlView, JsValue> {
        let decoded = decode(data).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(FmrlView {
            file_bytes: data.to_vec(),
            decoded,
        })
    }

    /// Decode and apply decay. Returns RGBA pixels. Also mutates file_bytes.
    pub fn decode_and_decay(&mut self) -> Result<Vec<u8>, JsValue> {
        let now = js_sys::Date::now() as u64;
        render(&mut self.decoded, now, &mut self.file_bytes)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Return the mutated file bytes for persistence after decode_and_decay.
    pub fn get_mutated_bytes(&self) -> Vec<u8> {
        self.file_bytes.clone()
    }

    /// Number of times this image has been viewed (using fade_level of tile 0 as proxy).
    pub fn view_count(&self) -> usize {
        self.decoded
            .age
            .first()
            .map(|a| a.fade_level as usize)
            .unwrap_or(0)
    }

    /// last_view timestamp (ms since Unix epoch) from tile 0. Returns f64 for JS compatibility.
    pub fn last_view_ms(&self) -> f64 {
        self.decoded
            .age
            .first()
            .map(|a| a.last_view as f64)
            .unwrap_or(0.0)
    }

    /// Average fade_level across all tiles (0–255).
    pub fn avg_fade_level(&self) -> u8 {
        if self.decoded.age.is_empty() {
            return 0;
        }
        let sum: u32 = self.decoded.age.iter().map(|a| a.fade_level as u32).sum();
        (sum / self.decoded.age.len() as u32) as u8
    }

    pub fn width(&self) -> u16 {
        self.decoded.ihdr.width
    }

    pub fn height(&self) -> u16 {
        self.decoded.ihdr.height
    }

    /// Returns the age types as a comma-separated string (e.g., "0,1,2")
    pub fn age_types(&self) -> String {
        self.decoded
            .ihdr
            .age_types
            .iter()
            .map(|at| at.as_u8().to_string())
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Returns the age levels (consolidation levels from fade_level) for all tiles.
    /// Each entry is the consolidation level for that tile (0=initial, 1=2x2 done, etc.)
    pub fn age_levels(&self) -> Vec<u8> {
        self.decoded.age.iter().map(|a| a.fade_level).collect()
    }

    /// Returns per-pixel ages extracted from packed tile data.
    /// Unpacks low nibble from packed format.
    pub fn pixel_ages(&self) -> Vec<u8> {
        let w = self.decoded.ihdr.width as usize;
        let h = self.decoded.ihdr.height as usize;
        let mut ages = vec![0u8; w * h];

        for tile in &self.decoded.tiles {
            let tx = tile.tx as usize;
            let ty = tile.ty as usize;
            let tile_ages = tile.pixel_ages();
            for py in 0..TILE_SIZE {
                let dst_y = ty * TILE_SIZE + py;
                let dst_x = tx * TILE_SIZE;
                let src_start = py * TILE_SIZE;
                let dst_start = dst_y * w + dst_x;
                ages[dst_start..dst_start + TILE_SIZE]
                    .copy_from_slice(&tile_ages[src_start..src_start + TILE_SIZE]);
            }
        }

        ages
    }
}

/// Encode raw RGB pixels into a new .fmrl file.
/// `rgb` must be `width * height * 3` bytes; dimensions must be multiples of 128.
/// Format: R = index × 16, G = contrast (0x00 for paper, 0xFF otherwise), B = age × 16
/// `age_types`: array of age type values (0=erosion, 1=consolidation, 2=bleach)
#[wasm_bindgen]
pub fn encode_rgb(
    rgb: &[u8],
    width: u16,
    height: u16,
    age_types: &[u8],
) -> Result<Vec<u8>, JsValue> {
    encode_rgb_with_levels(rgb, width, height, age_types, &[])
}

/// Encode raw RGB pixels with existing age levels.
/// `age_types`: array of age type values (0=erosion, 1=consolidation, 2=bleach)
/// `age_levels`: per-tile consolidation levels (empty = start fresh)
#[wasm_bindgen]
pub fn encode_rgb_with_levels(
    rgb: &[u8],
    width: u16,
    height: u16,
    age_types: &[u8],
    age_levels: &[u8],
) -> Result<Vec<u8>, JsValue> {
    let now = js_sys::Date::now() as u64;
    let mut image = FmrlImage::new(width, height, rgb.to_vec());

    // Convert age_types bytes to AgeType enum
    image.age_types = age_types
        .iter()
        .filter_map(|&at| AgeType::from_u8(at))
        .collect();

    // Ensure at least one age type
    if image.age_types.is_empty() {
        image.age_types = vec![AgeType::Erosion];
    }

    if !age_levels.is_empty() {
        image.age_levels = Some(age_levels.to_vec());
    }

    encode(&image, now).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Decode a .fmrl file and return RGB visualization pixels.
/// Format: R = index × 16, G = contrast (0x00 for paper, 0xFF otherwise), B = age × 16
/// Returns 3 bytes per pixel (RGB, no alpha)
#[wasm_bindgen]
pub fn decode_to_rgb(data: &[u8]) -> Result<Vec<u8>, JsValue> {
    let decoded = decode(data).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let w = decoded.ihdr.width as usize;
    let h = decoded.ihdr.height as usize;
    let mut rgb = vec![0u8; w * h * 3];

    for tile in &decoded.tiles {
        let tx = tile.tx as usize;
        let ty = tile.ty as usize;
        let tile_rgb = tile.to_rgb();
        for py in 0..TILE_SIZE {
            let dst_y = ty * TILE_SIZE + py;
            let dst_x = tx * TILE_SIZE;
            let src_start = py * TILE_SIZE * 3;
            let dst_start = (dst_y * w + dst_x) * 3;
            rgb[dst_start..dst_start + TILE_SIZE * 3]
                .copy_from_slice(&tile_rgb[src_start..src_start + TILE_SIZE * 3]);
        }
    }

    Ok(rgb)
}

/// Apply one consolidation step with per-pixel ages.
/// Returns [indices_out, pixel_ages_out] as a single concatenated array.
/// indices_out is width*height bytes, pixel_ages_out is width*height bytes.
#[wasm_bindgen]
pub fn consolidation_step_with_ages(
    indices: &[u8],
    pixel_ages: &[u8],
    width: u16,
    height: u16,
) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let (new_indices, new_ages) = age_by_consolidation(indices, pixel_ages, w, h);

    // Concatenate results: indices first, then ages
    let mut result = Vec::with_capacity(w * h * 2);
    result.extend_from_slice(&new_indices);
    result.extend_from_slice(&new_ages);
    result
}

/// Apply one consolidation step: reduce resolution by 2× then upscale back.
///
/// `data` must be `width * height` bytes of palette indices.
/// Each 2×2 block becomes one pixel with the most common index (lowest wins ties).
/// Result is upscaled back to original dimensions by duplication.
/// See `age::age_by_consolidation` for the full algorithm description.
#[wasm_bindgen]
pub fn consolidation_step_indices(data: &[u8], width: u16, height: u16) -> Vec<u8> {
    consolidation_step_with_age(
        data,
        width as usize,
        height as usize,
        &mut vec![0u8; ((width as usize / TILE_SIZE) * (height as usize / TILE_SIZE)).max(1)],
    )
}

/// Apply one convolutional bleach step.
///
/// Uses 2×2 convolution to detect and bleach "noisy" blocks:
/// - If 3+ different indices in 2×2 block → becomes paper
/// - If 2 indices with unequal counts → becomes paper
/// - If 2 indices with equal counts (2 each) AND diagonal pattern → becomes paper
/// See `age::age_by_bleaching` for the full algorithm description.
#[wasm_bindgen]
pub fn bleach_step_indices(data: &[u8], width: u16, height: u16) -> Vec<u8> {
    age_by_bleaching(data, width as usize, height as usize)
}

/// Create a fresh demo .fmrl file with a manuscript-like pattern.
/// The initial last_view is set 20 days in the past so decay is visible immediately.
#[wasm_bindgen]
pub fn create_demo_fmrl() -> Result<Vec<u8>, JsValue> {
    let w = 128u16;
    let h = 128u16;
    let palette = Palette::default();
    // RGB format: 3 bytes per pixel
    let mut pixels = vec![0u8; w as usize * h as usize * 3];

    // Background: aged paper (index 0 in v0.4+)
    // R = 0, G = 0 (paper), B = 0
    fill_all(&mut pixels, w, h, 0, 0, 0);

    // Outer border, 2px thick, 8px inset (ink = index 1)
    // R = 16, G = 255 (not paper), B = 0
    for t in 0..2u16 {
        hline(&mut pixels, w, 8 + t, 8, w - 16, 16, 255, 0);
        hline(&mut pixels, w, h - 9 - t, 8, w - 16, 16, 255, 0);
        vline(&mut pixels, w, 8, 8 + t, h - 16, 16, 255, 0);
        vline(&mut pixels, w, w - 9, 8 + t, h - 16, 16, 255, 0);
    }

    // Accent margin line (2px wide at x=27) - use a lighter shade (index 8)
    // R = 128, G = 255, B = 0
    vline(&mut pixels, w, 27, 12, h - 24, 128, 255, 0);
    vline(&mut pixels, w, 28, 12, h - 24, 128, 255, 0);

    // Horizontal manuscript lines every 12px in ink (index 1)
    // R = 16, G = 255, B = 0
    let mut y = 26u16;
    while y < h - 18 {
        hline(&mut pixels, w, y, 32, w - 44, 16, 255, 0);
        y += 12;
    }

    // Small accent ink blots (3×3) - use index 5 (mid-gray)
    // R = 80, G = 255, B = 0
    for &(bx, by) in &[(48u16, 25u16), (76, 49), (60, 73), (92, 97), (44, 101)] {
        filled_rect(&mut pixels, w, bx, by, 3, 3, 80, 255, 0);
    }

    // Pre-age 20 days so decay is visible on first load
    let twenty_days_ago = (js_sys::Date::now() as u64).saturating_sub(20 * 24 * 3600 * 1_000);

    let mut image = FmrlImage::new(w, h, pixels);
    image.palette = palette;
    image.meta = Some(serde_json::json!({
        "title": "FMRL Demo",
        "tags": ["manuscript", "decay", "demo"]
    }));

    encode(&image, twenty_days_ago).map_err(|e| JsValue::from_str(&e.to_string()))
}

// --- pixel helpers (used only in create_demo_fmrl) ---

fn fill_all(pixels: &mut [u8], w: u16, h: u16, r: u8, g: u8, b: u8) {
    for i in 0..(w as usize * h as usize) {
        let base = i * 3;
        pixels[base] = r;
        pixels[base + 1] = g;
        pixels[base + 2] = b;
    }
}

fn set_px(pixels: &mut [u8], w: u16, x: u16, y: u16, r: u8, g: u8, b: u8) {
    if x >= w {
        return;
    }
    let pos = (y as usize * w as usize + x as usize) * 3;
    if pos + 2 >= pixels.len() {
        return;
    }
    pixels[pos] = r;
    pixels[pos + 1] = g;
    pixels[pos + 2] = b;
}

fn hline(pixels: &mut [u8], w: u16, y: u16, x: u16, len: u16, r: u8, g: u8, b: u8) {
    for dx in 0..len {
        set_px(pixels, w, x + dx, y, r, g, b);
    }
}

fn vline(pixels: &mut [u8], w: u16, x: u16, y: u16, len: u16, r: u8, g: u8, b: u8) {
    for dy in 0..len {
        set_px(pixels, w, x, y + dy, r, g, b);
    }
}

fn filled_rect(pixels: &mut [u8], w: u16, x: u16, y: u16, rw: u16, rh: u16, r: u8, g: u8, b: u8) {
    for dy in 0..rh {
        for dx in 0..rw {
            set_px(pixels, w, x + dx, y + dy, r, g, b);
        }
    }
}
