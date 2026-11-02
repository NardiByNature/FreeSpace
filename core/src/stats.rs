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
pub const F_MEAN_PREF: usize = 16;
pub const F_HYBRIDS: usize = 17;
pub const F_OLDEST: usize = 18;
pub const F_MOST_MEALS: usize = 19;
pub const N_FIELDS: usize = 20;
pub const SAMPLE_STRIDE: usize = N_FIELDS + HUE_BINS;

/// Accumulated during a tick, folded into a sample every `every` ticks.
#[derive(Clone, Copy, Debug, Default)]
pub struct Tally {
    pub births: u32,
    pub deaths: u32,
    pub bites: u32,
    pub washed: u32,
    /// Sexual births whose parents differed in hue by more than 0.1.
    pub hybrids: u32,
    pub approach_sum: f32,
    pub approach_n: u32,
}

// ---- Chronicle: milestones the world notices about itself ----

pub const EV_FIRST_HUNTERS: u16 = 0;
pub const EV_HUNTERS_GONE: u16 = 1;
pub const EV_BOOM: u16 = 2;
pub const EV_CRASH: u16 = 3;
pub const EV_LINEAGE_LOST: u16 = 4;
pub const EV_LINEAGE_DOMINANT: u16 = 5;
pub const EV_GENERATION: u16 = 6;
pub const EV_WASHED_IN: u16 = 7;
pub const EV_QUIET: u16 = 8;
pub const EV_SPECIATION: u16 = 9;

pub const EVENT_CAP: usize = 256;
/// Samples between the two population readings compared for boom/crash.
const SWING_SPAN: usize = 100;
const SWING_COOLDOWN: u32 = 200;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    pub tick: u32,
    pub kind: u16,
    pub a: f32,
    pub b: f32,
}

/// Watches each sample row and records events. Holds the little state the
/// detectors need (flags, cooldowns, decaying per-bin peaks) so the same
/// story can repeat: hunters can appear, vanish, and appear again.
pub struct Chronicle {
    pub events: Vec<Event>,
    pub head: usize,
    pub len: usize,
    /// Monotonic count of events ever recorded, so a reader can tell when there are new ones.
    pub total: u32,
    pub samples: u32,
    pub has_hunters: bool,
    pub was_quiet: bool,
    pub last_boom: u32,
    pub last_crash: u32,
    pub gen_milestone: u32,
    pub bin_peak: [f32; HUE_BINS],
    pub bin_dominant: [bool; HUE_BINS],
    /// Consecutive samples with two major colour bands separated by empty bins.
    pub split_streak: u32,
    pub split_announced: bool,
    pub split_quiet: u32,
}

impl Default for Chronicle {
    fn default() -> Chronicle {
        Chronicle {
            events: vec![Event { tick: 0, kind: 0, a: 0.0, b: 0.0 }; EVENT_CAP],
            head: 0,
            len: 0,
            total: 0,
            samples: 0,
            has_hunters: false,
            was_quiet: false,
            last_boom: 0,
            last_crash: 0,
            gen_milestone: 0,
            bin_peak: [0.0; HUE_BINS],
            bin_dominant: [false; HUE_BINS],
            split_streak: 0,
            split_announced: false,
            split_quiet: 0,
        }
    }
}

/// Two hue bins both holding at least 15% of the population with every bin
/// on the shorter arc between them under 1%: the signature of lineages that
/// have stopped mixing. Returns the pair's bin indices.
pub fn find_split(row: &[f32], pop: f32) -> Option<(usize, usize)> {
    if pop <= 0.0 {
        return None;
    }
    let share = |b: usize| row[N_FIELDS + (b % HUE_BINS)] / pop;
    let majors: Vec<usize> = (0..HUE_BINS).filter(|&b| share(b) >= 0.15).collect();
    for (i, &a) in majors.iter().enumerate() {
        for &b in &majors[i + 1..] {
            let forward = b - a; // walking up from a to b
            let backward = HUE_BINS - forward;
            let (start, len) = if forward <= backward { (a, forward) } else { (b, backward) };
            if len < 2 {
                continue;
            }
            if (1..len).all(|k| share(start + k) < 0.01) {
                return Some((a, b));
            }
        }
    }
    None
}

impl Chronicle {
    pub fn push(&mut self, tick: u32, kind: u16, a: f32, b: f32) {
        self.events[self.head] = Event { tick, kind, a, b };
        self.head = (self.head + 1) % EVENT_CAP;
        if self.len < EVENT_CAP {
            self.len += 1;
        }
        self.total = self.total.wrapping_add(1);
    }

    /// Event `i` counting from the oldest retained.
    pub fn event(&self, i: usize) -> Event {
        self.events[(self.head + EVENT_CAP - self.len + i) % EVENT_CAP]
    }

