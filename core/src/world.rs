//! The world: nutrients, plants, creatures, and the tick that moves them.
//! Everything here is deterministic given the seed; no allocation happens in
//! the per-tick path once the capacity vectors are built.

use crate::brain::{self, forward};
use crate::fastmath::{hue_distance, sin, sin_cos, wrap_angle};
use crate::genome::{Genome, N_HID, N_IN, N_OUT, SENSE_MAX};
use crate::plants::{Nutrients, Plants};
use crate::rng::Rng;
use crate::spatial::{Hit, SpatialHash};
use crate::stats::{self, History, Tally, HUE_BINS, SAMPLE_STRIDE};

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub width: f32,
    pub height: f32,
    pub grid_cols: u32,
    pub grid_rows: u32,
    pub nutrient_capacity: f32,
    pub regrow_rate: f32,
    pub diffuse_every: u32,
    pub diffuse_k: f32,
    pub plant_cost: f32,
    pub plant_energy: f32,
    pub plant_spawn_rate: f32,
    pub plant_radius: f32,
    pub max_plants: u32,
    pub max_creatures: u32,
    pub initial_creatures: u32,
    pub initial_plants: u32,
    pub basal_cost: f32,
    pub move_cost: f32,
    pub sense_cost: f32,
    pub max_turn: f32,
    pub bite_damage: f32,
    pub bite_cost: f32,
    pub bite_size_ratio: f32,
    pub corpse_return: f32,
    pub child_share: f32,
    pub spatial_cell: f32,
    pub history_cap: u32,
    pub history_every: u32,
    /// When the population drops below this, founders wash in. 0 disables.
    pub min_population: u32,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            width: 2048.0,
            height: 2048.0,
            grid_cols: 64,
            grid_rows: 64,
            nutrient_capacity: 100.0,
            regrow_rate: 0.0015,
            diffuse_every: 8,
            diffuse_k: 0.1,
            plant_cost: 25.0,
            plant_energy: 25.0,
            plant_spawn_rate: 0.02,
            plant_radius: 2.0,
            max_plants: 40_000,
            max_creatures: 20_000,
            initial_creatures: 400,
            initial_plants: 4_000,
            basal_cost: 0.03,
            move_cost: 0.05,
            sense_cost: 0.01,
            max_turn: 0.25,
            bite_damage: 15.0,
            bite_cost: 1.0,
            bite_size_ratio: 1.1,
            corpse_return: 0.5,
            child_share: 0.4,
            spatial_cell: 32.0,
            history_cap: 2048,
            history_every: 10,
            min_population: 20,
        }
    }
}

impl Config {
    pub const N_FIELDS: usize = 30;
    /// Flat f32 encoding for snapshots. Integers survive exactly below 2^24.
    pub fn to_fields(&self) -> [f32; Self::N_FIELDS] {
        [
            self.width,
            self.height,
            self.grid_cols as f32,
            self.grid_rows as f32,
            self.nutrient_capacity,
            self.regrow_rate,
            self.diffuse_every as f32,
            self.diffuse_k,
            self.plant_cost,
            self.plant_energy,
            self.plant_spawn_rate,
            self.plant_radius,
            self.max_plants as f32,
            self.max_creatures as f32,
            self.initial_creatures as f32,
            self.initial_plants as f32,
            self.basal_cost,
            self.move_cost,
            self.sense_cost,
            self.max_turn,
            self.bite_damage,
            self.bite_cost,
            self.bite_size_ratio,
            self.corpse_return,
            self.child_share,
            self.spatial_cell,
            self.history_cap as f32,
            self.history_every as f32,
            self.min_population as f32,
            0.0,
        ]
    }
    pub fn from_fields(f: &[f32; Self::N_FIELDS]) -> Config {
        Config {
            width: f[0],
            height: f[1],
            grid_cols: f[2] as u32,
            grid_rows: f[3] as u32,
            nutrient_capacity: f[4],
            regrow_rate: f[5],
            diffuse_every: f[6] as u32,
            diffuse_k: f[7],
            plant_cost: f[8],
            plant_energy: f[9],
            plant_spawn_rate: f[10],
            plant_radius: f[11],
            max_plants: f[12] as u32,
            max_creatures: f[13] as u32,
            initial_creatures: f[14] as u32,
            initial_plants: f[15] as u32,
            basal_cost: f[16],
            move_cost: f[17],
            sense_cost: f[18],
            max_turn: f[19],
            bite_damage: f[20],
            bite_cost: f[21],
            bite_size_ratio: f[22],
            corpse_return: f[23],
            child_share: f[24],
            spatial_cell: f[25],
            history_cap: f[26] as u32,
            history_every: f[27] as u32,
            min_population: f[28] as u32,
        }
    }
}

