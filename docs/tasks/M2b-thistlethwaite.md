# M2b — Thistlethwaite solver and CLI

**Read first:** `concept.md` §9.1, §9.5, §12 (solver tests), §13, and the
amendment in §9.1; `docs/DECISIONS.md` (especially D019); `docs/tasks/M2a-*.md`;
the existing solver crate.

**Mode:** question-first. Return questions before writing files. No git. Use
`./scripts/docker.sh`.

## Goal

A deterministic Thistlethwaite solver that always solves, ≤45 moves, plus a CLI
`solve` command. This is the browser's default solver (D004), so keep tables
small and generation fast.

## Group chain (D019 — use exactly this)

| Phase | Goal | Allowed moves | Max | Coordinate |
|---|---|---|---|---|
| 1 | `G1 = <U,D,L,R,F2,B2>` | all 18 | 7 | `flip` |
| 2 | `G2 = <U,D,L2,R2,F2,B2>` | `G1` moves (14) | 10 | `(twist, slice)` pair |
| 3 | `G3 = <U2,D2,L2,R2,F2,B2>` | `G2` moves (10) | 13 (measured 9) | `G2/G3` quotient |
| 4 | solved | `G3` moves (6) | 15 | `cp_rank`, `ep_full = ep8_rank * 24 + sp_rank` |

### Phase details

1. **Phase 1 table**: distance over `flip` (2,048 entries), BFS **backwards**
   from `0` over all 18 inverse moves. `G1` requires only edge orientation:
   quarter-turn `L`/`R` move U/D edges into the E slice, so `G1` does not
   preserve the slice. Exact table, so phase 1 is a deterministic greedy
   descent: pick the lowest-index move whose child has distance one less.
2. **Phase 2**: `(twist, slice)` pair, `2187 * 495 = 1,082,565` entries, BFS
   backwards from `(0, SLICE_SOLVED)` over the 14 `G1` moves. This is what
   reaches `G2`: corner orientation *and* the four E-slice edges in place.
   Greedy descent on the pair.
3. **Phase 3**: quotient coordinate `(corner_tetrads, ud_edge_orbits)`.
   - Corner tetrads: positions `{0,2,5,7}` and `{1,3,4,6}` (the two G3 corner
     orbits). Encode for each position which tetrad its piece belongs to.
   - U/D edge orbits: `{UR,UL,DL,DR} = {0,2,6,4}` and
     `{UF,UB,DF,DB} = {1,3,7,5}`.
   - Encode as a compact integer (measured space: 4,900 states), build the
     transition table by composing the existing move tables, then BFS backwards
     from the goal.
   - Greedy descent. **Before leaving phase 3, assert the cube is in `G3`**
     with an explicit predicate (`co == 0`, `eo == 0`, `slice == SLICE_SOLVED`,
     corner tetrads correct, U/D edge orbits correct). If the predicate and the
     quotient ever disagree, fail loudly.
4. **Phase 4**: IDA* inside `G3` with `h = max(cp_dist, ep_full_dist)`:
   - `cp_rank` (40320) with the six `G3` moves,
   - `ep_full = ep8_rank * 24 + sp_rank` (967,680 states). A phase-4 state is
     only solved when both the eight U/D edges and the four slice edges are in
     place; an `ep8`-only coordinate would accept `sp != 0` states.
   - Transitions are composed from the existing `move_ep8`/`move_sp` tables, so
     no 967,680-row move table is stored.
   - Two distance tables (40320 and 967,680 bytes), BFS backwards from identity.
   Deterministic move order (ascending index); bound grows from `h` to 15.
   If 15 is exceeded, return `NoSolutionWithinLimits` rather than looping.

Before committing to the 15 bound, verify it with an exhaustive BFS over the
reachable `G3` state space in a `#[ignore]`d release-mode test or a one-off
`gen-tables --verify` mode, and print the true diameter. The measured reachable
count is 663,552 (not 3.98M) with diameter 15. If the diameter is 16 or more,
stop and escalate rather than relaxing the ≤45 contract.

All phase transitions operate on the real `Cube`; models are only heuristic
scaffolding. Keep the `Cube` in sync and never trust a coordinate as the source
of truth for the output.

## Tables

- Add a second bundle `tables/cc-thistlethwaite-v1.bin` (magic `b"CCTH"`,
  version 1), reusing the M2a header/checksum/atomic-write code by factoring it,
  not by copy-paste.
