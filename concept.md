# cc-livive-net — concept & build specification

> Deterministic, `no_std` 3x3 color-cube state machine for reinforcement
> learning: 18-move API, cubie/facelet encodings, action masks, seeded uniform
> scrambles, plus Thistlethwaite and Kociemba two-phase solvers.

This document is the authoritative brief for an agent building the project from
scratch. It fixes the conventions (which are the main source of bugs in cube
code), the public API, solver designs, test strategy, and milestones. When
something is marked **DECIDED**, do not re-litigate it without asking; when
marked **OPEN**, make the smallest reasonable choice and record it.

Live demo target: <https://cc.livive.net/>
Repository: <https://github.com/jpeckenpaugh/cc-livive-net>
License: MIT OR Apache-2.0 (same dual license as `mine-sm`).

---

## 1. Purpose

Build a fast, deterministic **3×3×3 color-cube environment** — not a game
front-end — that can be consumed by:

1. **Reinforcement-learning agents**: observation tensors, action masks,
   reproducible scrambles, optional reward helpers.
2. **Solver research/benchmarking**: exact state representation, phase
   coordinates, pruning tables, optimality metrics.
3. **A static browser demo**: apply moves, scramble, solve, and inspect state.

The cube is called a **color cube** in user-facing copy ("Rubik's" is a
trademark; do not use it in the project name or branding).

### Relationship to `mine-sm`

Same philosophy as `/Users/jarad/git/mine-sm`, different scale:

| | `mine-sm` | `cc-livive-net` |
|---|---|---|
| State space | small, enumerable | 4.3252 × 10¹⁹ reachable states |
| Core | `no_std`, bitboards | `no_std`, cubie arrays |
| Action count | 2 × 64 cells | 18 face turns |
| Observation | `[f32; 64]` | `[f32; 324]` (one-hot facelets) |
| Invalid moves | possible | none (every cube move is legal) |
| Solver | none (policy) | Thistlethwaite + Kociemba |
| Browser | hand-written JS mirror | Rust compiled to WASM (no second implementation) |

Do **not** copy the `mine-sm` single-file layout. Use a Cargo workspace.

---

## 2. Goals and non-goals

### Goals (v1)

- Exact, allocation-free core state machine with all 18 face turns.
- Facelet and cubie representations with lossless conversion.
- Uniform random-state generation from a `u64` seed.
- Validity checks for every invariant (orientation sums, permutation parity).
- Thistlethwaite solver: always solves, ≤45 moves.
- Kociemba two-phase solver: typical ~19–21 moves, ≤30; deterministic.
- Browser demo via WASM, with scrambling, playback, and solver stats.
- CI: fmt, clippy, tests, WASM build, browser smoke test.
- Comprehensive tests including property tests and golden positions.

### Non-goals (v1)

- 2×2, 4×4, or N×N cubes.
- Whole-cube rotations (`x`, `y`, `z`) as actions. Centers are fixed in space.
- A trained neural solver (DeepCubeA-style). The env supports one; it does not
  ship one.
- Optimal solving by default (IDA* + pattern databases is an optional oracle).
- Server-side rendering or accounts. The site is static.
- Python/Gym bindings (defer; design the core so PyO3 can wrap it later).

---

## 3. Conventions (read twice — bugs live here)

### 3.1 Faces and colors

**DECIDED**: fixed color scheme, matching the standard Western scheme:

| Face | Letter | Color |
|---|---|---|
| Up | `U` | white |
| Down | `D` | yellow |
| Front | `F` | green |
| Back | `B` | blue |
| Right | `R` | red |
| Left | `L` | orange |

Centers never move. The **solved state is unique**: every face is monochrome
and the centers match the table above.

### 3.2 Facelet ordering

**DECIDED**: use Kociemba's facelet order, `U R F D L B`, 9 facelets per face,
row-major *as viewed from outside that face*, top-left origin:

```
             +------------+
             | U1  U2  U3 |
             | U4  U5  U6 |
             | U7  U8  U9 |
+------------+------------+------------+------------+
| L1  L2  L3 | F1  F2  F3 | R1  R2  R3 | B1  B2  B3 |
| L4  L5  L6 | F4  F5  F6 | R4  R5  R6 | B4  B5  B6 |
| L7  L8  L9 | F7  F8  F9 | R7  R8  R9 | B7  B8  B9 |
+------------+------------+------------+------------+
             | D1  D2  D3 |
             | D4  D5  D6 |
             | D7  D8  D9 |
             +------------+
```

