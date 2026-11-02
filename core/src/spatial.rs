//! Uniform-grid spatial hash on a torus, rebuilt each tick by counting sort.
//! Nearest-neighbour queries walk cells ring by ring and stop as soon as the
//! best hit is closer than the next ring can be. No allocation per query.

pub struct SpatialHash {
    pub cell: f32,
    pub cols: usize,
    pub rows: usize,
    width: f32,
    height: f32,
    /// cell_start[c]..cell_start[c+1] indexes `items` for cell c.
    cell_start: Vec<u32>,
    items: Vec<u32>,
    counts: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub index: u32,
    pub dist2: f32,
    /// Shortest wrapped offset from the query point to the hit.
    pub dx: f32,
    pub dy: f32,
}

impl SpatialHash {
    pub fn new(width: f32, height: f32, cell: f32) -> SpatialHash {
        let cols = (width / cell).ceil() as usize;
        let rows = (height / cell).ceil() as usize;
        SpatialHash {
            cell,
            cols,
            rows,
            width,
            height,
            cell_start: vec![0; cols * rows + 1],
            items: Vec::new(),
            counts: vec![0; cols * rows],
        }
    }

    #[inline]
    fn cell_of(&self, x: f32, y: f32) -> usize {
        let cx = ((x / self.cell) as usize).min(self.cols - 1);
        let cy = ((y / self.cell) as usize).min(self.rows - 1);
        cy * self.cols + cx
    }

    /// Rebuild from positions; `alive` filters which indices are inserted.
    pub fn rebuild(&mut self, xs: &[f32], ys: &[f32], alive: &[bool]) {
        let n_cells = self.cols * self.rows;
        self.counts.iter_mut().for_each(|c| *c = 0);
        let mut live = 0usize;
        for i in 0..xs.len() {
            if alive[i] {
                let c = self.cell_of(xs[i], ys[i]);
                self.counts[c] += 1;
                live += 1;
            }
        }
        self.cell_start[0] = 0;
        for c in 0..n_cells {
            self.cell_start[c + 1] = self.cell_start[c] + self.counts[c];
        }
        self.items.clear();
        self.items.resize(live, 0);
        // Reuse counts as write cursors.
        self.counts.copy_from_slice(&self.cell_start[..n_cells]);
        for i in 0..xs.len() {
            if alive[i] {
                let c = self.cell_of(xs[i], ys[i]);
                self.items[self.counts[c] as usize] = i as u32;
                self.counts[c] += 1;
            }
        }
    }

    #[inline]
    fn wrap_delta(&self, d: f32, span: f32) -> f32 {
        if d > span * 0.5 {
            d - span
        } else if d < -span * 0.5 {
            d + span
        } else {
            d
        }
    }

    /// Nearest item within `radius` of (x, y), excluding index `skip`
    /// and anything `reject` says no to. Returns None if nothing qualifies.
    pub fn nearest<F: Fn(u32) -> bool>(
        &self,
        xs: &[f32],
        ys: &[f32],
        x: f32,
        y: f32,
        radius: f32,
        skip: u32,
        reject: F,
    ) -> Option<Hit> {
        let r2 = radius * radius;
        let max_ring = (radius / self.cell).ceil() as i32;
        let cx = ((x / self.cell) as i32).min(self.cols as i32 - 1);
        let cy = ((y / self.cell) as i32).min(self.rows as i32 - 1);
        let mut best: Option<Hit> = None;
        for ring in 0..=max_ring {
            // Anything in this ring is at least (ring-1)*cell away.
            if let Some(b) = best {
                let lower = (ring - 1).max(0) as f32 * self.cell;
                if b.dist2 <= lower * lower {
                    break;
                }
            }
            for oy in -ring..=ring {
                let edge_row = oy == -ring || oy == ring;
                let mut ox = -ring;
                while ox <= ring {
                    let gx = (cx + ox).rem_euclid(self.cols as i32) as usize;
                    let gy = (cy + oy).rem_euclid(self.rows as i32) as usize;
                    let c = gy * self.cols + gx;
                    let (s, e) = (self.cell_start[c] as usize, self.cell_start[c + 1] as usize);
                    for &idx in &self.items[s..e] {
                        if idx == skip {
                            continue;
                        }
                        let dx = self.wrap_delta(xs[idx as usize] - x, self.width);
                        let dy = self.wrap_delta(ys[idx as usize] - y, self.height);
                        let d2 = dx * dx + dy * dy;
                        if d2 <= r2 && best.is_none_or(|b| d2 < b.dist2) && !reject(idx) {
                            best = Some(Hit { index: idx, dist2: d2, dx, dy });
                        }
                    }
                    // Interior rows only visit the two edge columns.
                    ox += if edge_row || ox == ring { 1 } else { 2 * ring };
                }
            }
            // A small world can wrap onto itself; never walk more cells than exist.
            if (2 * ring + 1) as usize >= self.cols.max(self.rows) {
                break;
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    fn brute(xs: &[f32], ys: &[f32], alive: &[bool], w: f32, h: f32, x: f32, y: f32, r: f32, skip: u32) -> Option<(u32, f32)> {
        let mut best: Option<(u32, f32)> = None;
        for i in 0..xs.len() {
            if !alive[i] || i as u32 == skip {
                continue;
            }
            let mut dx = (xs[i] - x).abs();
            let mut dy = (ys[i] - y).abs();
            if dx > w * 0.5 {
                dx = w - dx;
            }
            if dy > h * 0.5 {
                dy = h - dy;
            }
            let d2 = dx * dx + dy * dy;
            if d2 <= r * r && best.is_none_or(|b| d2 < b.1) {
                best = Some((i as u32, d2));
            }
        }
        best
    }

    #[test]
    fn matches_brute_force_on_torus() {
        let (w, h) = (512.0f32, 384.0f32);
        let mut rng = Rng::new(21);
        let n = 3000;
        let xs: Vec<f32> = (0..n).map(|_| rng.range(0.0, w)).collect();
        let ys: Vec<f32> = (0..n).map(|_| rng.range(0.0, h)).collect();
        let alive: Vec<bool> = (0..n).map(|i| i % 7 != 0).collect();
        for cell in [16.0f32, 40.0, 100.0] {
            let mut sh = SpatialHash::new(w, h, cell);
            sh.rebuild(&xs, &ys, &alive);
            for q in 0..500 {
                let (x, y) = (rng.range(0.0, w), rng.range(0.0, h));
                let r = rng.range(5.0, 150.0);
                let skip = (q % n) as u32;
                let got = sh.nearest(&xs, &ys, x, y, r, skip, |_| false).map(|hit| (hit.index, hit.dist2));
                let want = brute(&xs, &ys, &alive, w, h, x, y, r, skip);
                match (got, want) {
                    (None, None) => {}
                    (Some(g), Some(t)) => assert!((g.1 - t.1).abs() < 1e-3, "cell {cell}: got {g:?} want {t:?}"),
                    _ => panic!("cell {cell}: got {got:?} want {want:?}"),
                }
            }
        }
    }

    #[test]
    fn reject_filter_is_honoured() {
        let xs = [10.0f32, 12.0, 30.0];
        let ys = [10.0f32, 10.0, 10.0];
        let alive = [true; 3];
        let mut sh = SpatialHash::new(100.0, 100.0, 10.0);
        sh.rebuild(&xs, &ys, &alive);
        let hit = sh.nearest(&xs, &ys, 10.0, 10.0, 50.0, 0, |i| i == 1).unwrap();
        assert_eq!(hit.index, 2);
    }
}
