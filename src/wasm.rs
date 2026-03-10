#![cfg(feature = "wasm")]

use wasm_bindgen::prelude::*;

use crate::age::age_step;
use crate::decode::{DecodedFmrl, decode};
use crate::encode::{FmrlImage, encode};
use crate::format::{ColorMode, Palette, TILE_SIZE};
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
        self.decoded.age.first().map(|a| a.fade_level as usize).unwrap_or(0)
    }

    /// last_view timestamp (ms since Unix epoch) from tile 0. Returns f64 for JS compatibility.
    pub fn last_view_ms(&self) -> f64 {
        self.decoded.age.first().map(|a| a.last_view as f64).unwrap_or(0.0)
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

    /// Returns the color mode: 3 = indexed, 6 = RGBA
    pub fn color_mode(&self) -> u8 {
        self.decoded.ihdr.color_mode.as_u8()
    }

    /// Returns true if this file uses RGBA mode
    pub fn is_rgba(&self) -> bool {
        self.decoded.ihdr.color_mode == ColorMode::Rgba
    }
}

/// Encode raw RGBA pixels into a new .fmrl file using indexed mode (palette quantization).
/// `rgba` must be `width * height * 4` bytes; dimensions must be multiples of 32.
#[wasm_bindgen]
pub fn encode_rgba(rgba: &[u8], width: u16, height: u16) -> Result<Vec<u8>, JsValue> {
    let now = js_sys::Date::now() as u64;
    let image = FmrlImage::new(width, height, rgba.to_vec());
    encode(&image, now).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Encode raw RGBA pixels into a new .fmrl file using full RGBA mode (no palette quantization).
/// `rgba` must be `width * height * 4` bytes; dimensions must be multiples of 32.
#[wasm_bindgen]
pub fn encode_rgba_full(rgba: &[u8], width: u16, height: u16) -> Result<Vec<u8>, JsValue> {
    let now = js_sys::Date::now() as u64;
    let image = FmrlImage::new_rgba(width, height, rgba.to_vec());
    encode(&image, now).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Decode a .fmrl file and return flat palette indices (0–3), row-major, width×height bytes.
/// Does not apply decay and does not mutate the file — intended for loading into an editor.
///
/// Note: For RGBA mode files, this converts RGBA back to indices via quantization.
/// Use `decode_to_rgba` to get raw RGBA data for RGBA mode files.
#[wasm_bindgen]
pub fn decode_to_indices(data: &[u8]) -> Result<Vec<u8>, JsValue> {
    let decoded = decode(data).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let w = decoded.ihdr.width as usize;
    let h = decoded.ihdr.height as usize;
    let mut indices = vec![1u8; w * h]; // default to paper

    match decoded.ihdr.color_mode {
        ColorMode::Indexed => {
            for tile in &decoded.tiles {
                let tx = tile.tx as usize;
                let ty = tile.ty as usize;
                let tile_indices = tile.indices();
                for py in 0..TILE_SIZE {
                    let dst_y = ty * TILE_SIZE + py;
                    let dst_x = tx * TILE_SIZE;
                    let src_start = py * TILE_SIZE;
                    let dst_start = dst_y * w + dst_x;
                    indices[dst_start..dst_start + TILE_SIZE]
                        .copy_from_slice(&tile_indices[src_start..src_start + TILE_SIZE]);
                }
            }
        }
        ColorMode::Rgba => {
            // Quantize RGBA back to indices for editor compatibility
            let palette = &decoded.palette;
            for tile in &decoded.tiles {
                let tx = tile.tx as usize;
                let ty = tile.ty as usize;
                let tile_rgba = tile.rgba();
                for py in 0..TILE_SIZE {
                    let dst_y = ty * TILE_SIZE + py;
                    let dst_x = tx * TILE_SIZE;
                    for px in 0..TILE_SIZE {
                        let src_base = (py * TILE_SIZE + px) * 4;
                        let r = tile_rgba[src_base];
                        let g = tile_rgba[src_base + 1];
                        let b = tile_rgba[src_base + 2];
                        let idx = quantize_to_palette(r, g, b, palette);
                        indices[dst_y * w + dst_x + px] = idx;
                    }
                }
            }
        }
    }

    Ok(indices)
}

/// Decode a .fmrl file and return raw RGBA pixels.
/// For indexed mode, this expands palette colors to RGBA.
/// For RGBA mode, this returns the original RGBA data.
#[wasm_bindgen]
pub fn decode_to_rgba(data: &[u8]) -> Result<Vec<u8>, JsValue> {
    let decoded = decode(data).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let w = decoded.ihdr.width as usize;
    let h = decoded.ihdr.height as usize;
    let mut rgba = vec![0u8; w * h * 4];

    match decoded.ihdr.color_mode {
        ColorMode::Indexed => {
            let palette = &decoded.palette;
            for tile in &decoded.tiles {
                let tx = tile.tx as usize;
                let ty = tile.ty as usize;
                let tile_indices = tile.indices();
                for py in 0..TILE_SIZE {
                    let dst_y = ty * TILE_SIZE + py;
                    let dst_x = tx * TILE_SIZE;
                    for px in 0..TILE_SIZE {
                        let idx = tile_indices[py * TILE_SIZE + px] as usize;
                        let [r, g, b] = palette.0[idx.min(3)];
                        let dst_base = (dst_y * w + dst_x + px) * 4;
                        rgba[dst_base] = r;
                        rgba[dst_base + 1] = g;
                        rgba[dst_base + 2] = b;
                        rgba[dst_base + 3] = 255;
                    }
                }
            }
        }
        ColorMode::Rgba => {
            for tile in &decoded.tiles {
                let tx = tile.tx as usize;
                let ty = tile.ty as usize;
                let tile_rgba = tile.rgba();
                for py in 0..TILE_SIZE {
                    let dst_y = ty * TILE_SIZE + py;
                    let dst_x = tx * TILE_SIZE;
                    let src_start = py * TILE_SIZE * 4;
                    let dst_start = (dst_y * w + dst_x) * 4;
                    rgba[dst_start..dst_start + TILE_SIZE * 4]
                        .copy_from_slice(&tile_rgba[src_start..src_start + TILE_SIZE * 4]);
                }
            }
        }
    }

    Ok(rgba)
}

/// Quantize an RGB value to the nearest palette index
fn quantize_to_palette(r: u8, g: u8, b: u8, palette: &Palette) -> u8 {
    let mut best_idx = 0u8;
    let mut best_dist = u32::MAX;
    for (i, color) in palette.0.iter().enumerate() {
        let dr = (r as i32) - (color[0] as i32);
        let dg = (g as i32) - (color[1] as i32);
        let db = (b as i32) - (color[2] as i32);
        let dist = (dr * dr + dg * dg + db * db) as u32;
        if dist < best_dist {
            best_dist = dist;
            best_idx = i as u8;
        }
    }
    best_idx
}

/// Apply one aging step to flat palette indices and return the result.
///
/// `data` must be `width * height` bytes of palette indices (0–3; 1 = paper).
/// Returns a new array of the same length with aged indices.
/// See `age::age_step` for the full algorithm description.
#[wasm_bindgen]
pub fn age_step_indices(data: &[u8], width: u16, height: u16) -> Vec<u8> {
    age_step(data, width as usize, height as usize)
}

/// Create a fresh demo .fmrl file with a manuscript-like pattern.
/// The initial last_view is set 20 days in the past so decay is visible immediately.
#[wasm_bindgen]
pub fn create_demo_fmrl() -> Result<Vec<u8>, JsValue> {
    let w = 128u16;
    let h = 128u16;
    let palette = Palette::default();
    let mut pixels = vec![0u8; w as usize * h as usize * 4];

    // Background: aged paper
    fill_all(&mut pixels, w, h, &palette, 1);

    // Outer border, 2px thick, 8px inset
    for t in 0..2u16 {
        hline(&mut pixels, w, 8 + t, 8, w - 16, &palette, 0);
        hline(&mut pixels, w, h - 9 - t, 8, w - 16, &palette, 0);
        vline(&mut pixels, w, 8, 8 + t, h - 16, &palette, 0);
        vline(&mut pixels, w, w - 9, 8 + t, h - 16, &palette, 0);
    }

    // Accent margin line (2px wide at x=27)
    vline(&mut pixels, w, 27, 12, h - 24, &palette, 2);
    vline(&mut pixels, w, 28, 12, h - 24, &palette, 2);

    // Horizontal manuscript lines every 12px in ink
    let mut y = 26u16;
    while y < h - 18 {
        hline(&mut pixels, w, y, 32, w - 44, &palette, 0);
        y += 12;
    }

    // Small accent ink blots (3×3)
    for &(bx, by) in &[(48u16, 25u16), (76, 49), (60, 73), (92, 97), (44, 101)] {
        filled_rect(&mut pixels, w, bx, by, 3, 3, &palette, 2);
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

fn fill_all(pixels: &mut [u8], w: u16, h: u16, palette: &Palette, idx: usize) {
    let [r, g, b] = palette.0[idx];
    for i in 0..(w as usize * h as usize) {
        pixels[i * 4] = r;
        pixels[i * 4 + 1] = g;
        pixels[i * 4 + 2] = b;
        pixels[i * 4 + 3] = 255;
    }
}

fn set_px(pixels: &mut [u8], w: u16, x: u16, y: u16, palette: &Palette, idx: usize) {
    if x >= w {
        return;
    }
    let pos = (y as usize * w as usize + x as usize) * 4;
    if pos + 3 >= pixels.len() {
        return;
    }
    let [r, g, b] = palette.0[idx];
    pixels[pos] = r;
    pixels[pos + 1] = g;
    pixels[pos + 2] = b;
    pixels[pos + 3] = 255;
}

fn hline(pixels: &mut [u8], w: u16, y: u16, x: u16, len: u16, palette: &Palette, idx: usize) {
    for dx in 0..len {
        set_px(pixels, w, x + dx, y, palette, idx);
    }
}

fn vline(pixels: &mut [u8], w: u16, x: u16, y: u16, len: u16, palette: &Palette, idx: usize) {
    for dy in 0..len {
        set_px(pixels, w, x, y + dy, palette, idx);
    }
}

fn filled_rect(
    pixels: &mut [u8],
    w: u16,
    x: u16,
    y: u16,
    rw: u16,
    rh: u16,
    palette: &Palette,
    idx: usize,
) {
    for dy in 0..rh {
        for dx in 0..rw {
            set_px(pixels, w, x + dx, y + dy, palette, idx);
        }
    }
}
