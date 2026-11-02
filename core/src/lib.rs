//! Tidepool core: a deterministic evolving ecosystem.
//!
//! The simulation modules are pure Rust with no JS dependency; `Sim` below is
//! the thin wasm-bindgen facade the TypeScript renderer talks to. Bulk data
//! (creatures, plants, nutrients, history) is exposed as pointers into linear
//! memory so the renderer reads it through typed-array views without copies.

pub mod brain;
pub mod codec;
pub mod fastmath;
pub mod genome;
pub mod plants;
pub mod rng;
pub mod snapshot;
pub mod spatial;
pub mod stats;
pub mod world;

use genome::{Genome, N_HID, N_IN, N_OUT, N_W};
use wasm_bindgen::prelude::*;
use world::{Config, World};

/// Packed creature row: x, y, heading, radius, hue, energy fraction, diet, slot.
pub const CREATURE_STRIDE: usize = 8;
/// Packed plant row: x, y, energy.
pub const PLANT_STRIDE: usize = 3;

// Offsets into the `inspect` row.
pub const I_ID: usize = 0;
pub const I_GENERATION: usize = 1;
pub const I_AGE: usize = 2;
pub const I_MAX_AGE: usize = 3;
pub const I_ENERGY: usize = 4;
pub const I_MAX_ENERGY: usize = 5;
pub const I_MEALS: usize = 6;
pub const I_RADIUS: usize = 7;
pub const I_TRAITS: usize = 8;
pub const I_INPUTS: usize = I_TRAITS + Genome::N_TRAITS;
pub const I_HIDDEN: usize = I_INPUTS + N_IN;
pub const I_OUTPUTS: usize = I_HIDDEN + N_HID;
pub const I_WEIGHTS: usize = I_OUTPUTS + N_OUT;
pub const INSPECT_LEN: usize = I_WEIGHTS + N_W;

#[wasm_bindgen]
pub struct Sim {
    world: World,
    creature_view: Vec<f32>,
    plant_view: Vec<f32>,
}