/// Struct-of-arrays creature storage. Slots are recycled through a free list;
/// `id` is unique for the life of a world so the UI can tell reuse apart.
pub struct Creatures {
    pub alive: Vec<bool>,
    pub id: Vec<u32>,
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    pub heading: Vec<f32>,
    pub energy: Vec<f32>,
    pub age: Vec<f32>,
    pub generation: Vec<u32>,
    pub meals: Vec<u32>,
    /// Hot copies of genome fields read for *other* creatures during sensing,
    /// kept in their own arrays so neighbour lookups stay cache-friendly.
    pub size: Vec<f32>,
    pub hue: Vec<f32>,
    pub radius: Vec<f32>,
    pub genome: Vec<Genome>,
    pub inputs: Vec<[f32; N_IN]>,
    pub hidden: Vec<[f32; N_HID]>,
    pub outputs: Vec<[f32; N_OUT]>,
    free: Vec<u32>,
    count: usize,
}

impl Creatures {
    pub fn new(capacity: usize) -> Creatures {
        let blank = Genome::from_parts([1.0, 1.0, 32.0, 0.0, 0.0, 0.7, 0.05], [0.0; crate::genome::N_W]);
        Creatures {
            alive: vec![false; capacity],
            id: vec![0; capacity],
            x: vec![0.0; capacity],
            y: vec![0.0; capacity],
            heading: vec![0.0; capacity],
            energy: vec![0.0; capacity],
            age: vec![0.0; capacity],
            generation: vec![0; capacity],
            meals: vec![0; capacity],
            size: vec![1.0; capacity],
            hue: vec![0.0; capacity],
            radius: vec![6.0; capacity],
            genome: vec![blank; capacity],
            inputs: vec![[0.0; N_IN]; capacity],
            hidden: vec![[0.0; N_HID]; capacity],
            outputs: vec![[0.0; N_OUT]; capacity],
            free: (0..capacity as u32).rev().collect(),
            count: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.alive.len()
    }
    pub fn count(&self) -> usize {
        self.count
    }
    pub fn is_full(&self) -> bool {
        self.free.is_empty()
    }

    pub(crate) fn spawn(&mut self, id: u32, x: f32, y: f32, heading: f32, energy: f32, generation: u32, genome: Genome) -> Option<u32> {
        let i = self.free.pop()?;
        let u = i as usize;
        self.alive[u] = true;
        self.id[u] = id;
        self.x[u] = x;
        self.y[u] = y;
        self.heading[u] = heading;
        self.energy[u] = energy;
        self.age[u] = 0.0;
        self.generation[u] = generation;
        self.meals[u] = 0;
        self.size[u] = genome.size;
        self.hue[u] = genome.hue;
        self.radius[u] = genome.radius();
        self.genome[u] = genome;
        self.inputs[u] = [0.0; N_IN];
        self.hidden[u] = [0.0; N_HID];
        self.outputs[u] = [0.0; N_OUT];
        self.count += 1;
        Some(i)
    }

    fn kill(&mut self, i: u32) {
        let u = i as usize;
        if self.alive[u] {
            self.alive[u] = false;
            self.free.push(i);
            self.count -= 1;
        }
    }

    pub fn free_list(&self) -> &[u32] {
        &self.free
    }

    pub fn set_free_list(&mut self, free: Vec<u32>) {
        self.free = free;
        self.count = self.alive.iter().filter(|a| **a).count();
    }

    pub fn rebuild_free_list(&mut self) {
        self.free.clear();
        self.count = 0;
        for i in (0..self.alive.len()).rev() {
            if self.alive[i] {
                self.count += 1;
            } else {
                self.free.push(i as u32);
            }
        }
    }

    pub fn total_energy(&self) -> f32 {
        let mut t = 0.0;
        for i in 0..self.alive.len() {
            if self.alive[i] {
                t += self.energy[i];
            }
        }
        t
    }
}

pub struct World {
    pub cfg: Config,
    pub seed: u32,
    pub tick: u32,
    pub next_id: u32,
    pub rng: Rng,
    pub nutrients: Nutrients,
    pub plants: Plants,
    pub creatures: Creatures,
    pub history: History,
    pub tally: Tally,
    plant_hash: SpatialHash,
    creature_hash: SpatialHash,
}

#[inline]
fn wrap(v: f32, span: f32) -> f32 {
    if v < 0.0 {
        v + span
    } else if v >= span {
        v - span
    } else {
        v
    }
}

impl World {
    pub fn new(seed: u32, cfg: Config) -> World {
        let mut w = World::empty(seed, cfg);
        let mut rng = Rng::new(seed);
        for _ in 0..w.cfg.initial_plants {
            let x = rng.range(0.0, w.cfg.width);
            let y = rng.range(0.0, w.cfg.height);
            w.plants.spawn(x, y, w.cfg.plant_energy);
        }
        for _ in 0..w.cfg.initial_creatures {
            let g = Genome::random(&mut rng);
            let x = rng.range(0.0, w.cfg.width);
            let y = rng.range(0.0, w.cfg.height);
            let h = rng.range(-core::f32::consts::PI, core::f32::consts::PI);
            let e = g.max_energy() * 0.5;
            let id = w.next_id;
            w.next_id += 1;
            w.creatures.spawn(id, x, y, h, e, 0, g);
        }
        w.rng = rng;
        w
    }