    /// Inspect the newest sample (already pushed to `history`) and the tally
    /// for the interval it covers.
    pub fn observe(&mut self, history: &History, washed: u32) {
        let Some(row) = history.latest() else { return };
        let tick = row[F_TICK] as u32;
        let pop = row[F_POP];
        let carn = row[F_CARN] as u32;
        self.samples += 1;

        if washed > 0 {
            self.push(tick, EV_WASHED_IN, washed as f32, 0.0);
        }

        if carn >= 5 && !self.has_hunters {
            self.has_hunters = true;
            self.push(tick, EV_FIRST_HUNTERS, carn as f32, 0.0);
        } else if carn == 0 && self.has_hunters {
            self.has_hunters = false;
            self.push(tick, EV_HUNTERS_GONE, 0.0, 0.0);
        }

        let quiet = pop <= 0.0;
        if quiet && !self.was_quiet {
            self.push(tick, EV_QUIET, 0.0, 0.0);
        }
        self.was_quiet = quiet;

        if history.len > SWING_SPAN {
            let before = history.row(history.len - 1 - SWING_SPAN)[F_POP];
            if before >= 50.0 && pop >= before * 1.6 && self.samples - self.last_boom >= SWING_COOLDOWN {
                self.last_boom = self.samples;
                self.push(tick, EV_BOOM, pop, before);
            }
            if before >= 100.0 && pop <= before * 0.6 && self.samples - self.last_crash >= SWING_COOLDOWN {
                self.last_crash = self.samples;
                self.push(tick, EV_CRASH, pop, before);
            }
        }

        let gen = row[F_GENERATION] as u32;
        if gen / 100 > self.gen_milestone {
            self.gen_milestone = gen / 100;
            self.push(tick, EV_GENERATION, (self.gen_milestone * 100) as f32, 0.0);
        }

        match find_split(row, pop) {
            Some((a, b)) => {
                self.split_streak += 1;
                self.split_quiet = 0;
                if self.split_streak >= 100 && !self.split_announced {
                    self.split_announced = true;
                    let hue = |bin: usize| (bin as f32 + 0.5) / HUE_BINS as f32;
                    self.push(tick, EV_SPECIATION, hue(a), hue(b));
                }
            }
            None => {
                self.split_streak = 0;
                self.split_quiet += 1;
                if self.split_quiet >= 300 {
                    self.split_announced = false;
                }
            }
        }

        if pop > 0.0 {
            for b in 0..HUE_BINS {
                let share = row[N_FIELDS + b] / pop;
                let hue = (b as f32 + 0.5) / HUE_BINS as f32;
                if share <= 0.0 && self.bin_peak[b] >= 0.04 {
                    self.push(tick, EV_LINEAGE_LOST, hue, self.bin_peak[b]);
                    self.bin_peak[b] = 0.0;
                } else {
                    // Decaying peak: remembers roughly the last few hundred samples.
                    self.bin_peak[b] = share.max(self.bin_peak[b] * 0.995);
                }
                if share >= 0.30 && !self.bin_dominant[b] {
                    self.bin_dominant[b] = true;
                    self.push(tick, EV_LINEAGE_DOMINANT, hue, share);
                } else if share < 0.15 {
                    self.bin_dominant[b] = false;
                }
            }
        }
    }
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
    fn chronicle_ring_and_total() {
        let mut c = Chronicle::default();
        for t in 0..(EVENT_CAP as u32 + 10) {
            c.push(t, EV_BOOM, 0.0, 0.0);
        }
        assert_eq!(c.len, EVENT_CAP);
        assert_eq!(c.total, EVENT_CAP as u32 + 10);
        assert_eq!(c.event(0).tick, 10);
        assert_eq!(c.event(c.len - 1).tick, EVENT_CAP as u32 + 9);
    }

    #[test]
    fn speciation_needs_two_separated_bands_for_a_while() {
        let mut h = History::new(512, 10);
        let mut c = Chronicle::default();
        let mut row = [0.0f32; SAMPLE_STRIDE];
        row[F_POP] = 100.0;
        row[N_FIELDS + 4] = 50.0;
        row[N_FIELDS + 5] = 50.0; // adjacent bins: one band, not a split
        for t in 0..120 {
            row[F_TICK] = t as f32 * 10.0;
            h.push(&row);
            c.observe(&h, 0);
        }
        assert!(!(0..c.len).any(|i| c.event(i).kind == EV_SPECIATION));
        row[N_FIELDS + 5] = 0.0;
        row[N_FIELDS + 20] = 50.0; // far apart with empty bins between
        for t in 120..240 {
            row[F_TICK] = t as f32 * 10.0;
            h.push(&row);
            c.observe(&h, 0);
        }
        let ev: Vec<Event> = (0..c.len).map(|i| c.event(i)).filter(|e| e.kind == EV_SPECIATION).collect();
        assert_eq!(ev.len(), 1);
        assert!((ev[0].a - 4.5 / HUE_BINS as f32).abs() < 1e-6);
    }

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
