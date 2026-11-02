use tidepool_core::world::{Config, World};

fn cfg() -> Config {
    Config {
        width: 768.0,
        height: 768.0,
        grid_cols: 24,
        grid_rows: 24,
        max_plants: 6_000,
        max_creatures: 3_000,
        initial_creatures: 300,
        initial_plants: 1_500,
        ..Config::default()
    }
}

#[test]
fn same_seed_same_bytes() {
    let mut a = World::new(77, cfg());
    let mut b = World::new(77, cfg());
    a.step(2_000);
    b.step(2_000);
    assert!(a.population() > 0);
    assert_eq!(a.snapshot(), b.snapshot());
}

#[test]
fn different_seed_different_bytes() {
    let mut a = World::new(77, cfg());
    let mut b = World::new(78, cfg());
    a.step(200);
    b.step(200);
    assert_ne!(a.snapshot(), b.snapshot());
}

#[test]
fn restore_continues_identically() {
    let mut a = World::new(5, cfg());
    a.step(1_200);
    let bytes = a.snapshot();
    let mut b = World::restore(&bytes).expect("restore");
    assert_eq!(b.snapshot(), bytes, "round trip is canonical");
    a.step(600);
    b.step(600);
    assert_eq!(a.snapshot(), b.snapshot());
}

#[test]
fn sexual_worlds_are_deterministic_and_restorable() {
    let mut c = cfg();
    c.sexual = true;
    let mut a = World::new(21, c.clone());
    let mut b = World::new(21, c);
    a.step(1_500);
    b.step(1_500);
    assert!(a.population() > 0);
    assert_eq!(a.snapshot(), b.snapshot());
    let bytes = a.snapshot();
    let mut r = World::restore(&bytes).unwrap();
    assert!(r.cfg.sexual);
    a.step(500);
    r.step(500);
    assert_eq!(a.snapshot(), r.snapshot());
}

#[test]
fn seasons_are_deterministic_and_bounded() {
    let mut c = cfg();
    c.season_depth = 0.45;
    c.season_period = 4_000.0;
    let mut a = World::new(31, c.clone());
    let mut b = World::new(31, c);
    a.step(2_000);
    b.step(2_000);
    assert_eq!(a.snapshot(), b.snapshot());
    // Midwinter at half a period: factor is 1 - depth.
    assert!((a.season_factor() - 0.55).abs() < 1e-4, "{}", a.season_factor());
    a.step(2_000);
    assert!((a.season_factor() - 1.0).abs() < 1e-4, "{}", a.season_factor());
    let mut flat = cfg();
    flat.season_depth = 0.0;
    let w = World::new(31, flat);
    assert_eq!(w.season_factor(), 1.0);
}

#[test]
fn rejects_garbage() {
    assert!(World::restore(b"nope").is_err());
    let mut a = World::new(1, cfg());
    a.step(10);
    let mut bytes = a.snapshot();
    bytes.truncate(bytes.len() / 2);
    assert!(World::restore(&bytes).is_err());
}
