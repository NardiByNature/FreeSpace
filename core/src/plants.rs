//! The bottom of the food chain: a nutrient field that regrows and diffuses,
//! and plants that sprout from it. Dead creatures return nutrients here, so
//! energy cycles instead of draining away.

pub struct Nutrients {
    pub cols: usize,
    pub rows: usize,
    pub cell_w: f32,
    pub cell_h: f32,
    pub level: Vec<f32>,
    pub capacity: Vec<f32>,
    scratch: Vec<f32>,
}

impl Nutrients {
    pub fn new(width: f32, height: f32, cols: usize, rows: usize, capacity: f32) -> Nutrients {
        let n = cols * rows;
        Nutrients {
            cols,
            rows,
            cell_w: width / cols as f32,
            cell_h: height / rows as f32,
            level: vec![capacity * 0.5; n],
            capacity: vec![capacity; n],
            scratch: vec![0.0; n],
        }
    }

    #[inline]
    pub fn cell_at(&self, x: f32, y: f32) -> usize {
        let cx = ((x / self.cell_w) as usize).min(self.cols - 1);
        let cy = ((y / self.cell_h) as usize).min(self.rows - 1);
        cy * self.cols + cx
    }

    /// Regrowth toward capacity scaled by the season (1 = full capacity).
    pub fn regrow(&mut self, rate: f32, season: f32) {
        for (l, c) in self.level.iter_mut().zip(self.capacity.iter()) {
            *l += rate * (*c * season - *l);
        }
    }

    /// Four-neighbour blur on the torus; `k` is the fraction moved per call.
    pub fn diffuse(&mut self, k: f32) {
        let (cols, rows) = (self.cols, self.rows);
        for y in 0..rows {
            let up = (y + rows - 1) % rows;
            let down = (y + 1) % rows;
            for x in 0..cols {
                let left = (x + cols - 1) % cols;
                let right = (x + 1) % cols;
                let avg = (self.level[up * cols + x]
                    + self.level[down * cols + x]
                    + self.level[y * cols + left]
                    + self.level[y * cols + right])
                    * 0.25;
                let i = y * cols + x;
                self.scratch[i] = self.level[i] + k * (avg - self.level[i]);
            }
        }
        core::mem::swap(&mut self.level, &mut self.scratch);
    }

    pub fn deposit(&mut self, x: f32, y: f32, amount: f32) {
        let i = self.cell_at(x, y);
        self.level[i] += amount;
    }

    pub fn total(&self) -> f32 {
        self.level.iter().sum()
    }
}

/// Struct-of-arrays plant storage with a free list, so slots are reused and
/// indices stay stable while a plant is alive.
pub struct Plants {
    pub alive: Vec<bool>,
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    pub energy: Vec<f32>,
    free: Vec<u32>,
    count: usize,
}

impl Plants {
    pub fn new(capacity: usize) -> Plants {
        Plants {
            alive: vec![false; capacity],
            x: vec![0.0; capacity],
            y: vec![0.0; capacity],
            energy: vec![0.0; capacity],
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

    pub fn spawn(&mut self, x: f32, y: f32, energy: f32) -> Option<u32> {
        let i = self.free.pop()?;
        let u = i as usize;
        self.alive[u] = true;
        self.x[u] = x;
        self.y[u] = y;
        self.energy[u] = energy;
        self.count += 1;
        Some(i)
    }

    pub fn kill(&mut self, i: u32) {
        let u = i as usize;
        if self.alive[u] {
            self.alive[u] = false;
            self.free.push(i);
            self.count -= 1;
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

    pub fn free_list(&self) -> &[u32] {
        &self.free
    }

    /// Install an exact free list (snapshot restore); `count` follows from `alive`.
    pub fn set_free_list(&mut self, free: Vec<u32>) {
        self.free = free;
        self.count = self.alive.iter().filter(|a| **a).count();
    }

    /// Rebuild the free list from `alive` when no exact list is available.
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regrow_approaches_capacity() {
        let mut n = Nutrients::new(100.0, 100.0, 4, 4, 100.0);
        n.level.iter_mut().for_each(|l| *l = 0.0);
        for _ in 0..5_000 {
            n.regrow(0.001, 1.0);
        }
        assert!(n.level.iter().all(|l| *l > 99.0));
    }

    #[test]
    fn diffuse_conserves_total() {
        let mut n = Nutrients::new(100.0, 100.0, 8, 8, 100.0);
        n.level.iter_mut().for_each(|l| *l = 0.0);
        n.level[0] = 640.0;
        let before = n.total();
        for _ in 0..50 {
            n.diffuse(0.2);
        }
        assert!((n.total() - before).abs() < 1e-2);
        assert!(n.level[0] < 100.0);
    }

    #[test]
    fn plant_slots_recycle() {
        let mut p = Plants::new(3);
        let a = p.spawn(1.0, 1.0, 5.0).unwrap();
        let _b = p.spawn(2.0, 2.0, 5.0).unwrap();
        let _c = p.spawn(3.0, 3.0, 5.0).unwrap();
        assert!(p.spawn(4.0, 4.0, 5.0).is_none());
        p.kill(a);
        assert_eq!(p.count(), 2);
        assert_eq!(p.spawn(9.0, 9.0, 1.0), Some(a));
        p.rebuild_free_list();
        assert_eq!(p.count(), 3);
    }
}