#[wasm_bindgen]
impl Sim {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32, sexual: bool) -> Sim {
        let cfg = Config { sexual, ..Config::default() };
        Sim::wrap(World::new(seed, cfg))
    }

    pub fn sexual(&self) -> bool {
        self.world.cfg.sexual
    }

    fn wrap(world: World) -> Sim {
        Sim { world, creature_view: Vec::new(), plant_view: Vec::new() }
    }

    /// Rebuild a world from `snapshot()` bytes.
    pub fn restore(bytes: &[u8]) -> Result<Sim, JsError> {
        World::restore(bytes).map(Sim::wrap).map_err(|e| JsError::new(&e))
    }

    pub fn snapshot(&self) -> Vec<u8> {
        self.world.snapshot()
    }

    pub fn step(&mut self, n: u32) {
        self.world.step(n);
    }

    pub fn tick(&self) -> u32 {
        self.world.tick
    }
    pub fn seed(&self) -> u32 {
        self.world.seed
    }
    pub fn width(&self) -> f32 {
        self.world.cfg.width
    }
    pub fn height(&self) -> f32 {
        self.world.cfg.height
    }
    pub fn population(&self) -> u32 {
        self.world.population() as u32
    }
    pub fn plant_count(&self) -> u32 {
        self.world.plants.count() as u32
    }
    pub fn max_creatures(&self) -> u32 {
        self.world.cfg.max_creatures
    }
    pub fn plant_max_energy(&self) -> f32 {
        self.world.cfg.plant_max_energy
    }
    pub fn plant_radius(&self) -> f32 {
        self.world.cfg.plant_radius
    }
    pub fn max_generation(&self) -> u32 {
        let c = &self.world.creatures;
        (0..c.capacity()).filter(|&i| c.alive[i]).map(|i| c.generation[i]).max().unwrap_or(0)
    }

    // ---- nutrient grid ----
    pub fn grid_cols(&self) -> u32 {
        self.world.nutrients.cols as u32
    }
    pub fn grid_rows(&self) -> u32 {
        self.world.nutrients.rows as u32
    }
    pub fn nutrient_capacity(&self) -> f32 {
        self.world.cfg.nutrient_capacity
    }
    pub fn nutrients_ptr(&self) -> *const f32 {
        self.world.nutrients.level.as_ptr()
    }

    // ---- packed render views ----
    /// Pack live creatures and plants into contiguous rows. Call once per frame.
    pub fn sync(&mut self) {
        let c = &self.world.creatures;
        self.creature_view.clear();
        for i in 0..c.capacity() {
            if !c.alive[i] {
                continue;
            }
            let g = &c.genome[i];
            self.creature_view.extend_from_slice(&[
                c.x[i],
                c.y[i],
                c.heading[i],
                c.radius[i],
                c.hue[i],
                (c.energy[i] / g.max_energy()).clamp(0.0, 1.0),
                g.diet,
                i as f32,
            ]);
        }
        let p = &self.world.plants;
        self.plant_view.clear();
        for i in 0..p.capacity() {
            if p.alive[i] {
                self.plant_view.extend_from_slice(&[p.x[i], p.y[i], p.energy[i]]);
            }
        }
    }
    pub fn creatures_ptr(&self) -> *const f32 {
        self.creature_view.as_ptr()
    }
    pub fn creatures_len(&self) -> u32 {
        (self.creature_view.len() / CREATURE_STRIDE) as u32
    }
    pub fn plants_ptr(&self) -> *const f32 {
        self.plant_view.as_ptr()
    }
    pub fn plants_len(&self) -> u32 {
        (self.plant_view.len() / PLANT_STRIDE) as u32
    }

    // ---- selection and inspection ----
    /// Slot of the nearest live creature within `r`, or -1.
    pub fn pick(&self, x: f32, y: f32, r: f32) -> i32 {
        self.world.pick(x, y, r).map(|s| s as i32).unwrap_or(-1)
    }
    /// Slot of a record holder (0 oldest, 1 most meals, 2 highest generation), or -1.
    pub fn champion(&self, kind: u32) -> i32 {
        self.world.champion(kind).map(|s| s as i32).unwrap_or(-1)
    }
    pub fn creature_alive(&self, slot: u32) -> bool {
        let c = &self.world.creatures;
        (slot as usize) < c.capacity() && c.alive[slot as usize]
    }
    pub fn creature_id(&self, slot: u32) -> u32 {
        let c = &self.world.creatures;
        if (slot as usize) < c.capacity() {
            c.id[slot as usize]
        } else {
            0
        }
    }
    /// Everything about one creature, as a flat row (see the `I_*` offsets).
    pub fn inspect(&self, slot: u32) -> Vec<f32> {
        let c = &self.world.creatures;
        let i = slot as usize;
        let mut out = vec![0.0f32; INSPECT_LEN];
        if i >= c.capacity() || !c.alive[i] {
            return out;
        }
        let g = &c.genome[i];
        out[I_ID] = c.id[i] as f32;
        out[I_GENERATION] = c.generation[i] as f32;
        out[I_AGE] = c.age[i];
        out[I_MAX_AGE] = g.max_age();
        out[I_ENERGY] = c.energy[i];
        out[I_MAX_ENERGY] = g.max_energy();
        out[I_MEALS] = c.meals[i] as f32;
        out[I_RADIUS] = c.radius[i];
        out[I_TRAITS..I_INPUTS].copy_from_slice(&g.traits());
        out[I_INPUTS..I_HIDDEN].copy_from_slice(&c.inputs[i]);
        out[I_HIDDEN..I_OUTPUTS].copy_from_slice(&c.hidden[i]);
        out[I_OUTPUTS..I_WEIGHTS].copy_from_slice(&c.outputs[i]);
        out[I_WEIGHTS..].copy_from_slice(&g.weights);
        out
    }

    // ---- history ----
    pub fn history_ptr(&self) -> *const f32 {
        self.world.history.data.as_ptr()
    }
    pub fn history_cap(&self) -> u32 {
        self.world.history.cap as u32
    }
    pub fn history_len(&self) -> u32 {
        self.world.history.len as u32
    }
    pub fn history_head(&self) -> u32 {
        self.world.history.head as u32
    }
    pub fn history_every(&self) -> u32 {
        self.world.history.every
    }

    // ---- chronicle ----
    /// Monotonic count of events ever recorded; poll it to notice new ones.
    pub fn event_total(&self) -> u32 {
        self.world.chronicle.total
    }
    /// The newest `n` events, oldest first, as flat [tick, kind, a, b] rows.
    pub fn events(&self, n: u32) -> Vec<f32> {
        let c = &self.world.chronicle;
        let n = (n as usize).min(c.len);
        let mut out = Vec::with_capacity(n * 4);
        for i in (c.len - n)..c.len {
            let e = c.event(i);
            out.extend_from_slice(&[e.tick as f32, e.kind as f32, e.a, e.b]);
        }
        out
    }

    // ---- interventions ----
    pub fn spawn_founders(&mut self, n: u32) {
        self.world.spawn_founders(n);
    }

    // ---- specimens ----
    /// A creature's genome as a portable code, or "" if the slot is empty.
    pub fn genome_code(&self, slot: u32) -> String {
        let c = &self.world.creatures;
        let i = slot as usize;
        if i >= c.capacity() || !c.alive[i] {
            return String::new();
        }
        codec::encode_genome(&c.genome[i])
    }
    /// Drop `n` copies of a coded genome in at random positions; returns how many fit (0 for a bad code).
    pub fn spawn_code(&mut self, code: &str, n: u32) -> u32 {
        match codec::decode_genome(code) {
            Ok(g) => self.world.spawn_genome(&g, n),
            Err(_) => 0,
        }
    }
    /// The 7 traits of a coded genome, or an empty array for a bad code.
    pub fn code_traits(&self, code: &str) -> Vec<f32> {
        codec::decode_genome(code).map(|g| g.traits().to_vec()).unwrap_or_default()
    }

    // ---- layout metadata so the TS side never hard-codes sizes ----
    /// [creature stride, plant stride, N_IN, N_HID, N_OUT, N_W, N_TRAITS,
    ///  stats fields, hue bins, sample stride, inspect len,
    ///  I_TRAITS, I_INPUTS, I_HIDDEN, I_OUTPUTS, I_WEIGHTS]
    pub fn layout() -> Vec<u32> {
        vec![
            CREATURE_STRIDE as u32,
            PLANT_STRIDE as u32,
            N_IN as u32,
            N_HID as u32,
            N_OUT as u32,
            N_W as u32,
            Genome::N_TRAITS as u32,
            stats::N_FIELDS as u32,
            stats::HUE_BINS as u32,
            stats::SAMPLE_STRIDE as u32,
            INSPECT_LEN as u32,
            I_TRAITS as u32,
            I_INPUTS as u32,
            I_HIDDEN as u32,
            I_OUTPUTS as u32,
            I_WEIGHTS as u32,
        ]
    }
    pub fn input_names() -> String {
        brain::INPUT_NAMES.join("|")
    }
    pub fn output_names() -> String {
        brain::OUTPUT_NAMES.join("|")
    }
    pub fn trait_names() -> String {
        "size|speed|sense|hue|diet|breed at|mutation|mate pref".to_string()
    }
}
