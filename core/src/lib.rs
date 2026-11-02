//! Tidepool core: a deterministic evolving ecosystem.
//! Pure simulation in the modules below; `wasm` holds the JS-facing facade.

pub mod brain;
pub mod fastmath;
pub mod genome;
pub mod rng;
pub mod spatial;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn hello(seed: u32) -> u32 {
    seed.wrapping_mul(2654435761)
}