    /// Allocated but unpopulated; used by `new` and by snapshot restore.
    pub fn empty(seed: u32, cfg: Config) -> World {
        let nutrients = Nutrients::new(cfg.width, cfg.height, cfg.grid_cols as usize, cfg.grid_rows as usize, cfg.nutrient_capacity);
        World {
            seed,
            tick: 0,
            next_id: 0,
            rng: Rng::new(seed),
            nutrients,
            plants: Plants::new(cfg.max_plants as usize),
            creatures: Creatures::new(cfg.max_creatures as usize),
            history: History::new(cfg.history_cap as usize, cfg.history_every),
            tally: Tally::default(),
            plant_hash: SpatialHash::new(cfg.width, cfg.height, cfg.spatial_cell),
            creature_hash: SpatialHash::new(cfg.width, cfg.height, cfg.spatial_cell),
            cfg,
        }
    }

    pub fn population(&self) -> usize {
        self.creatures.count()
    }

    pub fn step(&mut self, n: u32) {
        for _ in 0..n {
            self.tick_once();
        }
    }

    fn tick_once(&mut self) {
        self.tick += 1;
        self.grow();
        self.plant_hash.rebuild(&self.plants.x, &self.plants.y, &self.plants.alive);
        self.creature_hash.rebuild(&self.creatures.x, &self.creatures.y, &self.creatures.alive);
        self.act();
        self.reap();
        self.breed();
        self.wash_in();
        if self.tick % self.cfg.history_every == 0 {
            self.sample();
        }
    }