Flat index: `face_index * 9 + offset`, with faces in `U,R,F,D,L,B` order, so
`U1 = 0`, `U9 = 8`, `R1 = 9`, `F1 = 18`, `D1 = 27`, `L1 = 36`, `B1 = 45`.

Facelets store a `Color` (`U/D/F/B/R/L` = 0..5). This is the *output* of the
model, not the internal solver state.

### 3.3 Cubie ordering

**DECIDED**: use the standard Kociemba cubie order.

Corners (`cp[i]` = which corner *piece* is in position `i`):

| Index | Position | Piece |
|---|---|---|
| 0 | URF | URF |
| 1 | UFL | UFL |
| 2 | ULB | ULB |
| 3 | UBR | UBR |
| 4 | DFR | DFR |
| 5 | DLF | DLF |
| 6 | DBL | DBL |
| 7 | DRB | DRB |

Edges (`ep[i]` = which edge piece is in position `i`):

| Index | Position |
|---|---|
| 0 | UR |
| 1 | UF |
| 2 | UL |
| 3 | UB |
| 4 | DR |
| 5 | DF |
| 6 | DL |
| 7 | DB |
| 8 | FR |
| 9 | FL |
| 10 | BL |
| 11 | BR |

Facelet triples/pairs (standard Kociemba mapping):

```
URF: U9 R1 F3      UFL: U7 F1 L3      ULB: U1 L1 B3      UBR: U3 B1 R3
DFR: D3 F9 R7      DLF: D1 L9 F7      DBL: D7 B9 L7      DRB: D9 R9 B7

UR: U6 R2   UF: U8 F2   UL: U4 L2   UB: U2 B2
DR: D6 R8   DF: D2 F8   DL: D4 L8   DB: D8 B8
FR: F6 R4   FL: F4 L6   BL: B6 L4   BR: B4 R6
```

The first facelet of each triple/pair is the **reference facelet** and must
carry the piece's `U`/`D` color (corners, U/D edges) or `F`/`B` color (middle
edges). This drives orientation.

### 3.4 Orientation definitions

**DECIDED**: use the standard definitions:

- **Corner orientation (`co`)**: for a corner in a given position, find its
  `U`/`D`-colored sticker.
  - `0` — it is on the `U` or `D` facelet of the position;
  - `1` — it is on the `R` or `L` facelet;
  - `2` — it is on the `F` or `B` facelet.

  > **Amendment (2026-10-24, D011).** The prose above is not consistent with the
  > facelet triples in §3.3 and violates `sum(co) ≡ 0 (mod 3)` for a legal `R`
  > turn. The definition actually used, and the one Kociemba's tables assume, is
  > the **index of the piece's `U`/`D` sticker within the position's §3.3
  > triple**: `0` for the first facelet of the triple, `1` for the second, `2`
  > for the third. For `URF`, `DFR`, `DRB`, `ULB` this coincides with the
  > axis-based prose; for `UFL`, `UBR`, `DLF`, `DBL` it does not, because those
  > triples are ordered `(U/D, F/B, R/L)`.

  Sum of `co` over all 8 corners ≡ `0 (mod 3)`.

- **Edge orientation (`eo`)**:
  - For U/D-layer edges: `0` if the `U`/`D`-colored sticker is on a `U`/`D`
    facelet, else `1`.
  - For middle-slice edges (FR, FL, BL, BR): `0` if the `F`/`B`-colored sticker
    is on an `F`/`B` facelet, else `1`.

  Sum of `eo` over all 12 edges ≡ `0 (mod 2)`.

Any internally consistent convention works mathematically, but these exact
definitions are assumed by the Kociemba move tables and phase definitions below.
Validate them with the tests in §12.

### 3.5 Moves

**DECIDED**: 18 actions, half-turn metric (HTM), face-major order:

```
0: U    1: U2   2: U'
3: R    4: R2   5: R'
6: F    7: F2   8: F'
9: D   10: D2  11: D'
12: L  13: L2  14: L'
15: B  16: B2  17: B'
```

A move is clockwise as viewed from **outside** that face. `'` is a
counter-clockwise quarter turn; `2` is a half turn. Cost is 1 in HTM, 2 in QTM.

`Move` should be a `#[repr(transparent)] pub struct Move(pub u8)` with
associated constants, so an RL agent can treat it as a discrete action index
`0..18`. Provide:

- `Move::ALL: [Move; 18]`
- `Move::from_index(u8) -> Option<Move>`
- `Move::inverse(self) -> Move`
- `Move::quarter_turns(self) -> u8` (1 or 2)
- `Move::face(self) -> u8` (0..5)
- `Move::power(self) -> u8` (1 = clockwise, 2 = half, 3 = ccw)

### 3.6 State validity invariants

