#![cfg(feature = "wasm")]

use wasm_bindgen::prelude::*;

use crate::decode::{DecodedFmrl, decode};
use crate::render;

#[wasm_bindgen]
pub struct FmrlView {
    file_bytes: Vec<u8>,
    decoded: DecodedFmrl,
}

#[wasm_bindgen]
impl FmrlView {
    /// Create a new `FmrlView` from raw `.fmrl` file bytes.
    pub fn new(data: &[u8]) -> Result<FmrlView, JsValue> {
        let decoded = decode(data).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(FmrlView {
            file_bytes: data.to_vec(),
            decoded,
        })
    }

    /// Decode and apply decay. Returns RGBA pixel bytes (width * height * 4).
    /// Also mutates `file_bytes` (AGE chunk updated). Call `get_mutated_bytes()` to persist.
    pub fn decode_and_decay(&mut self) -> Result<Vec<u8>, JsValue> {
        let now = js_sys::Date::now() as u64;
        render(&mut self.decoded, now, &mut self.file_bytes)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Return the current (mutated) file bytes for persistence.
    pub fn get_mutated_bytes(&self) -> Vec<u8> {
        self.file_bytes.clone()
    }

    /// Total number of tiles (proxy for view_count since each render mutates all tiles).
    pub fn view_count(&self) -> usize {
        // Use fade_level of tile 0 as a proxy for view count
        self.decoded.age.first().map(|a| a.fade_level as usize).unwrap_or(0)
    }

    pub fn width(&self) -> u16 {
        self.decoded.ihdr.width
    }

    pub fn height(&self) -> u16 {
        self.decoded.ihdr.height
    }
}