    fn grow(&mut self) {
        let cfg = &self.cfg;
        self.nutrients.regrow(cfg.regrow_rate);
        if self.tick % cfg.diffuse_every == 0 {
            self.nutrients.diffuse(cfg.diffuse_k);
        }
        let cols = self.nutrients.cols;
        let (cw, ch) = (self.nutrients.cell_w, self.nutrients.cell_h);
        for i in 0..self.nutrients.level.len() {
            if self.plants.is_full() {
                break;
            }
            let level = self.nutrients.level[i];
            if level < cfg.plant_cost {
                continue;
            }
            let p = cfg.plant_spawn_rate * level / self.nutrients.capacity[i];
            if self.rng.chance(p) {
                let cx = (i % cols) as f32 * cw;
                let cy = (i / cols) as f32 * ch;
                let x = cx + self.rng.next_f32() * cw;
                let y = cy + self.rng.next_f32() * ch;
                if self.plants.spawn(x, y, cfg.plant_energy).is_some() {
                    self.nutrients.level[i] = level - cfg.plant_cost;
                }
            }
        }
    }

    fn act(&mut self) {
        let cfg = &self.cfg;
        let c = &mut self.creatures;
        let plants = &mut self.plants;
        // A slow shared clock with a per-creature phase, built from integer
        // arithmetic so it stays exact however large ticks and ids grow.
        let tick = self.tick;
        for i in 0..c.capacity() {
            if !c.alive[i] {
                continue;
            }
            let g = &c.genome[i];
            let (x, y, h) = (c.x[i], c.y[i], c.heading[i]);
            let (hy, hx) = sin_cos(h);
            let radius = g.radius();
            let sense = g.sense_range;
            let max_e = g.max_energy();

            // ---- sense ----
            let plant_hit: Option<Hit> = self.plant_hash.nearest(&plants.x, &plants.y, x, y, sense, u32::MAX, |_| false);
            let other_hit: Option<Hit> = self.creature_hash.nearest(&c.x, &c.y, x, y, sense, i as u32, |_| false);
            let mut inp = [0.0f32; N_IN];
            inp[brain::IN_BIAS] = 1.0;
            if let Some(hit) = plant_hit {
                let d = hit.dist2.sqrt().max(1e-3);
                let prox = 1.0 - d / sense;
                let fwd = (hit.dx * hx + hit.dy * hy) / d;
                let left = (-hit.dx * hy + hit.dy * hx) / d;
                inp[brain::IN_PLANT_X] = fwd * prox;
                inp[brain::IN_PLANT_Y] = left * prox;
                inp[brain::IN_PLANT_PROX] = prox;
            }
            if let Some(hit) = other_hit {
                let j = hit.index as usize;
                let d = hit.dist2.sqrt().max(1e-3);
                let prox = 1.0 - d / sense;
                let fwd = (hit.dx * hx + hit.dy * hy) / d;
                let left = (-hit.dx * hy + hit.dy * hx) / d;
                inp[brain::IN_CREATURE_X] = fwd * prox;
                inp[brain::IN_CREATURE_Y] = left * prox;
                inp[brain::IN_CREATURE_PROX] = prox;
                inp[brain::IN_CREATURE_SIZE] = (c.size[j] / g.size - 1.0).clamp(-1.0, 1.0);
                inp[brain::IN_CREATURE_HUE] = hue_distance(c.hue[j], g.hue) * 2.0;
            }
            inp[brain::IN_ENERGY] = (c.energy[i] / max_e).clamp(0.0, 1.0);
            inp[brain::IN_AGE] = (c.age[i] / g.max_age()).clamp(0.0, 1.0);
            let phase = (tick.wrapping_mul(53).wrapping_add(c.id[i].wrapping_mul(40_503)) & 0xFFFF) as f32;
            inp[brain::IN_OSC] = sin(phase * (core::f32::consts::TAU / 65_536.0) - core::f32::consts::PI);

            // ---- think ----
            let mut hid = [0.0f32; N_HID];
            let mut out = [0.0f32; N_OUT];
            forward(&g.weights, &inp, &mut hid, &mut out);
            c.inputs[i] = inp;
            c.hidden[i] = hid;
            c.outputs[i] = out;

            // ---- act ----
            let turn = out[brain::OUT_TURN] * cfg.max_turn;
            let thrust = (out[brain::OUT_THRUST] + 1.0) * 0.5;
            let nh = wrap_angle(h + turn);
            let (ny_dir, nx_dir) = sin_cos(nh);
            let v = thrust * g.speed;
            let mut nx = x + nx_dir * v;
            let mut ny = y + ny_dir * v;
            // Soft bodies: if overlapping the nearest neighbour, slide apart.
            // Besides looking right, this bounds local density, which keeps
            // neighbour queries cheap no matter how creatures cluster.
            if let Some(hit) = other_hit {
                let j = hit.index as usize;
                let min_d = radius + c.radius[j];
                if hit.dist2 < min_d * min_d {
                    let d = hit.dist2.sqrt().max(1e-3);
                    let push = (min_d - d) * 0.5;
                    nx -= hit.dx / d * push;
                    ny -= hit.dy / d * push;
                }
            }
            let nx = wrap(nx, cfg.width);
            let ny = wrap(ny, cfg.height);
            c.heading[i] = nh;
            c.x[i] = nx;
            c.y[i] = ny;

            let size = g.size;
            let cost = cfg.basal_cost * size * size.sqrt()
                + cfg.move_cost * thrust * thrust * g.speed * size
                + cfg.sense_cost * (sense / SENSE_MAX);
            let mut energy = c.energy[i] - cost;
            c.age[i] += 1.0;

            // Eat a plant we are touching.
            if let Some(hit) = plant_hit {
                let reach = radius + cfg.plant_radius;
                if hit.dist2 <= reach * reach && plants.alive[hit.index as usize] {
                    let gain = plants.energy[hit.index as usize] * (1.0 - g.diet);
                    plants.kill(hit.index);
                    energy = (energy + gain).min(max_e);
                    c.meals[i] += 1;
                }
            }
            // Bite a smaller neighbour if the brain says so.
            if out[brain::OUT_BITE] > 0.0 {
                if let Some(hit) = other_hit {
                    let j = hit.index as usize;
                    let reach = radius + c.radius[j];
                    if hit.dist2 <= reach * reach && c.size[j] <= size * cfg.bite_size_ratio && c.alive[j] {
                        energy -= cfg.bite_cost;
                        let dmg = (cfg.bite_damage * size).min(c.energy[j].max(0.0));
                        c.energy[j] -= dmg;
                        energy = (energy + dmg * g.diet).min(max_e);
                        if dmg > 0.0 {
                            c.meals[i] += 1;
                        }
                    }
                }
            }
            c.energy[i] = energy;

            // Food-approach score: how much of the thrust points at the target
            // this creature would most want (plants when herbivorous, prey otherwise).
            let target = if g.diet < 0.5 { plant_hit } else { other_hit };
            if let Some(hit) = target {
                let d = hit.dist2.sqrt().max(1e-3);
                let fwd = (hit.dx * nx_dir + hit.dy * ny_dir) / d;
                self.tally.approach_sum += fwd * thrust;
                self.tally.approach_n += 1;
            }
        }
    }

