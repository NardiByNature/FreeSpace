//! Versioned little-endian byte snapshots of a whole world, including RNG
//! state and free lists, so a restored world continues bit-for-bit.
//! Canonical layout: two worlds in the same state produce the same bytes.

use crate::genome::{Genome, N_HID, N_IN, N_OUT, N_W};
use crate::stats::{HUE_BINS, SAMPLE_STRIDE};
use crate::world::{Config, World};

pub const MAGIC: &[u8; 4] = b"TDPL";
pub const VERSION: u32 = 2;

struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn f32s(&mut self, v: &[f32]) {
        self.buf.reserve(v.len() * 4);
        for x in v {
            self.f32(*x);
        }
    }
    fn u32s(&mut self, v: &[u32]) {
        self.u32(v.len() as u32);
        for x in v {
            self.u32(*x);
        }
    }
}

struct Reader<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.pos + n > self.b.len() {
            return Err(format!("snapshot truncated at byte {}", self.pos));
        }
        let s = &self.b[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, String> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn f32(&mut self) -> Result<f32, String> {
        let s = self.take(4)?;
        Ok(f32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn f32s(&mut self, out: &mut [f32]) -> Result<(), String> {
        for v in out.iter_mut() {
            *v = self.f32()?;
        }
        Ok(())
    }
    fn u32s(&mut self) -> Result<Vec<u32>, String> {
        let n = self.u32()? as usize;
        if n > self.b.len() / 4 {
            return Err("snapshot list length out of range".into());
        }
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(self.u32()?);
        }
        Ok(v)
    }
}

impl World {
    pub fn snapshot(&self) -> Vec<u8> {
        let mut w = Writer { buf: Vec::with_capacity(1 << 20) };
        w.buf.extend_from_slice(MAGIC);
        w.u32(VERSION);
        w.f32s(&self.cfg.to_fields());
        w.u32(self.seed);
        w.u32(self.tick);
        w.u32(self.next_id);
        for s in self.rng.state() {
            w.u32(s);
        }
        w.u32(self.tally.births);
        w.u32(self.tally.deaths);
        w.u32(self.tally.bites);
        w.u32(self.tally.washed);
        w.f32(self.tally.approach_sum);
        w.u32(self.tally.approach_n);

        w.u32(self.nutrients.cols as u32);
        w.u32(self.nutrients.rows as u32);
        w.f32s(&self.nutrients.level);
        w.f32s(&self.nutrients.capacity);

        let p = &self.plants;
        w.u32(p.capacity() as u32);
        for i in 0..p.capacity() {
            w.u8(p.alive[i] as u8);
            if p.alive[i] {
                w.f32(p.x[i]);
                w.f32(p.y[i]);
                w.f32(p.energy[i]);
            }
        }
        w.u32s(p.free_list());

        let c = &self.creatures;
        w.u32(c.capacity() as u32);
        for i in 0..c.capacity() {
            w.u8(c.alive[i] as u8);
            if c.alive[i] {
                w.u32(c.id[i]);
                w.f32(c.x[i]);
                w.f32(c.y[i]);
                w.f32(c.heading[i]);
                w.f32(c.energy[i]);
                w.f32(c.age[i]);
                w.u32(c.generation[i]);
                w.u32(c.meals[i]);
                w.f32s(&c.genome[i].traits());
                w.f32s(&c.genome[i].weights);
                w.f32s(&c.inputs[i]);
                w.f32s(&c.hidden[i]);
                w.f32s(&c.outputs[i]);
            }
        }
        w.u32s(c.free_list());

        let h = &self.history;
        w.u32(h.every);
        w.u32(h.cap as u32);
        w.u32(h.len as u32);
        for i in 0..h.len {
            w.f32s(h.row(i));
        }

        let c = &self.chronicle;
        w.u32(c.total);
        w.u32(c.samples);
        w.u8(c.has_hunters as u8);
        w.u8(c.was_quiet as u8);
        w.u32(c.last_boom);
        w.u32(c.last_crash);
        w.u32(c.gen_milestone);
        w.f32s(&c.bin_peak);
        for d in c.bin_dominant {
            w.u8(d as u8);
        }
        w.u32(c.len as u32);
        for i in 0..c.len {
            let e = c.event(i);
            w.u32(e.tick);
            w.u32(e.kind as u32);
            w.f32(e.a);
            w.f32(e.b);
        }
        w.buf
    }

    pub fn restore(bytes: &[u8]) -> Result<World, String> {
        let mut r = Reader { b: bytes, pos: 0 };
        if r.take(4)? != MAGIC {
            return Err("not a Tidepool snapshot".into());
        }
        let version = r.u32()?;
        if version != VERSION {
            return Err(format!("this snapshot is format {version}; this build reads format {VERSION}"));
        }
        let mut fields = [0.0f32; Config::N_FIELDS];
        r.f32s(&mut fields)?;
        let cfg = Config::from_fields(&fields);
        if cfg.max_creatures == 0 || cfg.max_creatures > 1_000_000 || cfg.max_plants > 4_000_000 || cfg.grid_cols * cfg.grid_rows > 1 << 22 {
            return Err("snapshot config out of range".into());
        }
        let seed = r.u32()?;
        let mut w = World::empty(seed, cfg);
        w.tick = r.u32()?;
        w.next_id = r.u32()?;
        let st = [r.u32()?, r.u32()?, r.u32()?, r.u32()?];
        w.rng = crate::rng::Rng::from_state(st);
        w.tally.births = r.u32()?;
        w.tally.deaths = r.u32()?;
        w.tally.bites = r.u32()?;
        w.tally.washed = r.u32()?;
        w.tally.approach_sum = r.f32()?;
        w.tally.approach_n = r.u32()?;

        let cols = r.u32()? as usize;
        let rows = r.u32()? as usize;
        if cols != w.nutrients.cols || rows != w.nutrients.rows {
            return Err("nutrient grid size mismatch".into());
        }
        r.f32s(&mut w.nutrients.level)?;
        r.f32s(&mut w.nutrients.capacity)?;

        let pcap = r.u32()? as usize;
        if pcap != w.plants.capacity() {
            return Err("plant capacity mismatch".into());
        }
        for i in 0..pcap {
            let alive = r.u8()? != 0;
            w.plants.alive[i] = alive;
            if alive {
                w.plants.x[i] = r.f32()?;
                w.plants.y[i] = r.f32()?;
                w.plants.energy[i] = r.f32()?;
            }
        }
        w.plants.set_free_list(r.u32s()?);

        let ccap = r.u32()? as usize;
        if ccap != w.creatures.capacity() {
            return Err("creature capacity mismatch".into());
        }
        let c = &mut w.creatures;
        for i in 0..ccap {
            let alive = r.u8()? != 0;
            c.alive[i] = alive;
            if alive {
                c.id[i] = r.u32()?;
                c.x[i] = r.f32()?;
                c.y[i] = r.f32()?;
                c.heading[i] = r.f32()?;
                c.energy[i] = r.f32()?;
                c.age[i] = r.f32()?;
                c.generation[i] = r.u32()?;
                c.meals[i] = r.u32()?;
                let mut traits = [0.0f32; Genome::N_TRAITS];
                r.f32s(&mut traits)?;
                let mut weights = [0.0f32; N_W];
                r.f32s(&mut weights)?;
                let g = Genome::from_parts(traits, weights);
                c.size[i] = g.size;
                c.hue[i] = g.hue;
                c.radius[i] = g.radius();
                c.genome[i] = g;
                let mut inp = [0.0f32; N_IN];
                r.f32s(&mut inp)?;
                c.inputs[i] = inp;
                let mut hid = [0.0f32; N_HID];
                r.f32s(&mut hid)?;
                c.hidden[i] = hid;
                let mut out = [0.0f32; N_OUT];
                r.f32s(&mut out)?;
                c.outputs[i] = out;
            }
        }
        c.set_free_list(r.u32s()?);

        let every = r.u32()?;
        let cap = r.u32()? as usize;
        let len = r.u32()? as usize;
        if cap != w.history.cap || every != w.history.every || len > cap {
            return Err("history shape mismatch".into());
        }
        let mut row = [0.0f32; SAMPLE_STRIDE];
        for _ in 0..len {
            r.f32s(&mut row)?;
            w.history.push(&row);
        }

        let c = &mut w.chronicle;
        let total = r.u32()?;
        c.samples = r.u32()?;
        c.has_hunters = r.u8()? != 0;
        c.was_quiet = r.u8()? != 0;
        c.last_boom = r.u32()?;
        c.last_crash = r.u32()?;
        c.gen_milestone = r.u32()?;
        r.f32s(&mut c.bin_peak)?;
        for b in 0..HUE_BINS {
            c.bin_dominant[b] = r.u8()? != 0;
        }
        let n = r.u32()? as usize;
        if n > crate::stats::EVENT_CAP {
            return Err("too many events in snapshot".into());
        }
        for _ in 0..n {
            let tick = r.u32()?;
            let kind = r.u32()? as u16;
            let a = r.f32()?;
            let b = r.f32()?;
            c.push(tick, kind, a, b);
        }
        c.total = total;
        Ok(w)
    }
}
