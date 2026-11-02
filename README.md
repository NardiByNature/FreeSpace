# Tidepool

An evolving ecosystem you can watch.

Tidepool is a 2D petri dish in the browser. Every creature carries a genome:
a body plan (size, speed, sense range, diet, breeding threshold, mutation
rate, a colour) and the weights of a small neural network that reads what is
nearby and decides how to turn, how hard to swim, and whether to bite. Bushes
regrow from a patchy soil. Creatures graze, hunt, breed, starve, and die.
Nothing is scripted. Press play and selection does the rest: within a few
thousand ticks, steering toward food appears out of random weights, colours
drift into lineages, and if you leave it long enough, something starts eating
its neighbours.

Click any creature to watch its brain fire.

## Run it

```bash
npm ci
npm run dev
```

Then open the printed URL. `npm run build` writes a static `dist/`; serve it
with any static file server (`npm run preview` does), since browsers refuse
to load WebAssembly modules from a bare `file://` page.

Requirements: Node 20+, Rust with the `wasm32-unknown-unknown` target, and
[wasm-pack](https://drager.github.io/wasm-pack/). On first build wasm-pack
compiles a matching `wasm-bindgen-cli` from source; that takes a minute.

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

## Controls

| Action | How |
| --- | --- |
| pause / play | space, or the button |
| speed | 1x / 4x / 16x / max (as many ticks as fit each frame) |
| pan, zoom | drag, scroll wheel; `f` fits the whole dish |
| inspect | click a creature; `Esc` to deselect; tick "follow" to track it |
| new world | type a seed (or leave blank for random), pick the rule, press New world |
| save / load | a `.tdpl` snapshot restores the exact state, RNG included |
| rewind | click a dot on the timeline strip, or `[` for the previous checkpoint |
| add life | drops 50 random founders in, for when a dish goes quiet |
| specimens | Copy genome / Keep specimen on a creature; Paste genome drops 20 copies in |

The URL carries the seed and the rule (`#seed=…`, `#seed=…&sexual`). The
same seed always produces the same run, in every browser, because the whole
simulation is one WebAssembly module with no platform-dependent maths in it.

## Watching it

**Chronicle.** The side panel narrates milestones as they happen: the first
hunters, a lineage dying out, booms and crashes, a colour taking over,
generation marks, newcomers washing in. The same events show as marks on the
population chart. Everything in it is detected from the population samples;
nothing is scripted. The native test build and the browser produce the same
chronicle at the same ticks for the same seed.

**Time machine.** A checkpoint is kept every 2,000 ticks (the strip above the
charts). Click one to rewind. Because a checkpoint restores the world bit for
bit, a rewound dish that you leave alone replays exactly the future you just
watched; touch anything (add life, paste a genome) and it branches. Checked
in the browser: rewind ten thousand ticks, replay, compare snapshots, identical.

**Hall of fame.** Oldest living, most meals, highest generation, each with a
button that selects the record holder. Follow one with the camera and it
draws its path.

**Specimens.** Any creature's genome fits in a 700-character code. Copy it,
keep it in the browser with a name, drop twenty copies into another world.

## How it works

The simulation core is Rust compiled to WebAssembly (`core/`). The browser
side is TypeScript with Canvas 2D (`src/`). The renderer reads creature,
plant, nutrient and history data through typed-array views straight into
WASM memory; nothing is copied per frame except one creature's inspector row.

**World.** A torus, 2048 by 2048 units, over a 64 by 64 nutrient grid. Soil
fertility comes from seeded value noise, so there are rich patches and
barren ground. Nutrients regrow toward capacity and diffuse. Bushes sprout
where soil is rich, pull nutrients up until full, and are grazed a bite at a
time. Corpses return half their energy to the soil.

**Genome.** Seven clamped traits plus 123 network weights. Offspring are
copies with gaussian noise, scaled by a heritable mutation rate. Asexual.

**Brain.** Twelve inputs (nearest bush and nearest creature in body-relative
coordinates with proximity, the neighbour's relative size and colour
difference, own energy and age, a slow clock, a bias), eight tanh hidden
units, three outputs: turn, thrust, bite.

**Energy.** Basal cost grows with size, movement with thrust and speed,
sensing with range. Grazing yields `(1 - diet)` of what is taken; a bite
yields `sqrt(diet)`, and bite strength also scales with `sqrt(diet)`, so a
grazer's nip is weak and nearly free while a hunter's bite is costly and
brutal. That shape matters: it means the first small steps toward predation
already pay for themselves, so carnivores can actually evolve instead of
being stuck across a fitness valley. Breeding happens automatically above a
heritable energy threshold and gives the child 40% of the parent's energy.

**Two rules.** By default creatures bud: a child is a mutated copy of one
parent. The *sexual* rule is an experiment you can switch on per world. A
ready creature looks for a willing, ready neighbour whose colour is close
enough for both parties' heritable choosiness; the child takes each hidden
unit of the brain whole from one parent, each trait from one parent, then
mutates. A creature that waits 600 ticks without a partner buds alone.

The hypothesis was that mate choice by colour would turn colour drift into
real species. Across three seeds and 80,000 ticks it did not. Sexual worlds
ran larger populations (about 3,300 versus 1,800), but their colour bands
merged into one or two instead of separating, hybrids stayed near zero,
choosiness drifted with no direction, and no seed produced carnivores where
the budding worlds did. Recombination breaks up the coadapted brain-and-diet
combinations that predation needs, and nothing in the model penalises a
mismatched pair, so there is no pressure for choosiness. That is a real
result, and the rule ships off by default. Ideas for making it work: a cost
to hybrid offspring between differently adapted parents, or niches that
differ enough that a generalist loses. The headless harness has the numbers:
`cargo run --release --example run -- 1 80000 10000 sexual=1`.

**Determinism.** One seeded xoshiro128** generator owned by the world, fixed
iteration order, and sin/cos/tanh built from arithmetic only (`fastmath.rs`),
so the native test build and the WASM build agree bit for bit. The test suite
checks that two worlds from one seed produce identical snapshot bytes after
two thousand ticks, and that a restored snapshot continues identically.

**Performance.** Struct-of-arrays storage, a uniform-grid spatial hash with
ring-ordered nearest-neighbour search and early exit, and no per-tick
allocation. On an M-series laptop the headless core runs around 2,000 ticks
per second with 2,000 creatures and well over 500 with 9,000.

## Develop

```bash
cargo test --workspace          # simulation tests
npm run check                   # TypeScript
cargo run --release --example run -- <seed> <ticks> <report_every> [key=value …]
cargo run --release --example profile -- <seed> <warmup_ticks>
```

The `run` example prints population, diet classes, mean traits, the
food-seeking score and bites per sample; it accepts overrides such as
`regrow=0.002 bite=40 terrain=0.8` for tuning without a rebuild.

## Dependencies

Two npm packages (`vite`, `typescript`), one Rust crate (`wasm-bindgen`).
See [SECURITY.md](SECURITY.md) for how they were vetted.

## Ideas not done yet

WebGL point sprites for 50k+ bodies, the simulation in a Web Worker, a
lineage tree for the selected creature, seasons, a hybrid-fitness cost that
might make the sexual rule produce species, and branches you can jump
between on the timeline.
