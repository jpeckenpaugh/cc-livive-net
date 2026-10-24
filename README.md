# cc-livive-net

A deterministic, `no_std` 3×3×3 **colour cube** environment for reinforcement
learning and solver research: an 18-action state machine, cubie and facelet
encodings, seeded uniform scrambles, canonical action masks, and two solvers
(Thistlethwaite and Kociemba). The core is dependency-free and shared by the
native library, the CLI, and the WASM browser demo — there is no second cube
implementation in JavaScript.

The cube is called a **colour cube** in user-facing copy. *Rubik's Cube* is a
trademark of its owner; it is not used in the project name, crate names, or
domain.

## Status

Measured, not aspirational. Dates and machine context are in
[`docs/SOLVERS.md`](docs/SOLVERS.md).

| Milestone | Scope | State | Evidence |
|---|---|---|---|
| M0 | Core state machine, 18 moves, invariants | Done | 10k round-trips, `no_std` build |
| M1 | Seeded scrambles, notation, observations, masks | Done | 32 golden hashes, 10k seeds |
| M2 | Coordinates, tables, Thistlethwaite | Done | ≤41 moves on 1,000 cubes, median 0.12–0.25 ms |
| M3 | Kociemba two-phase | Done | 16–22 moves (mean 19.86), median 245–302 ms |
| M4 | WASM wrapper, browser demo, Pages | Done | 7.71 MiB raw / 2.63 MiB gzip; 10 browser assertions |
| M5 | Docs, licensing, benchmarks | Done | this README, `docs/RL.md`, `docs/SOLVERS.md`, `bench` |
| M6 | Optional optimal oracle | Not started | optional |

## Quick start

Everything runs in the pinned Docker image (D002); the host needs Docker, `git`,
and an editor.

```sh
./scripts/docker.sh build          # build the dev image (first run is slow)
./scripts/check.sh                 # fmt, clippy, no_std, tests, wasm, node smoke
```

### Core API

The crates are not published to crates.io. Inside this workspace, examples and
crates use path dependencies:

```toml
[dependencies]
color-cube = { path = "crates/color-cube" }
color-cube-solver = { path = "crates/color-cube-solver" }
```

An external crate can depend on the repository directly instead:

```toml
[dependencies]
color-cube = { git = "https://github.com/jpeckenpaugh/cc-livive-net" }
color-cube-solver = { git = "https://github.com/jpeckenpaugh/cc-livive-net" }
```

```rust
use color_cube::{Cube, Move, N_MOVES, N_OBSERVATIONS};

let mut cube = Cube::from_seed(42);          // deterministic uniform state
cube.apply(Move::from_index(0).unwrap());    // U
let facelets = cube.facelets();              // [Color; 54], U R F D L B
let mut observation = [0.0f32; N_OBSERVATIONS];
cube.write_observation(&mut observation);    // 324-float one-hot
let mut mask = [0.0f32; N_MOVES];
cube.write_action_mask(&mut mask);           // 18 ones
```

The complete, compiled example is
`crates/color-cube-cli/examples/quickstart.rs`:

```sh
./scripts/docker.sh cargo run --release -p color-cube-cli --example quickstart
```

```text
seeded: valid=true solved=false
first face after U: [L, R, L, L, U, B, L, R, R]
observation: len=324 sum=54
action mask sum=18
```

### Solvers

Tables are generated before the first solve (they are gitignored):

```sh
./scripts/gen-tables.sh
./scripts/docker.sh cargo run --release -p color-cube-cli --example solve
```

```text
moves=37 solution=F U' B R L F U D2 R' D' L' B2 D R L R2 U2 B2 U F2 U2 R2 F2 R2 F2 D R2 U2 R2 B2 R2 U2 B2 U2 B2 U2 R2
replay solved=true
```

```rust
use color_cube::Cube;
use color_cube_solver::{Limits, Solver, Thistlethwaite};

let cube = Cube::from_seed(42);
let solution = Thistlethwaite::new().solve(&cube, Limits::default())?;
let text: Vec<&str> = solution.moves.iter().map(|mv| mv.to_singmaster()).collect();
println!("{}", text.join(" "));
```

### CLI