    fn reap(&mut self) {
        let cfg = &self.cfg;
        let c = &mut self.creatures;
        for i in 0..c.capacity() {
            if !c.alive[i] {
                continue;
            }
            let dead = c.energy[i] <= 0.0 || c.age[i] >= c.genome[i].max_age();
            if dead {
                let back = c.energy[i].max(0.0) * cfg.corpse_return;
                if back > 0.0 {
                    self.nutrients.deposit(c.x[i], c.y[i], back);
                }
                c.kill(i as u32);
                self.tally.deaths += 1;
            }
        }
    }

    fn breed(&mut self) {
        let cap = self.creatures.capacity();
        for i in 0..cap {
            if !self.creatures.alive[i] || self.creatures.is_full() {
                continue;
            }
            let g = &self.creatures.genome[i];
            if self.creatures.energy[i] < g.reproduce_at * g.max_energy() {
                continue;
            }
            let child_e = self.creatures.energy[i] * self.cfg.child_share;
            self.creatures.energy[i] -= child_e;
            let child = g.mutate(&mut self.rng);
            let ang = self.rng.range(-core::f32::consts::PI, core::f32::consts::PI);
            let (sy, sx) = sin_cos(ang);
            let dist = g.radius() * 2.5;
            let x = wrap(self.creatures.x[i] + sx * dist, self.cfg.width);
            let y = wrap(self.creatures.y[i] + sy * dist, self.cfg.height);
            let gen = self.creatures.generation[i] + 1;
            let e = child_e.min(child.max_energy());
            let id = self.next_id;
            self.next_id += 1;
            self.creatures.spawn(id, x, y, ang, e, gen, child);
            self.tally.births += 1;
        }
    }

