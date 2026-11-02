//! Per-tick tallies and a fixed-size history of samples for the charts.
//! A sample is a flat f32 row: a few scalar fields followed by a hue
//! histogram, so the JS side can read the whole history as one view.

pub const HUE_BINS: usize = 32;

pub const F_TICK: usize = 0;
pub const F_POP: usize = 1;
pub const F_PLANTS: usize = 2;
pub const F_HERB: usize = 3;
pub const F_OMNI: usize = 4;
pub const F_CARN: usize = 5;
pub const F_BIRTHS: usize = 6;
pub const F_DEATHS: usize = 7;
pub const F_APPROACH: usize = 8;
pub const F_MEAN_SIZE: usize = 9;
pub const F_MEAN_SPEED: usize = 10;
pub const F_MEAN_SENSE: usize = 11;
pub const F_MEAN_DIET: usize = 12;
pub const F_NUTRIENTS: usize = 13;
pub const F_GENERATION: usize = 14;
pub const F_BITES: usize = 15;
pub const N_FIELDS: usize = 16;
pub const SAMPLE_STRIDE: usize = N_FIELDS + HUE_BINS;

/// Accumulated during a tick, folded into a sample every `every` ticks.
#[derive(Clone, Copy, Debug, Default)]
pub struct Tally {
    pub births: u32,
    pub deaths: u32,
    pub bites: u32,
    pub approach_sum: f32,
    pub approach_n: u32,
}

pub struct History {
    pub every: u32,
    pub cap: usize,
    pub data: Vec<f32>,
    /// Index of the next row to write.
    pub head: usize,
    pub len: usize,
}

impl History {
    pub fn new(cap: usize, every: u32) -> History {
        History {
            every,
            cap,
            data: vec![0.0; cap * SAMPLE_STRIDE],
            head: 0,
            len: 0,
        }
    }

    pub fn push(&mut self, row: &[f32; SAMPLE_STRIDE]) {
        let base = self.head * SAMPLE_STRIDE;
        self.data[base..base + SAMPLE_STRIDE].copy_from_slice(row);
        self.head = (self.head + 1) % self.cap;
        if self.len < self.cap {
            self.len += 1;
        }
    }

    /// Row `i` counting from the oldest retained sample.
    pub fn row(&self, i: usize) -> &[f32] {
        let start = (self.head + self.cap - self.len + i) % self.cap;
        &self.data[start * SAMPLE_STRIDE..(start + 1) * SAMPLE_STRIDE]
    }

    pub fn latest(&self) -> Option<&[f32]> {
        if self.len == 0 {
            None
        } else {
            Some(self.row(self.len - 1))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_keeps_newest() {
        let mut h = History::new(4, 1);
        for t in 0..6 {
            let mut row = [0.0f32; SAMPLE_STRIDE];
            row[F_TICK] = t as f32;
            h.push(&row);
        }
        assert_eq!(h.len, 4);
        assert_eq!(h.row(0)[F_TICK], 2.0);
        assert_eq!(h.latest().unwrap()[F_TICK], 5.0);
    }
}