```sh
./scripts/docker.sh cargo run --release -p color-cube-cli -- scramble --seed 42
# U2 B2 U2 L' U2 B' L' U2 F' U' R' D B U F L2 B U2 D F2 R F2 R2 U2 F2

./scripts/docker.sh cargo run --release -p color-cube-cli -- solve --seed 42
# F U' B R L F U D2 R' D' L' B2 D R L R2 U2 B2 U F2 U2 R2 F2 R2 F2 D R2 U2 R2 B2 R2 U2 B2 U2 B2 U2 R2
# solver=thistlethwaite phases=6 9 11 11 nodes=71813 elapsed_ms=26.786

./scripts/docker.sh cargo run --release -p color-cube-cli -- solve --seed 42 --solver kociemba
# U' B' U2 F' D' L D2 B R' B D2 F' R2 U F2 U D2 L2 F2
# solver=kociemba phases=12 7 total=19 elapsed_ms=334.194

./scripts/docker.sh cargo run --release -p color-cube-cli -- check --seed 42
# valid=true solved=false hash=59ed2d1424972c7c

./scripts/docker.sh cargo run --release -p color-cube-cli -- bench --n 100
```

Bench defaults to `n=100`, 10 warmups, both solvers; add `--gen` for table
generation timings, `--solver` to pick one. Timings are release-only and
indicative; the current numbers and caveats are in `docs/SOLVERS.md`.

### Browser demo

`web/` is plain HTML/JS over the wasm module; it keeps only a facelet snapshot
and never re-implements cube logic. The Pages workflow publishes it.

```sh
./scripts/browser-smoke.sh        # Playwright: scramble -> solve -> playback
```

## Reinforcement learning

The full contract is in [`docs/RL.md`](docs/RL.md):

- 18 actions in `U R F D L B` order, HTM costs, inverses;
- 324-float observation (`facelet * 6 + color`), one-hot, sum 54;
- unrestricted mask by default, canonical mask pruning per D014;
- reward constants (+1 solved, −0.01 per move, 100-move cap) and curriculum
  caveat;
- frozen `from_seed` determinism and golden hashes;
- a zero-dependency rollout example (`examples/rollout.rs`).

## Solvers

[`docs/SOLVERS.md`](docs/SOLVERS.md) has phases, table sizes, generation
timings, and every measured number.

- **Thistlethwaite**: four phases, measured diameters 7/10/13/15, worst 41
  moves, median 0.118 ms on the 1,000-cube M2 gate, replay-verified.
- **Kociemba**: two-phase IDA*, 16–22 moves (mean 19.86), median 245 ms on the
  1,000-cube M3 gate. The `<50 ms` typical target is a known, accepted miss
  (D025).
- The browser ships **Thistlethwaite only** (D004); Kociemba tables stay
  native/CLI. Both solvers replay every solution before returning it.

## Architecture

```
crates/color-cube/          no_std core: state, moves, notation, scrambles, RL surface
crates/color-cube-solver/   std: coordinates, tables, Thistlethwaite, Kociemba
crates/color-cube-cli/      solve / scramble / check / bench
crates/color-cube-wasm/     wasm-bindgen wrapper over the same core
web/                        static demo (HTML + JS, no cube logic)
```

The core has zero dependencies, including dev-dependencies (D008). `Cube` is
pure state; counters and episodes live in callers (D007). Tables are generated
from the conventions, never transcribed, and carry a magic number, version,
parameter hash, and checksum.

## Build, test, CI

| Command | What it does |
|---|---|
| `./scripts/docker.sh build` | Build the pinned Rust/Node/wasm-pack image |
| `./scripts/gen-tables.sh` | Generate the three table bundles into `tables/` |
| `./scripts/check.sh` | fmt, tables, clippy, `no_std`, tests, both wasm builds, Node smoke |
| `./scripts/browser-smoke.sh` | Playwright browser test |
| `./scripts/docker.sh cargo doc --no-deps` | Rust API docs |
| `./scripts/docker.sh cargo run --release -p color-cube-cli -- bench` | Benchmarks |

GitHub Actions runs the same Docker gates in three jobs: `rust`, `wasm`, and
`browser` (`.github/workflows/ci.yml`); Pages deploys `web/` via
`.github/workflows/pages.yml`. `scripts/check.sh` is the single local gate and
is what "green" means (D002).

## Attribution and licence

Solvers are implementations from published descriptions. Cite **Herbert
Kociemba**, *Two-phase algorithm* (1992), and **Morwen Thistlethwaite** (1981).
No solver source or table data is copied; tables are generated by this
repository from the conventions in `concept.md` §3.

Project code is dual-licensed under either of:

- MIT — [`LICENSE-MIT`](LICENSE-MIT)
- Apache License 2.0 — [`LICENSE-APACHE`](LICENSE-APACHE)

at your option. The root [`LICENSE`](LICENSE) keeps the MIT text. The Apache text
was fetched verbatim from
<https://www.apache.org/licenses/LICENSE-2.0.txt> on 2026-10-24
(SHA-256 `cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30`).

`concept.md` is the authoritative technical brief; `docs/DECISIONS.md` records
the numbered deviations from it.