A cubie state is valid iff:

1. `cp` and `ep` are permutations (each piece appears exactly once).
2. `sum(co) % 3 == 0`.
3. `sum(eo) % 2 == 0`.
4. `permutation_parity(cp) == permutation_parity(ep)`.

`Cube::is_valid()` must check all four. `Cube::apply` must preserve all four;
`Cube::from_seed`/`from_facelets` must reject invalid inputs (or return
`Result`).

---

## 4. Core state machine API (`color-cube` crate, `no_std`)

```rust
#![no_std]

pub const N_MOVES: usize = 18;
pub const N_FACELETS: usize = 54;
pub const N_COLORS: usize = 6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cube {
    pub cp: [u8; 8],
    pub co: [u8; 8],
    pub ep: [u8; 12],
    pub eo: [u8; 12],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StepResult {
    pub solved: bool,
    pub move_count: u64,   // caller-maintained or stored in a wrapper
}

impl Cube {
    /// Fully solved cube (identity permutations, zero orientations).
    pub const SOLVED: Cube;

    /// Uniformly random valid state derived from `seed`.
    pub fn from_seed(seed: u64) -> Self;

    /// Random-walk scramble (casual/demo use; NOT uniform).
    pub fn from_scramble(seed: u64, turns: u8) -> (Self, Scramble);

    pub fn apply(&mut self, mv: Move);
    pub fn apply_all(&mut self, moves: &[Move]);
    pub fn apply_inverse(&mut self, mv: Move);

    pub fn is_solved(&self) -> bool;
    pub fn is_valid(&self) -> bool;

    /// 54 colors, Kociemba facelet order.
    pub fn facelets(&self) -> [Color; 54];

    /// Build from facelets; verifies centers and all invariants.
    pub fn from_facelets(facelets: &[Color; 54]) -> Result<Self, CubeError>;

    /// One-hot observation: [facelet][color] => index f*6 + c.
    pub fn write_observation(&self, out: &mut [f32; 324]);

    /// All 1.0 in v1; see §8 for optional pruning.
    pub fn write_action_mask(&self, out: &mut [f32; 18]);

    /// Search-oriented helpers (no allocation).
    pub fn corner_twist(&self) -> u16;  // 0..2186
    pub fn edge_flip(&self) -> u16;     // 0..2047
    pub fn slice_combination(&self) -> u16; // 0..494
}
```

**Scramble** is a fixed-capacity struct so the core stays allocation-free:

```rust
pub struct Scramble {
    pub moves: [Move; 32],
    pub len: u8,
}
```

`from_scramble` should reject/avoid already-solved output (retry with
`seed + 1` or append a harmless move; document the choice).

### Error type

Non-exhaustive enum: `InvalidCenter`, `InvalidPermutation`, `Twist`, `Flip`,
`Parity`, `MalformedFacelets`. Implement `core::fmt::Display` without `std`.

### Determinism

- No floating point in state logic.
- `from_seed` is stable forever: document the exact PRNG (SplitMix64, same
  constants as `mine-sm`) and the sampling order.
- Determinism test: seed → facelet hash must not change across platforms.

---

## 5. Cube moves: derivation, not transcription

**DECIDED**: derive move tables from facelet geometry and verify against known
properties; do not hand-type 18 × 20 cubie arrays.

Recommended approach:

1. Define each of the six 90° face turns as a permutation of the 54 facelets
   (or directly of the 20 cubies with orientation deltas), using the net in
   §3.2.
2. Compose to build `U2 = U·U`, `U' = U·U·U`, and similarly for all faces.
3. Generate the 18 `Cube` transitions by applying the facelet permutation and
   converting back to cubies.

Cross-checks (all must pass before any solver work):

- `apply(X); apply(X); apply(X); apply(X)` returns the original cube for every
  quarter turn `X`.
- `apply(X); apply(X')` is identity for all 18 moves.
- `(U R U' R')` repeated 6 times is identity (sexy move order 6).
- `(R U R' U')` is order 6.
- Known move identities: `U D = D U`, `R L = L R`, `F B = B F` (opposite faces
  commute), etc.
- The standard Kociemba `cpU/cpR/...` tables can be used as a reference in
  tests only (see §12).

If using reference tables, cite Kociemba and keep them in `tests/`, not in the
core path.

---

## 6. Coordinates and tables (solver support)

All coordinates are integers with dense ranges; use `u16`/`u32` and const
bounds.