    /// Founders wash in when the dish is nearly empty (configurable).
    fn wash_in(&mut self) {
        let min = self.cfg.min_population as usize;
        if min == 0 || self.creatures.count() >= min {
            return;
        }
        let need = min - self.creatures.count();
        for _ in 0..need {
            let g = Genome::random(&mut self.rng);
            let x = self.rng.range(0.0, self.cfg.width);
            let y = self.rng.range(0.0, self.cfg.height);
            let h = self.rng.range(-core::f32::consts::PI, core::f32::consts::PI);
            let e = g.max_energy() * 0.5;
            let id = self.next_id;
            self.next_id += 1;
            self.creatures.spawn(id, x, y, h, e, 0, g);
        }
    }

    /// Drop `n` fresh founders in. Exposed for the UI's "add life" button.
    pub fn spawn_founders(&mut self, n: u32) {
        for _ in 0..n {
            if self.creatures.is_full() {
                break;
            }
            let g = Genome::random(&mut self.rng);
            let x = self.rng.range(0.0, self.cfg.width);
            let y = self.rng.range(0.0, self.cfg.height);
            let h = self.rng.range(-core::f32::consts::PI, core::f32::consts::PI);
            let e = g.max_energy() * 0.5;
            let id = self.next_id;
            self.next_id += 1;
            self.creatures.spawn(id, x, y, h, e, 0, g);
        }
    }

    fn sample(&mut self) {
        let c = &self.creatures;
        let mut row = [0.0f32; SAMPLE_STRIDE];
        let mut n = 0u32;
        let (mut herb, mut omni, mut carn) = (0u32, 0u32, 0u32);
        let (mut s_size, mut s_speed, mut s_sense, mut s_diet) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        let mut max_gen = 0u32;
        for i in 0..c.capacity() {
            if !c.alive[i] {
                continue;
            }
            let g = &c.genome[i];
            n += 1;
            if g.diet < 0.33 {
                herb += 1;
            } else if g.diet < 0.66 {
                omni += 1;
            } else {
                carn += 1;
            }
            s_size += g.size;
            s_speed += g.speed;
            s_sense += g.sense_range;
            s_diet += g.diet;
            max_gen = max_gen.max(c.generation[i]);
            let bin = ((g.hue * HUE_BINS as f32) as usize).min(HUE_BINS - 1);
            row[stats::N_FIELDS + bin] += 1.0;
        }
        let inv = if n > 0 { 1.0 / n as f32 } else { 0.0 };
        row[stats::F_TICK] = self.tick as f32;
        row[stats::F_POP] = n as f32;
        row[stats::F_PLANTS] = self.plants.count() as f32;
        row[stats::F_HERB] = herb as f32;
        row[stats::F_OMNI] = omni as f32;
        row[stats::F_CARN] = carn as f32;
        row[stats::F_BIRTHS] = self.tally.births as f32;
        row[stats::F_DEATHS] = self.tally.deaths as f32;
        row[stats::F_APPROACH] = if self.tally.approach_n > 0 { self.tally.approach_sum / self.tally.approach_n as f32 } else { 0.0 };
        row[stats::F_MEAN_SIZE] = s_size * inv;
        row[stats::F_MEAN_SPEED] = s_speed * inv;
        row[stats::F_MEAN_SENSE] = s_sense * inv;
        row[stats::F_MEAN_DIET] = s_diet * inv;
        row[stats::F_NUTRIENTS] = self.nutrients.total();
        row[stats::F_GENERATION] = max_gen as f32;
        self.history.push(&row);
        self.tally = Tally::default();
    }

