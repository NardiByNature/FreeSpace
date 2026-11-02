//! Headless run for tuning: `cargo run --release --example run -- <seed> <ticks>`
use std::time::Instant;
use tidepool_core::stats::*;
use tidepool_core::world::{Config, World};

/// Typical genome distances: parent to child, two members of the same colour
/// band, and two random live creatures. Guides the choice of `hybrid_cost`.
fn distance_report(w: &World) {
    use tidepool_core::genome::Genome;
    use tidepool_core::rng::Rng;
    let c = &w.creatures;
    let alive: Vec<usize> = (0..c.capacity()).filter(|&i| c.alive[i]).collect();
    if alive.len() < 10 {
        return;
    }
    let mut rng = Rng::new(0xD15);
    let pick = |rng: &mut Rng| alive[rng.below(alive.len() as u32) as usize];
    let stats = |label: &str, vals: &mut Vec<f32>| {
        if vals.is_empty() {
            println!("  {label:<14} (no pairs)");
            return;
        }
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mean: f32 = vals.iter().sum::<f32>() / vals.len() as f32;
        println!("  {label:<14} mean {:.3}  p10 {:.3}  p90 {:.3}  (n={})", mean, vals[vals.len() / 10], vals[vals.len() * 9 / 10], vals.len());
    };
    println!("genome distances:");
    let mut kids = Vec::new();
    for _ in 0..300 {
        let i = pick(&mut rng);
        let child = c.genome[i].mutate(&mut rng);
        kids.push(c.genome[i].distance(&child));
    }
    stats("parent-child", &mut kids);
    let mut same = Vec::new();
    let mut other = Vec::new();
    for _ in 0..3000 {
        let (i, j) = (pick(&mut rng), pick(&mut rng));
        if i == j {
            continue;
        }
        let d = c.genome[i].distance(&c.genome[j]);
        if tidepool_core::fastmath::hue_distance(c.genome[i].hue, c.genome[j].hue) < 0.03 {
            same.push(d);
        } else {
            other.push(d);
        }
    }
    stats("same colour", &mut same);
    stats("other colour", &mut other);
    let mut founders = Vec::new();
    for _ in 0..300 {
        let a = Genome::random(&mut rng);
        let b = Genome::random(&mut rng);
        founders.push(a.distance(&b));
    }
    stats("two founders", &mut founders);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let ticks: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let every: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1_000);
    let mut cfg = Config::default();
    for kv in args.iter().skip(4) {
        let (k, v) = kv.split_once('=').expect("override as key=value");
        let f: f32 = v.parse().expect("numeric value");
        match k {
            "founders" => cfg.initial_creatures = f as u32,
            "plants" => cfg.initial_plants = f as u32,
            "bite" => cfg.bite_damage = f,
            "bite_cost" => cfg.bite_cost = f,
            "regrow" => cfg.regrow_rate = f,
            "growth" => cfg.plant_growth = f,
            "graze" => cfg.graze_bite = f,
            "plant_max" => cfg.plant_max_energy = f,
            "spawn" => cfg.plant_spawn_rate = f,
            "basal" => cfg.basal_cost = f,
            "move" => cfg.move_cost = f,
            "min_pop" => cfg.min_population = f as u32,
            "corpse" => cfg.corpse_return = f,
            "terrain" => cfg.terrain_contrast = f,
            "reach" => cfg.bite_reach = f,
            "sexual" => cfg.sexual = f > 0.5,
            "solo" => cfg.solo_after = f,
            "season" => cfg.season_depth = f,
            "period" => cfg.season_period = f,
            "hybrid" => cfg.hybrid_cost = f,
            other => panic!("unknown override {other}"),
        }
    }
    let mut w = World::new(seed, cfg);
    println!("seed {seed}  world {}x{}  founders {}", w.cfg.width, w.cfg.height, w.cfg.initial_creatures);
    println!("{:>6} {:>6} {:>6} {:>5} {:>5} {:>5} {:>7} {:>6} {:>6} {:>5} {:>4} {:>8} {:>5} {:>5} {:>5} {:>5} {:>7}",
        "tick", "pop", "plants", "herb", "omni", "carn", "apprch", "sense", "speed", "diet", "gen", "nutr", "bites", "pref", "hybr", "bands", "tps");
    let start = Instant::now();
    let mut last = Instant::now();
    let mut done = 0u32;
    while done < ticks {
        let n = every.min(ticks - done);
        w.step(n);
        done += n;
        let dt = last.elapsed().as_secs_f64();
        last = Instant::now();
        let r = w.history.latest().unwrap();
        // Colour bands: contiguous runs of hue bins each holding at least 2% of the population.
        let pop = r[F_POP].max(1.0);
        let occupied: Vec<bool> = (0..HUE_BINS).map(|b| r[N_FIELDS + b] / pop >= 0.02).collect();
        let mut bands = (0..HUE_BINS).filter(|&b| occupied[b] && !occupied[(b + HUE_BINS - 1) % HUE_BINS]).count();
        if bands == 0 && occupied.iter().any(|o| *o) {
            bands = 1;
        }
        println!("{:>6} {:>6} {:>6} {:>5} {:>5} {:>5} {:>7.3} {:>6.1} {:>6.2} {:>5.2} {:>4} {:>8.0} {:>5} {:>5.2} {:>5} {:>5} {:>7.0}",
            r[F_TICK], r[F_POP], r[F_PLANTS], r[F_HERB], r[F_OMNI], r[F_CARN], r[F_APPROACH],
            r[F_MEAN_SENSE], r[F_MEAN_SPEED], r[F_MEAN_DIET], r[F_GENERATION], r[F_NUTRIENTS], r[F_BITES], r[F_MEAN_PREF], r[F_HYBRIDS], bands, n as f64 / dt);
    }
    let total = start.elapsed().as_secs_f64();
    println!("{ticks} ticks in {total:.2}s = {:.0} ticks/s", ticks as f64 / total);
    distance_report(&w);
    let c = &w.chronicle;
    let names = ["first hunters", "hunters gone", "boom", "crash", "lineage lost", "lineage dominant", "generation", "washed in", "quiet", "speciation"];
    println!("chronicle ({} events, showing last {}):", c.total, c.len.min(25));
    for i in c.len.saturating_sub(25)..c.len {
        let e = c.event(i);
        println!("  t{:>7}  {:<17} a={:.3} b={:.3}", e.tick, names.get(e.kind as usize).unwrap_or(&"?"), e.a, e.b);
    }
}