| Coordinate | Range | Definition |
|---|---|---|
| `twist` | 0..=2186 | `sum(co[i] * 3^i)` for `i = 0..6`; `co[7]` implied by mod-3 |
| `flip` | 0..=2047 | `sum(eo[i] * 2^i)` for `i = 0..10`; `eo[11]` implied by mod-2 |
| `slice` | 0..=494 | rank of the 4-subset of positions holding FR,FL,BL,BR |
| `cp` | 0..=40319 | Lehmer rank of the 8 corner permutation |
| `ep8` | 0..=40319 | rank of the 8 U/D-edge permutation |
| `sp` | 0..=23 | rank of the 4 middle-slice-edge permutation |

Provide bidirectional conversions (`coords::twist(&Cube) -> u16`,
`coords::set_twist(&mut Cube, u16)`, etc.) and property-test round-trips.

**Move tables** (solver crate, `std`):

- `move_twist[2187][18]`
- `move_flip[2048][18]`
- `move_slice[495][18]`
- `move_cp[40320][18]`, `move_ep8[40320][18]`, `move_sp[24][18]`

Store as flat `Vec<u16>`/`Vec<u8>`, row-major. Provide `gen-tables` binary that
writes a versioned binary format with a magic header, parameter hash, and
checksum. Tables are **not committed**; generate once per machine/CI and cache
(see §11).

Pruning tables are 1-byte distances (`0xFF` = unvisited):

- **Kociemba phase 1**: full coordinate space `2187 × 2048 × 495 =
  2,217,093,120`. In practice, symmetry-reduce (16-fold is the usual choice) to
  roughly 138M entries (~139 MB). If that exceeds the browser budget, use the
  split/heuristic approach from `min2phase` or ship Thistlethwaite in WASM
  first.
- **Kociemba phase 2**: two tables of `40320 × 24 = 967,680` entries each
  (`cp × sp`, `ep8 × sp`).

Compute pruning tables by backward BFS from the goal coordinate using the
inverse move tables, propagating `dist[succ] = dist[state] + 1`.

---

## 7. Scrambling

Two distinct APIs, clearly documented:

### 7.1 `Cube::from_seed(seed)` — uniform random state

This is what benchmarks and RL evaluation must use. Algorithm:

1. `cp` — Fisher–Yates shuffle of `[0..8)` using SplitMix64 (or unrank a
   uniform value in `0..40320`).
2. `co[0..7]` — independent uniform values in `0..3`; set
   `co[7] = (3 - sum) % 3`.
3. `ep` — Fisher–Yates shuffle of `[0..12)`.
4. `eo[0..11]` — independent uniform bits; set `eo[11] = sum % 2`.
5. Parity fix: if `parity(cp) != parity(ep)`, swap `ep[10]` and `ep[11]`.
   Swapping two elements is a bijection between odd and even permutations of
   the same set, so the result remains uniform.
6. If the result is solved (probability ~1/4.3e19), retry with `seed + 1` and
   document that this is a measure-zero adjustment.

This produces exactly the uniform distribution over the 4.3252 × 10¹⁹ valid
states; there is no rejection sampling and no bias.

### 7.2 `Cube::from_scramble(seed, turns)` — random walk

For demos and human-facing scrambles only. Use 25–30 turns with immediate
inverse/repeat pruning, then verify not solved. Do **not** use this for
"uniform random cube" claims.

### 7.3 Scramble notation

Provide `to_singmaster(&[Move]) -> String` (`U R2 F' ...`) and a parser for
tests/fixtures. Unknown or malformed tokens are errors, not silently ignored.

---

## 8. RL interface

### Observation

Default: `[f32; 324]`, order `facelet * 6 + color`, one-hot per facelet. This
is permutation-invariant over faces and simple for a policy head.

Optional (behind a feature or separate method): `[f32; 54]` color indices
normalized to `0.0..1.0`, matching the spirit of `mine-sm`'s compact encoding.
Document which is canonical for benchmarks.

### Action mask

Every one of the 18 moves is always legal. For v1:

- `write_action_mask` returns all `1.0`.
- `write_canonical_mask(last_move, out)` (helper, not the default) zeroes
  immediately redundant moves:
  - same face as `last_move`;
  - a move commuting with `last_move` when the pair is in non-canonical order
    (e.g. `D` after `U`, `L` after `R`, `B` after `F`).

This matches the move pruning used by the solvers and avoids the agent learning
`U U'` loops.

### Episode and reward suggestions (documented, not enforced)

- Max 100 moves; terminate on solved.
- Reward: `+1.0` solved, else `-0.01` per move (or `-1` per move with `0` at
  success; publish both as constants).
- Scramble curriculum by **optimal distance**, not raw move count: uniformly
  random states are ~17–18 moves from solved and are a hard starting point.
  Generate easy states by applying `n` random moves and record `n` as a rough
  difficulty label (not true distance).

