//! Headless run for tuning: `cargo run --release --example run -- <seed> <ticks>`
use std::time::Instant;
use tidepool_core::stats::*;
use tidepool_core::world::{Config, World};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let ticks: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let every: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1_000);
    let mut cfg = Config::default();
    if let Some(n) = args.get(4).and_then(|s| s.parse().ok()) {
        cfg.initial_creatures = n;
    }
    let mut w = World::new(seed, cfg);
    println!("seed {seed}  world {}x{}  founders {}", w.cfg.width, w.cfg.height, w.cfg.initial_creatures);
    println!("{:>6} {:>6} {:>6} {:>5} {:>5} {:>5} {:>7} {:>6} {:>6} {:>5} {:>4} {:>8} {:>7}",
        "tick", "pop", "plants", "herb", "omni", "carn", "apprch", "sense", "speed", "diet", "gen", "nutr", "tps");
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
        println!("{:>6} {:>6} {:>6} {:>5} {:>5} {:>5} {:>7.3} {:>6.1} {:>6.2} {:>5.2} {:>4} {:>8.0} {:>7.0}",
            r[F_TICK], r[F_POP], r[F_PLANTS], r[F_HERB], r[F_OMNI], r[F_CARN], r[F_APPROACH],
            r[F_MEAN_SENSE], r[F_MEAN_SPEED], r[F_MEAN_DIET], r[F_GENERATION], r[F_NUTRIENTS], n as f64 / dt);
    }
    let total = start.elapsed().as_secs_f64();
    println!("{ticks} ticks in {total:.2}s = {:.0} ticks/s", ticks as f64 / total);
}
