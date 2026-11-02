use tidepool_core::genome::Genome;
use tidepool_core::rng::Rng;
use tidepool_core::stats::*;
use tidepool_core::world::{Config, World};

fn cfg() -> Config {
    Config {
        width: 768.0,
        height: 768.0,
        grid_cols: 24,
        grid_rows: 24,
        max_plants: 6_000,
        max_creatures: 3_000,
        initial_creatures: 200,
        initial_plants: 1_500,
        ..Config::default()
    }
}

fn kinds(w: &World) -> Vec<u16> {
    (0..w.chronicle.len).map(|i| w.chronicle.event(i).kind).collect()
}

fn carnivore(rng: &mut Rng, hue: f32) -> Genome {
    let mut g = Genome::random(rng);
    g.diet = 1.0;
    g.hue = hue;
    g
}

#[test]
fn hunters_appear_and_vanish() {
    let mut w = World::new(11, cfg());
    let mut rng = Rng::new(99);
    let g = carnivore(&mut rng, 0.5);
    assert_eq!(w.spawn_genome(&g, 20), 20);
    w.step(10);
    assert!(kinds(&w).contains(&EV_FIRST_HUNTERS), "{:?}", kinds(&w));
    // Starve every carnivore; next sample should notice they are gone.
    for i in 0..w.creatures.capacity() {
        if w.creatures.alive[i] && w.creatures.genome[i].diet > 0.66 {
            w.creatures.energy[i] = -1.0e6;
        }
    }
    w.step(10);
    let k = kinds(&w);
    let first = k.iter().position(|k| *k == EV_FIRST_HUNTERS).unwrap();
    let gone = k.iter().position(|k| *k == EV_HUNTERS_GONE).expect("hunters gone");
    assert!(gone > first);
    assert_eq!(k.iter().filter(|k| **k == EV_FIRST_HUNTERS).count(), 1);
}

#[test]
fn lineage_dominance_and_loss() {
    let mut w = World::new(12, cfg());
    let mut rng = Rng::new(7);
    let mut g = Genome::random(&mut rng);
    g.hue = 0.5;
    w.spawn_genome(&g, 400);
    w.step(10);
    assert!(kinds(&w).contains(&EV_LINEAGE_DOMINANT));
    // Wipe out everything in that hue bin and its neighbours, so no newborn
    // can drift back into the bin before the next sample.
    let bin = (0.5 * HUE_BINS as f32) as usize;
    for i in 0..w.creatures.capacity() {
        let b = ((w.creatures.genome[i].hue * HUE_BINS as f32) as usize).min(HUE_BINS - 1);
        if w.creatures.alive[i] && b.abs_diff(bin) <= 2 {
            w.creatures.energy[i] = -1.0e6;
        }
    }
    w.step(10);
    let hue = (bin as f32 + 0.5) / HUE_BINS as f32;
    let lost = (0..w.chronicle.len)
        .map(|i| w.chronicle.event(i))
        .find(|e| e.kind == EV_LINEAGE_LOST && (e.a - hue).abs() < 1e-6)
        .expect("lineage lost for the spawned colour");
    assert!(lost.b > 0.3, "peak share {}", lost.b);
}

#[test]
fn quiet_dish_and_wash_in() {
    let mut c = cfg();
    c.min_population = 0;
    let mut w = World::new(13, c);
    for i in 0..w.creatures.capacity() {
        w.creatures.energy[i] = -1.0e6;
    }
    w.step(10);
    assert!(kinds(&w).contains(&EV_QUIET));
    assert_eq!(w.population(), 0);

    let mut w = World::new(13, cfg());
    for i in 0..w.creatures.capacity() {
        w.creatures.energy[i] = -1.0e6;
    }
    w.step(10);
    let washed = (0..w.chronicle.len).map(|i| w.chronicle.event(i)).find(|e| e.kind == EV_WASHED_IN).expect("washed in");
    assert!(washed.a >= 20.0);
}

#[test]
fn generation_milestones() {
    let mut w = World::new(14, cfg());
    let i = (0..w.creatures.capacity()).find(|&i| w.creatures.alive[i]).unwrap();
    w.creatures.generation[i] = 250;
    w.step(10);
    let ev = (0..w.chronicle.len).map(|i| w.chronicle.event(i)).find(|e| e.kind == EV_GENERATION).expect("generation");
    assert_eq!(ev.a, 200.0);
}

#[test]
fn events_survive_snapshots() {
    let mut w = World::new(15, cfg());
    let mut rng = Rng::new(3);
    w.spawn_genome(&carnivore(&mut rng, 0.1), 20);
    w.step(30);
    let bytes = w.snapshot();
    let r = World::restore(&bytes).unwrap();
    assert_eq!(r.chronicle.total, w.chronicle.total);
    assert_eq!(kinds(&r), kinds(&w));
    assert_eq!(r.snapshot(), bytes);
}