    /// One tick with per-phase timings in seconds: grow, hash, act, reap, breed, sample.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn profile_tick(&mut self) -> [f64; 6] {
        use std::time::Instant;
        let mut out = [0.0; 6];
        let t = Instant::now();
        self.tick += 1;
        self.grow();
        out[0] = t.elapsed().as_secs_f64();
        let t = Instant::now();
        self.plant_hash.rebuild(&self.plants.x, &self.plants.y, &self.plants.alive);
        self.creature_hash.rebuild(&self.creatures.x, &self.creatures.y, &self.creatures.alive);
        out[1] = t.elapsed().as_secs_f64();
        let t = Instant::now();
        self.act();
        out[2] = t.elapsed().as_secs_f64();
        let t = Instant::now();
        self.reap();
        out[3] = t.elapsed().as_secs_f64();
        let t = Instant::now();
        self.breed();
        self.wash_in();
        out[4] = t.elapsed().as_secs_f64();
        let t = Instant::now();
        if self.tick % self.cfg.history_every == 0 {
            self.sample();
        }
        out[5] = t.elapsed().as_secs_f64();
        out
    }

    /// Nearest living creature to (x, y) within `r`, as a slot index.
    pub fn pick(&self, x: f32, y: f32, r: f32) -> Option<u32> {
        // The hash is one tick stale at most; good enough for a click.
        self.creature_hash
            .nearest(&self.creatures.x, &self.creatures.y, x, y, r, u32::MAX, |i| !self.creatures.alive[i as usize])
            .map(|h| h.index)
    }

    pub fn total_energy(&self) -> f32 {
        self.nutrients.total() + self.plants.total_energy() + self.creatures.total_energy()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small() -> Config {
        Config {
            width: 512.0,
            height: 512.0,
            grid_cols: 16,
            grid_rows: 16,
            max_plants: 4_000,
            max_creatures: 2_000,
            initial_creatures: 150,
            initial_plants: 800,
            ..Config::default()
        }
    }

    #[test]
    fn runs_and_keeps_counts_consistent() {
        let mut w = World::new(1, small());
        w.step(300);
        let live = w.creatures.alive.iter().filter(|a| **a).count();
        assert_eq!(live, w.population());
        let plants = w.plants.alive.iter().filter(|a| **a).count();
        assert_eq!(plants, w.plants.count());
        assert!(w.population() > 0);
    }

    #[test]
    fn no_energy_from_nothing_when_regrowth_is_off() {
        let mut cfg = small();
        cfg.regrow_rate = 0.0;
        cfg.min_population = 0;
        let mut w = World::new(2, cfg);
        let mut prev = w.total_energy();
        for _ in 0..200 {
            w.step(1);
            let now = w.total_energy();
            assert!(now <= prev + 1e-2, "energy rose {prev} -> {now}");
            prev = now;
        }
    }

    #[test]
    fn positions_stay_in_bounds() {
        let mut w = World::new(3, small());
        w.step(500);
        for i in 0..w.creatures.capacity() {
            if w.creatures.alive[i] {
                assert!((0.0..512.0).contains(&w.creatures.x[i]));
                assert!((0.0..512.0).contains(&w.creatures.y[i]));
            }
        }
    }
}
