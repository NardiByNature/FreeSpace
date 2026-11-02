//! `cargo run --release --example profile -- <seed> <warmup_ticks>`: per-phase timings.
use tidepool_core::world::{Config, World};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let warm: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3000);
    let mut w = World::new(seed, Config::default());
    w.step(warm);
    let mut acc = [0.0f64; 6];
    let n = 100;
    for _ in 0..n {
        let t = w.profile_tick();
        for k in 0..6 {
            acc[k] += t[k];
        }
    }
    println!("after {warm} ticks: pop {} plants {} sense {:.1}", w.population(), w.plants.count(),
        w.history.latest().map(|r| r[tidepool_core::stats::F_MEAN_SENSE]).unwrap_or(0.0));
    let names = ["grow", "hash", "act", "reap", "breed", "sample"];
    let total: f64 = acc.iter().sum();
    for k in 0..6 {
        println!("{:>7}: {:>8.3} ms/tick  {:>5.1}%", names[k], acc[k] / n as f64 * 1e3, acc[k] / total * 100.0);
    }
    println!("  total: {:>8.3} ms/tick", total / n as f64 * 1e3);
}