- Table ids: `1 = phase1_flip (u8, 2048)`, `2 = phase2_twist_slice (u8,
  1082565)`, `3 = phase3_quotient (u8, 4900)`, `4 = phase4_cp (u8, 40320)`,
  `5 = phase4_ep_full (u8, 967680)`.
- Extend `gen-tables` to write both bundles; print per-table time and size.
- Transition generation must compose the existing move tables, not copy a
  `Cube` per state; phase 1 alone is ~18M transitions.
- `Thistlethwaite::new()` only **loads** (so `TablesMissing` is reachable);
  provide `Thistlethwaite::generate(dir)` separately for tests/dev. The solver
  loads both bundles: `cc-tables-v1.bin` (move tables) and
  `cc-thistlethwaite-v1.bin` (distances).
- `TableCorrupt` from the bundle loader maps to `SolveError::TablesMissing`
  (the spec's `SolveError` has no corrupt variant); log the distinction in the
  CLI.

## Solver API (`color-cube-solver`)

Per `concept.md` §9.5:

```rust
pub struct Limits { pub max_length: Option<u8>, pub time_ms: Option<u32> }
pub struct Solution { pub moves: Vec<Move>, pub optimal: bool, pub phases: Option<[usize;2]> }
pub trait Solver { fn solve(&self, cube: &Cube, limits: Limits) -> Result<Solution, SolveError>; }
pub enum SolveError { AlreadySolved, Timeout, NoSolutionWithinLimits, InvalidState, TablesMissing }
```

- `Thistlethwaite` implements `Solver`, is `Send + Sync`, and uses `OnceLock`
  for tables; do not mutate global state during a solve.
- `solve(SOLVED)` returns `Err(AlreadySolved)` (pick this; test it).
- `InvalidState` for `!cube.is_valid()`.
- `time_ms` is checked between phase steps, not inside tight loops; when it
  fires return `Timeout` and never a partial/invalid move list.
- `max_length` is enforced on the combined solution.
- **Replay verification is mandatory**: apply the produced moves to a copy of
  the input and assert solved before returning; a failure is an internal bug,
  surfaced as an error, not a silent bad solution.
- Expose `solve_with_phases(&self, cube) -> Result<(Solution, [usize;4]), _>`
  or equivalent so tests and the CLI can report the per-phase split.

## CLI (`color-cube-cli`)

Keep it dependency-free (`std` only):

```
color-cube-cli solve --seed <u64> [--time-ms N] [--max-length N]
color-cube-cli solve --moves "U R2 F'"
color-cube-cli scramble --seed <u64> [--turns N]
color-cube-cli check --seed <u64>
```

- `solve --seed` uses `Cube::from_seed` (uniform) and prints the Singmaster
  solution plus phase lengths and elapsed time to stderr.
- `solve --moves` parses Singmaster via `parse_moves`; a dot product cube is
  not needed.
- Exit non-zero with a readable message on parse/solve errors. No panics for
  user input.

CLI details: binary name `color-cube-cli`; `--seed` and `--moves` are mutually
exclusive and one is required; default `scramble --turns` is 25; solution text
goes to stdout and phase lengths/timing to stderr; `AlreadySolved` exits 0 and
prints an empty solution line; `--time-ms 0` means "timeout before the first
phase" and is the deterministic test hook; `check --seed` prints
`valid=<bool> solved=<bool> hash=<fnv1a-hex>` and exits non-zero only if invalid.

Solve time after warm tables: target <20 ms typical; report the distribution
over the 1,000-cube run.

## Tests (all must run green)

1. 1,000 `from_seed` cubes: solved, replay-verified, ≤45 moves, each phase
   ≤ its max, subgroup assertion after each phase.
2. Determinism: same cube twice → identical move list; the 1,000-cube run twice
   → identical aggregate (length, hash).
3. Golden: commit 16 `(seed -> solution Singmaster)` fixtures; a mismatch means
   a convention change.
4. `AlreadySolved`, `InvalidState`, `Timeout` (time_ms = 0 or 1),
   `max_length` too small.
5. Table corruption → `TablesMissing`/`TableCorrupt`; missing directory →
   `TablesMissing`.
6. Property: for 100 random *scrambles* (not uniform) too, the solution is
   ≤45 and replays.
7. CLI smoke: `solve --seed 42` prints a replayable solution (parse it back and
   apply); malformed `--moves` exits non-zero.

## Out of scope

Kociemba, optimality claims, WASM, browser. Do not change M0/M1/M2a public APIs
except additions.

## Reporting

Files, commands with output, table sizes and generation times, solve-time
distribution (min/median/max over the 1,000), interpretations, unresolved
items. Under 900 words.
