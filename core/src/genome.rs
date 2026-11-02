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
    /// How alike in colour a partner must be (sexual worlds only).
    /// 0 accepts anyone; near 1 accepts only a near-identical hue.
    pub mate_pref: f32,
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
            mate_pref: rng.range(0.0, 0.5),
            weights,
        }
    }

    /// Would a creature with this genome accept `other` as a mate?
    #[inline]
    pub fn accepts(&self, other: &Genome) -> bool {
        let tol = 1.0 - self.mate_pref;
        crate::fastmath::hue_distance(self.hue, other.hue) <= 0.5 * tol * tol
    }

    /// Recombine two parents. The brain has structure, so each hidden unit
    /// travels as a block (its input weights plus its output weights) from
    /// one parent; output biases come from one parent each. Traits pick a
    /// parent at random, except hue, which is the circular midpoint so colour
    /// behaves like a blended quantitative trait.
    pub fn crossover(a: &Genome, b: &Genome, rng: &mut Rng) -> Genome {
        let pick = |rng: &mut Rng, x: f32, y: f32| if rng.chance(0.5) { x } else { y };
        let mut weights = [0.0f32; N_W];
        let out_base = N_IN * N_HID;
        for h in 0..N_HID {
            let src = if rng.chance(0.5) { a } else { b };
            weights[h * N_IN..(h + 1) * N_IN].copy_from_slice(&src.weights[h * N_IN..(h + 1) * N_IN]);
            for o in 0..N_OUT {
                let k = out_base + o * (N_HID + 1) + h;
                weights[k] = src.weights[k];
            }
        }
        for o in 0..N_OUT {
            let k = out_base + o * (N_HID + 1) + N_HID;
            weights[k] = pick(rng, a.weights[k], b.weights[k]);
        }
        let mut d = b.hue - a.hue;
        if d > 0.5 {
            d -= 1.0;
        } else if d < -0.5 {
            d += 1.0;
        }
        Genome {
            size: pick(rng, a.size, b.size),
            speed: pick(rng, a.speed, b.speed),
            sense_range: pick(rng, a.sense_range, b.sense_range),
            hue: wrap01(a.hue + d * 0.5),
            diet: pick(rng, a.diet, b.diet),
            reproduce_at: pick(rng, a.reproduce_at, b.reproduce_at),
            mutation_rate: pick(rng, a.mutation_rate, b.mutation_rate),
            mate_pref: pick(rng, a.mate_pref, b.mate_pref),
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
        child.mate_pref = clamp(self.mate_pref + rng.gaussian() * 0.04 * k, 0.0, 1.0);
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

    /// Pull every field into its legal range (used when decoding codes).
    pub fn clamped(mut self) -> Genome {
        self.size = clamp(self.size, SIZE_MIN, SIZE_MAX);
        self.speed = clamp(self.speed, SPEED_MIN, SPEED_MAX);
        self.sense_range = clamp(self.sense_range, SENSE_MIN, SENSE_MAX);
        self.hue = wrap01(self.hue);
        self.diet = clamp(self.diet, 0.0, 1.0);
        self.reproduce_at = clamp(self.reproduce_at, REPRO_MIN, REPRO_MAX);
        self.mutation_rate = clamp(self.mutation_rate, MUT_MIN, MUT_MAX);
        self.mate_pref = clamp(self.mate_pref, 0.0, 1.0);
        for w in self.weights.iter_mut() {
            *w = clamp(*w, -W_LIMIT, W_LIMIT);
        }
        self
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
    pub const N_TRAITS: usize = 8;
    pub fn traits(&self) -> [f32; Self::N_TRAITS] {
        [
            self.size,
            self.speed,
            self.sense_range,
            self.hue,
            self.diet,
            self.reproduce_at,
            self.mutation_rate,
            self.mate_pref,
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
            mate_pref: t[7],
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
            assert!((0.0..=1.0).contains(&g.mate_pref));
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
    fn crossover_takes_blocks_from_parents() {
        let mut rng = Rng::new(10);
        let a = Genome::random(&mut rng);
        let b = Genome::random(&mut rng);
        for _ in 0..50 {
            let c = Genome::crossover(&a, &b, &mut rng);
            for i in 0..N_W {
                assert!(c.weights[i] == a.weights[i] || c.weights[i] == b.weights[i], "weight {i} from nowhere");
            }
            // Each hidden unit's input row is wholly from one parent.
            for h in 0..N_HID {
                let row = &c.weights[h * N_IN..(h + 1) * N_IN];
                let from_a = row == &a.weights[h * N_IN..(h + 1) * N_IN];
                let from_b = row == &b.weights[h * N_IN..(h + 1) * N_IN];
                assert!(from_a || from_b);
            }
            assert!(c.size == a.size || c.size == b.size);
            assert!(c.diet == a.diet || c.diet == b.diet);
            assert!((0.0..1.0).contains(&c.hue));
            assert!(crate::fastmath::hue_distance(c.hue, a.hue) <= crate::fastmath::hue_distance(a.hue, b.hue) * 0.5 + 1e-5);
        }
    }

    #[test]
    fn mate_acceptance_depends_on_preference() {
        let mut rng = Rng::new(12);
        let mut a = Genome::random(&mut rng);
        let mut b = Genome::random(&mut rng);
        a.hue = 0.2;
        b.hue = 0.45;
        a.mate_pref = 0.0;
        assert!(a.accepts(&b));
        a.mate_pref = 0.5; // tolerance 0.125
        assert!(!a.accepts(&b));
        b.hue = 0.3;
        assert!(a.accepts(&b));
    }

    #[test]
    fn parts_round_trip() {
        let mut rng = Rng::new(8);
        let g = Genome::random(&mut rng);
        assert_eq!(Genome::from_parts(g.traits(), g.weights), g);
    }
}
