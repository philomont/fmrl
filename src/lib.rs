pub mod age;
pub mod decode;
pub mod decay;
pub mod encode;
pub mod error;
pub mod format;
pub mod prng;

#[cfg(feature = "wasm")]
pub mod wasm;

pub use age::age_step;
pub use decode::{DecodedFmrl, TileData, decode, patch_age_chunk};
pub use encode::{FmrlImage, encode};
pub use error::FmrlError;
pub use format::{Palette, AgeEntry};

use format::TILE_SIZE;

/// Get current time in milliseconds since Unix epoch.
/// Gated by feature flag: native uses SystemTime, WASM uses js_sys::Date.
#[cfg(not(feature = "wasm"))]
pub fn now_ms() -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(feature = "wasm")]
pub fn now_ms() -> u64 {
    js_sys::Date::now() as u64
}

/// Render an already-decoded `.fmrl` image to RGBA pixels, and mutate the
/// AGE chunk in `file_bytes` in place (updating `last_view`, `fade_level`,
/// `edge_damage`).
///
/// Returns `width * height * 4` RGBA bytes.
///
/// The caller is responsible for persisting `file_bytes` to disk (or sending
/// them back to JavaScript via `FmrlView::get_mutated_bytes()`).
pub fn render(
    decoded: &mut DecodedFmrl,
    now_ms: u64,
    file_bytes: &mut [u8],
) -> Result<Vec<u8>, FmrlError> {
    let w = decoded.ihdr.width as usize;
    let h = decoded.ihdr.height as usize;
    let decay_policy = decoded.ihdr.decay_policy;

    let mut rgba = vec![0u8; w * h * 4];

    for tile_idx in 0..decoded.tiles.len() {
        let tile = &decoded.tiles[tile_idx];
        let tx = tile.tx as usize;
        let ty = tile.ty as usize;

        // Render tile
        let tile_rgba = decay::render_tile_with_policy(
            tile,
            &decoded.age[tile_idx],
            &decoded.palette,
            now_ms,
            decay_policy,
        );

        // Blit tile pixels into output buffer
        for py in 0..TILE_SIZE {
            let dst_y = ty * TILE_SIZE + py;
            let dst_x_start = tx * TILE_SIZE;
            let dst_base = (dst_y * w + dst_x_start) * 4;
            let src_base = py * TILE_SIZE * 4;
            rgba[dst_base..dst_base + TILE_SIZE * 4]
                .copy_from_slice(&tile_rgba[src_base..src_base + TILE_SIZE * 4]);
        }

        // Mutate age entry
        decay::mutate_age(&mut decoded.age[tile_idx], now_ms);
    }

    // Patch AGE chunk in file_bytes
    patch_age_chunk(file_bytes, &decoded.age_chunk_range, &decoded.age);

    Ok(rgba)
}
