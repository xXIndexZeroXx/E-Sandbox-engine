# E-Sandbox engine

The simulation engine of E-Sandbox: [ngspice-rs](https://github.com/michaelnavazhylau/ngspice-rs)
(a Rust implementation of ngspice) with a small WebAssembly wrapper, built into two files that a
web page loads: `engine_bg.wasm` and its loader `engine.js`.

This folder is complete on its own. Everything that goes into the engine files is here or is
fetched from a public source at a pinned revision, so anyone can rebuild them, change them, and
use their own build.

## What is in it

| Path | What | Licence |
|---|---|---|
| `src/lib.rs` | The wrapper: takes a SPICE deck, runs it, hands the results to JavaScript | BSD-3-Clause (`LICENSE`) |
| `patches/` | Changes made to ngspice-rs, as patch files | BSD-3-Clause, as ngspice-rs |
| `fetch-source.mjs` | Fetches ngspice-rs at the pinned revision and applies the patches | BSD-3-Clause (`LICENSE`) |
| `vendor/ngspice-rs/` (fetched, not committed) | The simulator's source as built | Its own: see below |

## Licences of what is built in

Nothing here changes any of them.

- **ngspice-rs**: BSD-3-Clause by its `Cargo.toml`; its `README` says "Modified BSD, the same
  license as ngspice". Its `NOTICE`, `COPYING` (ngspice's, with the list of ngspice files under
  other terms), `LICENSE-ECL-2.0` and `AUTHORS` arrive with the fetched source in
  `vendor/ngspice-rs/`.
- **SPARSE 1.3** (translated in ngspice-rs): the Berkeley notice in `NOTICE`.
- **BSIM4 4.8.3** (translated in ngspice-rs): Educational Community License 2.0.
- **Rust crates** in `Cargo.lock` (wasm-bindgen, serde, petgraph, winnow and others): MIT,
  Apache-2.0, Zlib or Unlicense, each crate's own.

ngspice's `COPYING` lists `src/frontend/numparam` as LGPL v2 or later. ngspice-rs reproduces
that part's behaviour in its own code and states BSD-3-Clause for it. In case that part is ever
held to carry the LGPL, this folder already gives what the LGPL asks for: the complete source of
the engine files, the changes made, and the means to rebuild them.

## Changes made to ngspice-rs

| Patch | What it changes |
|---|---|
| `patches/switch-step-bound.patch` | A switch's time-step bound that is not finite is ignored, as ngspice's `swtrunc.c` does, instead of stopping the run |

## Building

Needs Rust, [`wasm-pack`](https://rustwasm.github.io/wasm-pack/), Node.js and git.

```sh
node fetch-source.mjs
wasm-pack build . --release --target web --out-dir pkg --out-name engine
```

The result is `pkg/engine_bg.wasm` and `pkg/engine.js`. `ngspice-rs` is pinned by revision in
`Cargo.toml`; `fetch-source.mjs` puts that revision, patched, in `vendor/ngspice-rs`, which
Cargo's `[patch]` section points to.

## Using your own build in E-Sandbox

In the app: Settings, About, **Simulation engine**, Use my own build. Choose your
`engine_bg.wasm` (and your `engine.js` if you changed the wrapper's interface) and reload. The
files stay in your browser.
