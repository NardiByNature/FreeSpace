//! Heritable traits plus the brain's weights. Everything an offspring copies
//! from its parent lives here; `mutate` is the only source of variation.

use crate::rng::Rng;

pub const N_IN: usize = 12;
pub const N_HID: usize = 8;
pub const N_OUT: usize = 3;
/// Input->hidden weights, then (hidden + 1 bias)->output weights.
pub const N_W: usize = N_IN * N_HID + (N_HID + 1) * N_OUT;

pub const SIZE_MIN: f32 = 0.6;
pub const SIZE_MAX: f32 = 2.5;
pub const SPEED_MIN: f32 = 0.5;
pub const SPEED_MAX: f32 = 3.0;
pub const SENSE_MIN: f32 = 16.0;
pub const SENSE_MAX: f32 = 96.0;
pub const REPRO_MIN: f32 = 0.4;
pub const REPRO_MAX: f32 = 0.95;
pub const MUT_MIN: f32 = 0.005;
pub const MUT_MAX: f32 = 0.2;
pub const W_LIMIT: f32 = 4.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Genome {
    pub size: f32,
    pub speed: f32,
    pub sense_range: f32,
    /// Visual only. Drifts under mutation, so lineages become colour bands.
    pub hue: f32,
    /// 0 = herbivore, 1 = carnivore. Scales digestion efficiency of plants vs meat.
    pub diet: f32,
    /// Fraction of max energy at which the creature splits.
    pub reproduce_at: f32,
    pub mutation_rate: f32,
    pub weights: [f32; N_W],
}

#[inline]
fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

#[inline]
fn wrap01(v: f32) -> f32 {
    let mut v = v;
    while v >= 1.0 {
        v -= 1.0;
    }
    while v < 0.0 {
        v += 1.0;
    }
    v
}

impl Genome {
    /// A founder: moderate traits, random colour, small random weights.
    pub fn random(rng: &mut Rng) -> Genome {
        let mut weights = [0.0f32; N_W];
        for w in weights.iter_mut() {
            *w = rng.gaussian() * 0.5;
        }
        Genome {
            size: rng.range(0.8, 1.3),
            speed: rng.range(0.8, 1.8),
            sense_range: rng.range(30.0, 70.0),
            hue: rng.next_f32(),
            diet: rng.range(0.0, 0.3),
            reproduce_at: rng.range(0.6, 0.85),
            mutation_rate: rng.range(0.02, 0.08),
            weights,
        }
    }

    /// Copy with noise. `mutation_rate` scales every perturbation and is
    /// itself heritable, so lineages can tune how fast they explore.
    pub fn mutate(&self, rng: &mut Rng) -> Genome {
        let m = self.mutation_rate;
        let k = m / 0.05; // 1.0 at the founder default
        let mut child = self.clone();
        child.size = clamp(self.size + rng.gaussian() * 0.05 * k, SIZE_MIN, SIZE_MAX);
        child.speed = clamp(self.speed + rng.gaussian() * 0.06 * k, SPEED_MIN, SPEED_MAX);
        child.sense_range = clamp(self.sense_range + rng.gaussian() * 2.5 * k, SENSE_MIN, SENSE_MAX);
        child.hue = wrap01(self.hue + rng.gaussian() * 0.015 * k);
        child.diet = clamp(self.diet + rng.gaussian() * 0.03 * k, 0.0, 1.0);
        child.reproduce_at = clamp(self.reproduce_at + rng.gaussian() * 0.02 * k, REPRO_MIN, REPRO_MAX);
        child.mutation_rate = clamp(self.mutation_rate * (1.0 + rng.gaussian() * 0.1), MUT_MIN, MUT_MAX);
        for w in child.weights.iter_mut() {
            // Most weights are left alone; a few get nudged; rarely one is reset.
            let roll = rng.next_f32();
            if roll < m * 4.0 {
                *w = clamp(*w + rng.gaussian() * 0.3 * k, -W_LIMIT, W_LIMIT);
            } else if roll < m * 4.0 + 0.002 {
                *w = rng.gaussian() * 0.5;
            }
        }
        child
    }

    pub fn max_energy(&self) -> f32 {
        100.0 * self.size
    }

    /// Body radius in world units.
    pub fn radius(&self) -> f32 {
        3.0 + 3.0 * self.size
    }

    /// Ticks until death from old age.
    pub fn max_age(&self) -> f32 {
        5_000.0 + 2_000.0 * self.size
    }

    /// Flat trait vector for snapshots and the inspector.
    pub const N_TRAITS: usize = 7;
    pub fn traits(&self) -> [f32; Self::N_TRAITS] {
        [
            self.size,
            self.speed,
            self.sense_range,
            self.hue,
            self.diet,
            self.reproduce_at,
            self.mutation_rate,
        ]
    }
    pub fn from_parts(t: [f32; Self::N_TRAITS], weights: [f32; N_W]) -> Genome {
        Genome {
            size: t[0],
            speed: t[1],
            sense_range: t[2],
            hue: t[3],
            diet: t[4],
            reproduce_at: t[5],
            mutation_rate: t[6],
            weights,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_stays_within_bounds() {
        let mut rng = Rng::new(5);
        let mut g = Genome::random(&mut rng);
        g.mutation_rate = MUT_MAX; // worst case
        for _ in 0..5_000 {
            g = g.mutate(&mut rng);
            assert!((SIZE_MIN..=SIZE_MAX).contains(&g.size));
            assert!((SPEED_MIN..=SPEED_MAX).contains(&g.speed));
            assert!((SENSE_MIN..=SENSE_MAX).contains(&g.sense_range));
            assert!((0.0..1.0).contains(&g.hue));
            assert!((0.0..=1.0).contains(&g.diet));
            assert!((REPRO_MIN..=REPRO_MAX).contains(&g.reproduce_at));
            assert!((MUT_MIN..=MUT_MAX).contains(&g.mutation_rate));
            for w in g.weights {
                assert!(w.abs() <= W_LIMIT);
            }
        }
    }

    #[test]
    fn mutation_changes_something_but_not_everything() {
        let mut rng = Rng::new(6);
        let g = Genome::random(&mut rng);
        let c = g.mutate(&mut rng);
        let changed = g.weights.iter().zip(c.weights.iter()).filter(|(a, b)| a != b).count();
        assert!(changed > 0 && changed < N_W / 2, "changed {changed}");
    }

    #[test]
    fn parts_round_trip() {
        let mut rng = Rng::new(8);
        let g = Genome::random(&mut rng);
        assert_eq!(Genome::from_parts(g.traits(), g.weights), g);
    }
}