### `no_std` parity

The core stays `no_std`; RL utilities that need allocation (replay buffers,
solvers) live in `color-cube-solver` or a separate `color-cube-rl` crate.

---

## 9. Solvers

### 9.1 Thistlethwaite (first solver; required)

Four phases, each solved by BFS over a small coordinate, then concatenated.

| Phase | Goal subgroup | Generators after phase | Max moves | Coordinate(s) |
|---|---|---|---|---|
| 0→1 | `G1` | `U D L R F2 B2` | ≤7 | edge flip (2048) |
| 1→2 | `G2` | `U D L2 R2 F2 B2` | ≤10 | corner twist (2187) + slice (495) |
| 2→3 | `G3` | `U2 D2 L2 R2 F2 B2` | ≤13 | corner tetrad + U/D-edge orbit quotient |
| 3→4 | solved | — | ≤15 | corner and edge permutation (40320 each) |

> **Amendment (2026-10-24, D019/D021).** The original table listed the subgroups
> as `<L,R,F,B,U2,D2>`, `<L,R,F2,B2,U2,D2>`, `<L2,R2,F2,B2,U2,D2>`. Those are not
> the Thistlethwaite groups: the first contains quarter-turn `F`, which flips
> edges, so it cannot be the edge-oriented subgroup. The chain above is the
> published one. Phase 1 targets edge orientation only (`G1` does not preserve
> the E slice); phase 2 must target both corner twist and the slice, and its
> coordinate is the pair. See D021 for the measured diameters.

Implementation notes:

- Build BFS distance tables for each coordinate using only the moves allowed in
  that phase.
- At solve time, for each phase, run IDA* (or greedy descent on the distance
  table) to choose moves that reduce the coordinate to 0, then project the
  resulting cube into the next phase.
- Phase 3 must place the four middle-slice edges into the middle slice and the
  corners into their correct tetrads. Track both; a common bug is solving one
  and ignoring the other.
- Phase 4 solves within the half-turn-only group; a small IDA* over corner and
  edge permutations is sufficient.
- Output: `Solution { moves, optimal: false, phases: [usize; 4] }`.
- Assert after every phase that the cube is in the expected subgroup.

Worst case ≤45 moves, tables well under 10 MB, fast enough for WASM.

### 9.2 Kociemba two-phase (required; native, WASM if budget allows)

Phase 1: reach `G1 = <U, D, R2, L2, F2, B2>`, i.e. all corners oriented, all
edges oriented, and the four middle-slice edges in the U/D slice. Search with
IDA* over `(twist, flip, slice)` using the phase-1 pruning table. Bound ≤12.

Phase 2: from a `G1` state, search within `G1`’s 10 move types using
`(cp, ep8, sp)` pruning. Bound ≤18.

Driver:

1. Run phase-1 IDA*; at every node with `h == 0` (in `G1`), run a phase-2
   search bounded by `best_len - depth - 1`.
2. Keep the shortest total solution found; stop when the phase-1 lower bound
   plus current depth cannot beat `best_len`.
3. Respect a wall-clock limit and a max-length limit; return the best found.
4. Deterministic tie-breaking: iterate moves in index order `0..18`; do not use
   hash-map iteration or randomness.

Move pruning (both phases):

- Never apply the same face twice in a row.
- For commuting opposite-face pairs, enforce canonical order: allow `D` only
  after `U`, `L` after `R`, `B` after `F` (or the reverse; pick one and test).
- In phase 2, allow only `U,U2,U',D,D2,D',R2,L2,F2,B2`.

Always verify the final move list by replaying it on a copy of the input cube
and asserting `is_solved()`; never trust the search's internal coordinates.

Expected results: ~19–21 moves typical, ≤30 worst case, milliseconds per cube
once tables are warm.

### 9.3 Optimal solver (optional oracle, post-v1)

IDA* with disjoint pattern databases (corner PDB ~88M entries; edge groups
similarly sized). Useful to:

- verify the true distance of golden positions (e.g. superflip = 20);
- measure the optimality gap of the fast solvers;
- produce `optimal: true` solutions for small user-requested cases.

Not required for the demo; keep it behind a CLI feature flag because memory and
build time are significant.

### 9.4 Human method (optional, demo-only)

