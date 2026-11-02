//! Deterministic PRNG: xoshiro128** seeded through splitmix32.
//! Every random decision in the simulation flows through one `Rng` owned
//! by the world, so a seed fully determines a run.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    s: [u32; 4],
}

#[inline]
fn splitmix32(state: &mut u32) -> u32 {
    *state = state.wrapping_add(0x9E37_79B9);
    let mut z = *state;
    z = (z ^ (z >> 16)).wrapping_mul(0x85EB_CA6B);
    z = (z ^ (z >> 13)).wrapping_mul(0xC2B2_AE35);
    z ^ (z >> 16)
}

impl Rng {
    pub fn new(seed: u32) -> Rng {
        let mut st = seed;
        let mut s = [0u32; 4];
        for v in s.iter_mut() {
            *v = splitmix32(&mut st);
        }
        if s == [0; 4] {
            s[0] = 1;
        }
        Rng { s }
    }

    pub fn state(&self) -> [u32; 4] {
        self.s
    }

    pub fn from_state(s: [u32; 4]) -> Rng {
        Rng { s }
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 9;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(11);
        result
    }

    /// Uniform in [0, 1) with 24 bits of precision.
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }

    /// Uniform in [lo, hi).
    #[inline]
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }

    /// Uniform integer in [0, n). `n` must be > 0.
    #[inline]
    pub fn below(&mut self, n: u32) -> u32 {
        // Lemire's multiply-shift; slight bias is irrelevant here and it is branch-free.
        ((self.next_u32() as u64 * n as u64) >> 32) as u32
    }

    #[inline]
    pub fn chance(&mut self, p: f32) -> bool {
        self.next_f32() < p
    }

    /// Approximately standard normal via Irwin-Hall (sum of 12 uniforms).
    /// Arithmetic only, so it is bit-identical on every platform. Tails stop at +-6.
    #[inline]
    pub fn gaussian(&mut self) -> f32 {
        let mut acc = 0.0f32;
        for _ in 0..12 {
            acc += self.next_f32();
        }
        acc - 6.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..10_000 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn different_seeds_differ() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        let same = (0..64).filter(|_| a.next_u32() == b.next_u32()).count();
        assert!(same < 2);
    }

    #[test]
    fn state_round_trip() {
        let mut a = Rng::new(7);
        for _ in 0..100 {
            a.next_u32();
        }
        let mut b = Rng::from_state(a.state());
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn uniform_is_in_range_and_roughly_flat() {
        let mut r = Rng::new(3);
        let mut bins = [0u32; 10];
        for _ in 0..100_000 {
            let v = r.next_f32();
            assert!((0.0..1.0).contains(&v));
            bins[(v * 10.0) as usize] += 1;
        }
        for b in bins {
            assert!((9_000..11_000).contains(&b), "bin {b}");
        }
    }

    #[test]
    fn gaussian_moments() {
        let mut r = Rng::new(9);
        let n = 200_000;
        let (mut sum, mut sq) = (0.0f64, 0.0f64);
        for _ in 0..n {
            let g = r.gaussian() as f64;
            sum += g;
            sq += g * g;
        }
        let mean = sum / n as f64;
        let var = sq / n as f64 - mean * mean;
        assert!(mean.abs() < 0.01, "mean {mean}");
        assert!((var - 1.0).abs() < 0.02, "var {var}");
    }

    #[test]
    fn below_is_in_range() {
        let mut r = Rng::new(11);
        for _ in 0..10_000 {
            assert!(r.below(7) < 7);
        }
    }
}
