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
fn rejects_garbage() {
    assert!(World::restore(b"nope").is_err());
    let mut a = World::new(1, cfg());
    a.step(10);
    let mut bytes = a.snapshot();
    bytes.truncate(bytes.len() / 2);
    assert!(World::restore(&bytes).is_err());
}