A layer-by-layer or CFOP explainer is useful for the website ("here's what the
solver is doing") but is separate from `Solution`. If added, generate
human-readable step labels, not just a move list.

### 9.5 Solver API

```rust
pub struct Limits {
    pub max_length: Option<u8>,
    pub time_ms: Option<u32>,
}

pub struct Solution {
    pub moves: Vec<Move>,
    pub optimal: bool,
    pub phases: Option<[usize; 2]>, // Kociemba split point
}

pub trait Solver {
    fn solve(&self, cube: &Cube, limits: Limits) -> Result<Solution, SolveError>;
}
```

`SolveError`: `AlreadySolved`, `Timeout`, `NoSolutionWithinLimits`,
`InvalidState`, `TablesMissing`.

Solvers must be `Send + Sync` after table initialization and must not mutate
global state during a solve (RW lock the tables or use `OnceLock`).

---

## 10. Browser demo (cc.livive.net)

**DECIDED**: compile the Rust core + solvers to WebAssembly. Do **not** write a
second JavaScript cube engine; dual implementations drift (see `mine-sm`'s
`engine.js`, which needed a conformance suite for exactly this reason).

Stack:

- `wasm-bindgen` wrapper crate exporting JSON-free, typed functions:
  `scramble(seed) -> String`, `solve(facelets) -> String`, `apply(moves)`,
  `observe(facelets) -> Float32Array`, `check() -> CheckReport`.
- `wasm-pack build --target web` or `wasm-bindgen` CLI; commit build output
  under `web/pkg/` or build it in the Pages workflow.
- Plain HTML/CSS/JS front end with an isometric or CSS-3D cube rendered on
  `<canvas>`; no framework required.
- UI features: scramble, reset, apply move buttons, click-to-turn, auto-solve
  with step playback and move counter, solver timing, observation/action-mask
  inspector, and a "run conformance" panel.
- Accessibility: color-blind-safe palette option, keyboard controls, and
  text labels for moves.
- If the full Kociemba phase-1 table exceeds the payload budget, lazy-load it
  as a separate asset or default the browser to Thistlethwaite and label the
  mode (`≤45 moves, small download`) vs Kociemba (`~20 moves, larger download`).

Add `CNAME` = `cc.livive.net` and deploy to GitHub Pages from `main`.

Showcase recording (optional, mirroring `mine-sm`): Playwright script that
records a 30s video, thumbnail, and gallery stills into `demos/latest/`, driven
by a `showcase.config.mjs`. Manual `workflow_dispatch` only.

---

## 11. Repository layout

```
cc-livive-net/
├── Cargo.toml                 # workspace
├── concept.md                 # this file
├── README.md
├── LICENSE-MIT
├── LICENSE-APACHE
├── .github/workflows/
│   ├── ci.yml
│   ├── pages.yml
│   └── showcase.yml           # optional/manual
├── crates/
│   ├── color-cube/            # no_std core
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── color-cube-solver/     # std: coords, tables, Thistlethwaite, Kociemba
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── coords.rs
│   │       ├── tables.rs
│   │       ├── thistlethwaite.rs
│   │       ├── kociemba.rs
│   │       └── bin/gen-tables.rs
│   ├── color-cube-cli/        # scramble/solve/bench
│   └── color-cube-wasm/       # wasm-bindgen wrapper (std)
├── web/
│   ├── index.html
│   ├── app.js
│   ├── style.css
│   └── pkg/                   # generated wasm (gitignored or committed)
├── scripts/
│   ├── check.sh               # fmt + clippy + tests + node smoke
│   ├── gen-tables.sh
│   └── record-showcase.mjs    # optional
├── demos/latest/              # optional showcase output
└── tests/                     # workspace-level integration/property tests
```

Workspace `Cargo.toml`:

```toml
[workspace]
members = ["crates/color-cube", "crates/color-cube-solver",
           "crates/color-cube-cli", "crates/color-cube-wasm"]
resolver = "2"

[workspace.package]
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/jpeckenpaugh/cc-livive-net"

[workspace.lints.rust]
unsafe_code = "forbid"
missing_debug_implementations = "warn"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
```

The core crate must build with `--no-default-features` and `#![no_std]`; the
solver, CLI, and WASM crates may use `std`. Do not add dependencies to the core
crate.

---

## 12. Testing strategy

### Core invariants (must all exist)

- Facelet ↔ cubie round-trip for random valid cubes (10k property cases).
- All 18 moves preserve the four invariants.
- `X^4 = identity` and `X · X' = identity` for all faces.
- `(R U R' U')^6 = identity`, `(U R U' R')^6 = identity`.
- Opposite faces commute; same-axis move composition rules.
- `Cube::SOLVED.is_solved()`, `is_valid()`; solved is unique (exactly one
  facelet arrangement with fixed centers).
- `from_facelets` rejects: wrong centers, duplicate/missing pieces, twist/flip/
  parity violations. One negative test per error variant.
- Coordinate round-trips: for every `twist`/`flip`/`slice`, set then get.
  For permutations, test all 40320 corner/edge ranks (fast) and all 24 slice
  ranks.
- `from_seed`: 10k seeds are valid, not solved, and stable (golden hash).
- `from_seed` uniformity smoke test: corner/edge permutation parity matches;
  orientation sums zero. Do not attempt a statistical proof in CI.

### Move-table tests

- `move_*[coord][mv]` agrees with applying the move to a concrete cube that
  realizes `coord`.
- Composition: `move_table[move_table[x][A]][B]` matches applying `A` then `B`
  on a cube (for coordinates).
- Table files load with the expected magic/version/checksum; corrupt header →
  `TablesMissing`/`TableCorrupt`.

### Solver tests

- **Round-trip**: for 1,000 uniform random cubes, `solve` output applied to the
  cube yields solved.
- **Bounds**: Thistlethwaite ≤45; Kociemba ≤30 and phase-1 ≤12 / phase-2 ≤18.
- **Subgroup assertions**: after each Thistlethwaite phase and after Kociemba
  phase 1, verify the projected coordinates are zero / slice in U/D.
- **Optimality spot checks**: with the optional optimal solver (or a committed
  fixture), assert superflip distance = 20 and a handful of known distances.
- **Golden vectors**: commit `(seed → facelets → solution)` hashes for 100
  seeds; a mismatch means nondeterminism or a convention change.
- **Always-solved property**: `AlreadySolved` is returned for the solved cube,
  not an empty solution (or return an empty solution; pick one and test it).
- **Timeout behavior**: a tiny `time_ms` returns `Timeout`/best-effort without
  panicking, and never returns an invalid move list.
- **Pruning completeness**: solving with canonical pruning must succeed on the
  same 1,000 cubes as without it (guard against over-pruning).

### Useful golden positions

- **Superflip** (all edges flipped, pieces in place): true distance 20 HTM. A
  commonly cited 20-move generator is
  `U R2 F B R B2 R U2 L B2 R U' D' R2 F R' L B2 U2 F2`;
  verify it produces exactly 12 flipped edges and 0 solved corners, and use the
  optimal solver to confirm 20 before hard-coding the assertion.
- **Checkerboard**: `U2 D2 R2 L2 F2 B2`; useful rendering/regression fixture.
- **Six-spot / cube-in-cube** patterns: nice visual fixtures; do not assume
  their distance without an oracle.

### Performance checks

Non-gating benchmarks (criterion or a simple CLI timing mode):

- `apply` throughput: ≥10M moves/s single-threaded (release).
- Thistlethwaite: <20 ms typical.
- Kociemba: <50 ms typical with warm tables; report phase split.
- Table generation: record wall time and size in the table manifest.

---

## 13. CI / build scripts

`scripts/check.sh` (mirror `mine-sm`, readable summary, non-zero exit on
failure):

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. Generate tables if missing (cache by source hash) and run `cargo test --all`
4. `cargo build -p color-cube --no-default-features` (proves `no_std`)
5. `wasm-pack build crates/color-cube-wasm --target web`
6. Node smoke: load `web/pkg`, scramble a known seed, solve, assert solved.
7. Optionally `cargo bench` in report-only mode.

`.github/workflows/ci.yml`:

- `rust` job: fmt, clippy, test (with table cache keyed on
  `crates/color-cube-solver/src` + table params).
- `wasm` job: build wasm, run the browser smoke test (Playwright or a Node
  `wasm-bindgen` smoke), upload artifact.
- Keep jobs independent/parallel; do not serialize Rust on WASM.

`.github/workflows/pages.yml`: build WASM, copy `web/`, deploy to Pages on
`main`. Cache cargo and table generation.

Table artifacts are large; do not add them to git. Use `actions/cache` with a
hash of the generator source and a manifest version. For local dev,
`./scripts/gen-tables.sh` writes to `tables/` (gitignored).

---

## 14. Milestones and acceptance criteria

### M0 — Skeleton and conventions

- Workspace builds; core crate is `no_std` and `forbid(unsafe_code)`.
- `Cube`, `Move`, `Color`, `CubeError` defined; solved state, facelets, and
  conversions implemented.
- 18 moves derived and passing the identity/order tests in §12.
- `is_valid` enforces all four invariants.
- **Done when**: `cargo test -p color-cube` passes, including 10k property
  round-trips.

### M1 — Seeded scrambles and observations

- `from_seed` uniform construction with parity fix; golden hashes committed.
- `from_scramble` random walk; Singmaster formatting/parsing.
- `write_observation` (324 floats) and `write_action_mask` (18 ones).
- **Done when**: 10k seeds valid/unsolved/stable; a documented seed produces a
  documented facelet hash.

### M2 — Thistlethwaite

- Coordinates + move tables + phase BFS/IDA*.
- Solves 1,000 random cubes, ≤45 moves, all replay-verified.
- **Done when**: solver tests green and a CLI `solve` command works.

### M3 — Kociemba two-phase

- Phase-1/2 pruning tables with generator + manifest.
- Deterministic search with time/length limits.
- Typical lengths reported; ≤30 guaranteed within limits.
- **Done when**: 1,000 random cubes solved and replay-verified; optimality gap
  reported against the optional oracle on a sample.

### M4 — Browser demo

- WASM wrapper, cube renderer, scramble/solve/playback/inspector.
- Conformance panel: invariants, move identities, solver round-trips.
- Pages deployment at `cc.livive.net`.
- **Done when**: CI builds WASM and a headless browser test solves a seeded
  scramble end-to-end.

### M5 — RL polish and docs

- README quickstart, observation/action-mask spec, reward conventions.
- Difficulty curriculum helper; benchmark numbers published.
- Optional Python bindings (`color-cube-py`) using PyO3, if requested.
- **Done when**: an external user can add the crate and train a trivial agent
  from the README alone.

### M6 — Optional optimal oracle

- IDA* + pattern databases, CLI-only, `optimal: true`.
- Superflip distance test enabled.
- **Done when**: oracle agrees with Kociemba lengths and proves superflip = 20.

---

## 15. Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Wrong orientation convention breaks phase solvers | High | Derive from facelets; cross-check against Kociemba reference tables in tests; assert subgroup membership each phase |
| Parity bug in `from_seed` | Medium (biased RL eval) | Explicit parity fix + uniformity argument documented + fixture tests |
| Table size blows the WASM budget | Medium | Symmetry reduction, lazy-load, or Thistlethwaite-only browser mode |
| Nondeterministic solver (HashMap iteration, time-based cutoffs) | Medium | Fixed move order; deterministic tie-breaks; time limit is opt-in and documented as affecting output |
| Copying GPL solver code | Legal | Implement from published algorithms; cite Kociemba/Thistlethwaite; no vendored GPL |
| Dual implementation drift | Medium | WASM only; no JS engine |
| `no_std` broken by accidental deps | Low | CI builds core with `--no-default-features`; keep deps at zero |
| Human-facing "solved" ambiguity | Low | Fix centers; unique solved state; document |

---

## 16. Attribution and licensing

- Project code: MIT OR Apache-2.0.
- Cite Herbert Kociemba, *Two-phase algorithm* (1992), and Morwen
  Thistlethwaite (1981) in the README and solver module docs.
- If reference tables or test vectors are adapted, note the source and keep
  them test-only.
- Do not use "Rubik's" in the repo name, crate name, or domain. Use "color
  cube" in UI copy, with a note that Rubik's Cube is a trademark of its owner.

---

## 17. Open decisions (pick sensible defaults and record them)

1. Crate names: `color-cube`, `color-cube-solver`, `color-cube-wasm` (assumed).
2. Browser solver mode: Kociemba if tables fit under ~25 MB compressed,
   otherwise Thistlethwaite default + optional lazy Kociemba.
3. Observation canonical form: 324-float one-hot (assumed) vs 54-float indices.
4. Whether `Cube` stores `move_count` or callers track it (assumed: callers /
   wrapper).
5. Whether to expose facelet-indexed moves (piece moves) or only face turns
   (assumed: face turns only).
6. Python bindings: defer to M5.
7. Showcase recording: optional, only if the demo warrants it.

---

## 18. Agent quick start

```sh
cargo new --lib crates/color-cube
cargo new --lib crates/color-cube-solver
cargo new --bin crates/color-cube-cli
# add workspace members, then:

# 1. Implement core conventions + 18 moves + tests (M0).
cargo test -p color-cube

# 2. Add seeded scrambles and observations (M1).
# 3. Generate tables and implement Thistlethwaite (M2).
cargo run --release -p color-cube-solver --bin gen-tables
cargo test -p color-cube-solver

# 4. Add Kociemba (M3).
# 5. Add wasm wrapper + web demo (M4).
wasm-pack build crates/color-cube-wasm --target web
./scripts/check.sh
```

**Definition of done for v1**: a fresh clone can `./scripts/check.sh` and pass,
the CLI solves any uniform random cube (≤30 moves, replay-verified,
deterministic), and `cc.livive.net` scrambles and solves in the browser without a
JavaScript cube implementation.
