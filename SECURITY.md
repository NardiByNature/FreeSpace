# Dependency vetting

Tidepool is built from a deliberately small set of third-party code. This file
records what was checked when each dependency was adopted, so the review is
visible rather than assumed. Re-run the checks when bumping anything.

## Surface

| Layer | Direct dependencies | Why |
| --- | --- | --- |
| npm (dev only) | `vite@8.3.4`, `typescript@5.9.3` | bundler/dev server, type checker |
| Rust | `wasm-bindgen@0.2.130` | JS/WASM bindings |

Nothing on the npm side ships to the browser except Vite's tiny module preload
helper. The runtime is the compiled `.wasm` and our own TypeScript.

Deliberately avoided: a test runner on the npm side (all simulation tests run
under `cargo test`), `rand`/`serde` on the Rust side (the RNG is 40 lines,
snapshots are a hand-rolled byte format), `console_error_panic_hook`, and any
Vite plugin.

## Policy

- `.npmrc`: `save-exact=true`, `ignore-scripts=true`, `fund=false`.
  Vite's native pieces (rolldown, lightningcss) arrive as prebuilt optional
  platform packages, not through install scripts, so scripts stay off with no
  loss. Verified: `vite build` and `tsc` work with scripts disabled.
- Prefer the newest release that is at least 14 days old, unless a newer one
  carries a security fix. Transitive packages are held to the same rule via
  `overrides` in `package.json`.
- `package-lock.json` and `Cargo.lock` are committed. Install with `npm ci`.
- `wasm-opt` is disabled in `core/Cargo.toml` so wasm-pack does not fetch a
  second binary. wasm-pack builds a matching `wasm-bindgen-cli` from crates.io
  source on first use (`cargo install`), it does not download a prebuilt binary.

## Review log

### 2026-11-02, initial adoption

Registry metadata checked with `npm view` before installing.

| Package | Version | Published | Maintainers | Notes |
| --- | --- | --- | --- | --- |
| vite | 8.3.4 | 2026-10-08 | yyx990803, vitebot (voidzero) | 8.3.5 (2026-10-21) reviewed: changelog has no security fixes, so the older pin stands |
| typescript | 5.9.3 | 2026-07 | Microsoft TypeScript team | pure JS; 7.x ships native binaries, not needed here |
| rolldown | 1.2.14 (override) | 2026-10-14 | rolldown team, yyx990803 | Vite 8.3.4 resolves `~1.2.14`; 1.2.16 was 4 days old |
| postcss | 8.5.30 (override) | 2026-10-12 | ai (Andrey Sitnik) | 8.5.32 was 7 days old |
| lightningcss | 1.33.0 | 2026-07-20 | devongovett | |
| picomatch | 4.0.7 | 2026-08-24 | jonschlinkert et al. | |
| tinyglobby | 0.2.17 | 2026-05-30 | superchupu | |
| fsevents | 2.3.3 | 2024-09-18 | pipobscure, paulmillr | macOS only, optional |

After `npm install --ignore-scripts`:

- `npm audit`: 0 vulnerabilities.
- `npm audit signatures`: 17/17 packages have verified registry signatures;
  9 have Sigstore provenance attestations (`vite`, `rolldown`,
  `@rolldown/binding-darwin-arm64`, `@rolldown/pluginutils`, `@oxc-project/types`,
  `postcss`, `nanoid`, `tinyglobby`, `detect-libc`). The remainder
  (`typescript`, `lightningcss`, `lightningcss-darwin-arm64`, `picomatch`,
  `fdir`, `picocolors`, `source-map-js`, `fsevents`) are signature-only; none
  of those projects publish provenance at all, so this is expected rather than
  a red flag for a particular release.
- `npm query` for `preinstall`/`install`/`postinstall`: none in the tree.
- `npm ls --all`: 17 packages, every one accounted for above or a direct child
  of one of them (`nanoid`, `picocolors`, `source-map-js` under postcss;
  `fdir` under tinyglobby; `detect-libc` under lightningcss;
  `@oxc-project/types`, `@rolldown/pluginutils` under rolldown). Native
  binding packages are published by the same maintainers as their parents.

Rust: `Cargo.lock` resolves to `wasm-bindgen` plus its own macro crates
(`wasm-bindgen-macro`, `wasm-bindgen-macro-support`, `wasm-bindgen-shared`,
`bumpalo`, `cfg-if`, `proc-macro2`, `quote`, `syn`, `unicode-ident`,
`rustversion`). All are rust-lang or wasm-bindgen org crates or
proc-macro ecosystem staples with years of history.

### 2026-11-02, phase 4 (chronicle, time machine, specimens, sexual rule)

No dependency changes. The base64 codec for specimen codes is hand-written
in `core/src/codec.rs` rather than a crate. `npm ls --all` and
`npm audit signatures` re-run with the same result as above.

## How to re-check

```bash
npm ci
npm audit
npm audit signatures
npm query ':attr(scripts, [preinstall]), :attr(scripts, [install]), :attr(scripts, [postinstall])'
npm ls --all
cargo tree
```
