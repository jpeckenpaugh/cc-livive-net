# Decision log

Numbered, append-only. Each entry: context, decision, consequence. Product
conventions fixed by `concept.md` §3 are restated here only when a downstream
choice depends on them.

---

## D001 — Commits go directly to `main`; only the PM commits

**Context.** Build is run by staged sub-agents against a single working tree.
**Decision.** Sub-agents never run git and never commit. The PM reviews, runs
the gates in Docker, and commits to `main` in small green steps. No PR flow.
**Consequence.** History is linear and bisectable. Review quality depends on the
PM actually re-running gates rather than trusting the implementer's report.

## D002 — Docker is the only build environment

**Context.** Host is macOS 27 / arm64 with a native Rust toolchain, but builds
must be reproducible and not depend on host state.
**Decision.** All cargo/wasm/node work runs via `./scripts/docker.sh`, which
mounts the repo into a pinned image and keeps `CARGO_HOME` and the target dir in
named volumes. Native tooling on the host is used only for git and editing.
**Consequence.** First build is slow (image + dependency cache). Everything
after is fast. CI can reuse the same Dockerfile.

## D003 — Playwright/Chromium arrives at M4, not now

**Context.** A browser is needed for the WASM smoke test and screenshots.
**Decision.** Keep the base image lean (rust + node + wasm-pack). Add the
browser layer when M4 starts.
**Consequence.** M0–M3 images stay small; M4 rebuild is a one-time cost.
**Superseded by D026:** no browser layer is needed in v1.

## D004 — Browser ships Thistlethwaite by default

**Context.** Kociemba phase-1 pruning reduction is roughly 139 MB, well beyond a
comfortable static-site payload (`concept.md` §6, §17.2).
**Decision.** The WASM bundle exposes Thistlethwaite as the always-available
solver; Kociemba stays native (CLI) and will be offered in the browser only as
an optional lazy-loaded asset if, at M4, it fits an agreed budget.
**Consequence.** Demo download stays small; the "~20 moves" mode is a
progressive enhancement, not a launch requirement.

## D005 — No showcase recording in v1

**Context.** `concept.md` §10 and §17.7 mark the Playwright video as optional.
**Decision.** Out of scope for v1. Revisit after the Pages deployment is live
and stable.
**Consequence.** One less workflow to maintain; `demos/latest/` does not exist.

## D006 — Canonical observation is the 324-float one-hot

**Context.** `concept.md` §8 offers a 324-float one-hot and a 54-float compact
form.
**Decision.** 324-float one-hot (`facelet * 6 + color`) is canonical for
benchmarks and RL. The compact 54-float form is an auxiliary helper only.
**Consequence.** Observation hash fixtures are defined over 324 floats.

## D007 — `Cube` does not store a move counter

**Context.** `concept.md` §4 shows `StepResult { solved, move_count }` but §17.4
leaves storage open.
**Decision.** The core `Cube` is pure state. Counters live in callers and in a
`Run`/`Episode` wrapper if we need one. `StepResult` may be introduced later for
the RL wrapper.
**Consequence.** `no_std`, `Copy`, and cheap. No counter drift between clones.

## D008 — Core crate has zero dependencies, including dev-dependencies

**Context.** `concept.md` §11 forbids dependencies in the core crate.
**Decision.** The ban extends to dev-dependencies. Tests use a hand-written
SplitMix64 and hand-written property loops instead of `proptest`/`rand`.
**Consequence.** `cargo test -p color-cube` works offline in `no_std`-adjacent
conditions, and CI cannot accidentally pull a dependency into the core graph.

## D009 — `Color` discriminants follow the spec's colour order, not face order

**Context.** `concept.md` §3.2 fixes face order as `U R F D L B` but states the
colour encoding as `U/D/F/B/R/L = 0..5`. Those are different orders, and it is an
easy place to introduce a silent permutation bug.
**Decision.** `Color` is a `#[repr(u8)]` enum with discriminants `U=0, D=1, F=2,
B=3, R=4, L=5`, exactly as written in §3.2. Face order is only ever used for
indexing facelets (`face_index * 9 + offset`).
**Consequence.** Conversions between face index and colour are deliberate
tables, never `as u8` arithmetic. Test fixtures must state both orders.

## D010 — Invalid `Move` values are a documented panic

**Context.** `Move(pub u8)` (required by `concept.md` §3.5) lets safe code
construct `Move(200)`.
**Decision.** `from_index` is the checked constructor. `apply`, `inverse`,
`face`, `power`, and `quarter_turns` document a precondition that the index is
`< 18` and panic otherwise. No masking, no silent no-op.
**Consequence.** Internal code only ever builds moves via `ALL`/`from_index`.
A total API can be added later as `try_apply` if a caller needs it.

## D011 — Corner orientation is the index within the position's facelet triple

**Context.** `concept.md` §3.4 describes corner orientation by which face the
`U`/`D` sticker sits on (`0` = U/D, `1` = R/L, `2` = F/B). That prose contradicts
the facelet triples in §3.3 for the four positions ordered `(U/D, F/B, R/L)` —
`UFL`, `UBR`, `DLF`, `DBL`. Read literally it gives `sum(co) = 8 ≡ 2 (mod 3)`
for a legal `R` turn, so no correct cube implementation can use it as written.
The implementer caught this during M0.
**Decision.** Use the index of the piece's `U`/`D` sticker within the position's
§3.3 triple (`0/1/2`). For edges, use the index of the reference sticker
(`U`/`D` for U/D edges, `F`/`B` for middle edges) within the pair. This
reproduces Kociemba's published `co`/`eo` deltas exactly, which is the authority
the solvers depend on. `concept.md` §3.4 carries an amendment note pointing
here.
**Consequence.** Golden fixtures and any future table generator must use this
definition. The visual "which face is the sticker on" description remains
useful only for `URF`, `DFR`, `DRB`, `ULB`.

## D012 — `Cargo.lock` is committed

**Context.** A workspace with pinned path members and (later) solver
dependencies must build reproducibly in CI and Docker.
**Decision.** Commit the root `Cargo.lock`. Binary and workspace builds pin it;
the core crate itself still has no external dependencies.
**Consequence.** Dependency bumps are explicit commits.

## D013 — Integration tests may use `std`; the library may not

**Context.** D008 bans dependencies in the core; `no_std` is a property of the
library, not of its test harness.
**Decision.** `crates/color-cube/tests/*.rs` are ordinary `std` integration
tests, still with zero dependencies (hand-written SplitMix64, reference tables
inline).
**Consequence.** Test ergonomics improve without weakening the `no_std` gate,
which is enforced by `cargo build -p color-cube --no-default-features`.

## D014 — Canonical move ordering prunes the lower face index

**Context.** `concept.md` §8 gives "`D` after `U`" as an example of a pruned
non-canonical pair, while §9 says "allow `D` only after `U`". The two sentences
describe opposite pruning directions. Both are valid conventions, but only one
can be implemented.
**Decision.** Follow §9. Opposite faces commute; keep the move whose face index
is higher (`U=0, R=1, F=2, D=3, L=4, B=5`), so `D` is allowed after `U`, `L`
after `R`, `B` after `F`, and the reverse pairs are pruned. Same-face moves are
always pruned. `last = None` prunes nothing.
**Consequence.** The canonical action mask, the random walk, and later solver
search all share one rule. §8's example is treated as the typo.

## D015 — `from_seed` is a frozen 36-draw contract

**Context.** Golden hashes and benchmark comparability require the sampling
order to never change.
**Decision.** One `SplitMix64` stream per seed, draws in this exact order:
corner Fisher–Yates (7 draws, bounds 8..2), `co[0..7]` (7 draws, bound 3),
edge Fisher–Yates (11 draws, bounds 12..2), `eo[0..11]` (11 draws, bound 2).
`co[7]`/`eo[11]` are derived by sum. The parity fix swaps `ep[10]`/`ep[11]` with
no draw. If the result is solved, restart the whole algorithm with
`seed.wrapping_add(1)`, capped at 1024 attempts (then panic; unreachable).
Bounded draws use modulo, so the distribution is uniform up to a per-draw
deviation below `1/2^64`; this is documented rather than corrected, and no
state-level rejection is used.
**Consequence.** Changing the draw order or the PRNG invalidates every golden
fixture and is a breaking change.

## D016 — Notation and parser semantics

**Context.** The core is `no_std` and allocation-free, but notation is
text-shaped.
**Decision.** `write_singmaster` is all-or-nothing: on a short buffer it
returns `BufferTooSmall` and writes nothing. Tokens are separated by single
spaces with no trailing space; an empty slice writes nothing and returns `Ok(0)`.
`parse_moves` is uppercase-only, rejects unknown tokens, accepts empty or
whitespace-only input as an empty list, and caps at 64 moves
(`TooManyMoves`). Malformed tokens are never skipped.
**Consequence.** Round-tripping is lossless and testable without `alloc`; a
friendlier lowercase-tolerant parser can live in the CLI.

## D017 — Coordinate encodings and setter validity

**Context.** The solver needs to realise a cube from arbitrary coordinate
values, including coordinate pairs that are individually legal but jointly
parity-inconsistent.
**Decision.** `twist` is the seven-term base-3 encoding (`co[7]` implied),
`flip` the eleven-term base-2 encoding, `slice` the combinadic
`C(p0,1)+C(p1,2)+C(p2,3)+C(p3,4)` with `SLICE_SOLVED = 494`, `cp`/`ep8` Lehmer
ranks, `sp` the 24 orderings of the middle edges. Core setters are raw
(`set_slice_combination` may break parity); solver setters are total and repair
parity by swapping within a set that preserves the coordinate just written.
`set_cp_ep8` exists so both permutation coordinates can be realised together.
**Consequence.** "Valid cube" is a property of the solver setter, not the core
raw setter. Tests assert that distinction rather than papering over it.

## D018 — `ep8`/`sp` tables are defined only inside `G1`

**Context.** `ep8` (eight U/D edges in positions 0..7) and `sp` (four middle
edges in 8..11) are undefined the moment a middle-slice edge leaves the slice,
which any quarter turn of `R`, `L`, `F`, `B` does.
**Decision.** Keep the spec's `x 18` table shape, fill non-`G1` columns with
`u16::MAX`/`u8::MAX`, and export `PHASE2_MOVES` (indices `0,1,2,9,10,11,4,7,13,16`).
Phase 2 and all table tests iterate only those moves.
**Consequence.** Phase-2 search cannot silently read garbage; the sentinel is
obvious in debuggers and tests. Tables stay a uniform shape.

## D019 — Thistlethwaite uses the published group chain (corrects §9.1)

**Context.** `concept.md` §9.1 listed `G1 = <L,R,F,B,U2,D2>`, which contains
quarter-turn `F` and therefore is not the edge-oriented subgroup it claims to
be. Also, reaching `G1` requires both edge orientation and the four E-slice
edges in the E slice, so "edge flip" alone is not a sufficient phase-1
coordinate.
**Decision.** Use the published chain: `G1 = <U,D,L,R,F2,B2>`,
`G2 = <U,D,L2,R2,F2,B2>`, `G3 = <U2,D2,L2,R2,F2,B2>`.
- Phase 1: `(flip, slice)` pair table, 2048 × 495, all 18 moves, ≤7.
- Phase 2: `twist` table, `G1` moves, ≤10.
- Phase 3: quotient of `G2` by `G3` (corner tetrad pattern × U/D-edge orbit
  pattern), `G2` moves, ≤13.
- Phase 4: IDA* within `G3` using corner and edge permutation tables (40320
  each), ≤15.
Subgroup membership is asserted after every phase and every solution is
replayed on a copy of the input cube.
**Consequence.** `concept.md` §9.1 carries an amendment note. Total worst case
stays ≤45 moves, tables stay small enough for WASM. The phase table in this
entry is superseded by D021 (phase 1 is `flip` only); the group chain and the
replay/assert rules stand.

## D020 — Phase 4's edge coordinate includes the slice permutation

**Context.** The phase-4 goal is the identity, not merely `ep8 == 0`. Within
`G3` the four slice edges are freely permuted, so an edge coordinate of 40320
(8!) would accept states with the slice edges out of place and let the solver
return an unsolved cube.
**Decision.** Use `ep_full = ep8_rank * 24 + sp_rank` (967,680 states), with
transitions composed from the existing `move_ep8`/`move_sp` tables. Phase 4 is
IDA* with `h = max(cp_dist, ep_full_dist)`.
**Consequence.** One extra 1 MB distance table; replay verification would have
caught the bug, but the contract now cannot express it.

## D021 — Thistlethwaite phase split corrected again: flip, then (twist, slice)

**Context.** D019 said phase 1 should target `(flip, slice)` and phase 2 `twist`.
Measured diameters disproved the premise: quarter-turn `L`/`R` move U/D edges
into the E slice, so `G1 = <U,D,L,R,F2,B2>` preserves edge orientation but not
the slice. Including `slice` in phase 1 therefore targets a smaller subgroup
(diameter 9), while `twist` alone in phase 2 does not reach `G2` because the
slice may still be dirty.
**Decision.** Phase 1 is `flip` only, all 18 moves, diameter 7. Phase 2 is the
`(twist, slice)` pair, 2187 × 495 = 1,082,565 entries, `G1` moves, diameter 10.
Phase 3 quotient, 4,900 states, diameter 9 (keep ≤13 as the contract bound).
Phase 4 `G3`, 663,552 reachable states, diameter 15, IDA* with
`cp`/`ep_full` heuristics. Worst-case sum 41, still ≤45.
**Consequence.** `concept.md` §9.1 and this log's D019 phase table are corrected.
The total table budget is essentially unchanged (+67 KB). G3's reachable size is
663,552, not 3.98M.

## D022 — Phase 3 uses the exact (cp, edge-orbit) coordinate, not the 4,900 pattern pair

**Context.** D021 assumed the pair `(corner tetrad pattern, U/D edge orbit
pattern)` was a congruence of `G2` by `G3` and would need only 4,900 rows.
Measured transitions showed it is not: cp states 0 and 414 share a pattern but
`U` sends them to different patterns, so a 4,900-row table can reach the goal
pattern while the cube is not in `G3`. Phase 4 then failed on seeds 0, 3, 4, 42.
**Decision.** Use `(cp_rank, ud_edge_orbit_pattern)` — `40320 * 70 = 2,822,400`
rows — with a multi-source BFS whose goal set is the `G3` corner subgroup (96
elements) plus the correct edge pattern. Measured diameter 13, exactly the
contract bound. Keep the explicit `is_g3` assertion before phase 4.
**Consequence.** The distance bundle grows by ~2.8 MB to 4.92 MB (total ~8.0 MB
with move tables), still within the ≤10 MB budget. The "4,900 / max 9" numbers
in D021 were wrong and should not be used.

## D023 — Thistlethwaite performance is accepted as-is

**Context.** 1,000 uniform cubes: median 0.118 ms, max 39.3 ms (seed 833, 7.18M
phase-4 IDA* nodes). The concept's performance note says "<20 ms typical".
**Decision.** Accept: the median is two orders of magnitude under the target, and
the worst case is still below interactive latency for a demo. Do not add the
663,552-byte full-`G3` distance table in v1.
**Consequence.** Worst-case solve time is unbounded by wall clock inside phase 4;
`time_ms` is only checked between phases. Revisit if the demo shows jank or if a
strict worst-case bound is required later.

## D024 — Kociemba phase 1 uses split heuristics, not the 139 MB table

**Context.** `concept.md` §6 describes a full phase-1 pruning table of
`2187 × 2048 × 495 = 2.2B` entries, symmetry-reduced to ~138M (~139 MB), and
explicitly permits the split/heuristic approach when the budget is a problem.
The browser ships Thistlethwaite (D004), so Kociemba is native/CLI-only.
**Decision.** For M3, build two phase-1 heuristics — `(twist, slice)` and
`(flip, slice)`, ~1.08M and ~1.01M bytes — and use `h = max(h1, h2)` in the
phase-1 IDA*. Phase-2 tables stay `cp × sp` and `ep8 × sp` (967,680 each).
Solution lengths are unaffected: each phase is still searched to optimality
within its bound; only pruning strength and search time change.
**Consequence.** Table generation stays seconds, not minutes, and CI needs no
multi-hundred-megabyte cache. If search time ever becomes the bottleneck, the
symmetry-reduced table can be added later behind a feature flag.

## D025 — Add a third phase-1 pair table after the split heuristic missed the target

**Context.** With `h = max(dist(twist,slice), dist(flip,slice))`, 1,000 uniform
cubes took 473 ms median / 1.9 s max and 21.8B phase-1 nodes. `concept.md` §12
targets <50 ms typical, so D024's two-table split is too weak in practice.
**Decision.** Add `dist(twist, flip)` (2187 × 2048 = 4,478,976 bytes, ~4.5 MB)
and use `h = max` of all three phase-1 tables. This is still ~30× smaller than
the 138M-entry symmetry table and keeps the same solution lengths.
**Consequence.** Kociemba's bundle grows to ~8.5 MB (total tables ~16.5 MB,
native/CLI only). Measured after the change: median 245 ms, p95 496 ms, max
854 ms, phase-1 nodes 8.1B — still above the 50 ms target. Accepted as a
documented miss: §12 benchmarks are non-gating, and Kociemba is CLI-only (D004).

## D026 — WASM smoke is Node, not a browser

**Context.** `concept.md` §13 allows either Playwright or a Node `wasm-bindgen`
smoke. Chromium is a large image layer and D005 already cuts the showcase
recording.
**Decision.** Build the crate twice (`--target web` for the site, `--target
nodejs` for tests) and run `scripts/smoke.mjs` via `createRequire`. No
Playwright, no root `package.json`, `.mjs` only.
**Consequence.** Fast CI, small image. A real browser test can be added at M4b
if the UI needs one; the DOM is plain JS and stateless.

## D027 — Tables are embedded in the wasm; budget is transferred size

**Context.** The browser needs the move bundle (3.07 MB) and Thistlethwaite
distances (4.92 MB) at startup.
**Decision.** `include_bytes!` both bundles, generated before compilation by
`gen-tables`, with a `build.rs` that fails with "run ./scripts/gen-tables.sh"
when they are absent. Accept ~8.5 MB raw wasm / ~2.7 MB gzipped; no runtime
fetch and no binaryen requirement in v1. Kociemba tables never enter the wasm.
**Consequence.** Build order is part of the contract: fmt → generate → clippy →
build. CI caches the table bundle by generator source hash.

## D028 — WASM-facing API conventions

**Context.** `wasm32-unknown-unknown` does not reliably implement
`std::time::Instant`, and JS numbers do not carry `u64`.
**Decision.** Put a `cfg(target_arch = "wasm32")` clock shim in the solver
(native unchanged; wasm treats time limits as disabled and never calls
`Instant::now`). Measure `elapsed_ms` in the wasm crate with `js_sys::Date::now`.
`scramble` takes `u32` and zero-extends to `u64`. Fallible exports return
`Result<T, JsValue>`; an already-solved cube yields an empty successful report,
not an error.
**Consequence.** No panic crosses the boundary, and the JS API stays
BigInt-free. The `u32` seed space is a documented stable subset.

## D029 — The optimal oracle is partial: superflip is not proved

**Context.** M6's acceptance requires superflip = 20. The oracle was built with
`max(corner PDB 88M, edgeA PDB 42.6M, edgeB PDB 42.6M)`.
`h(superflip) = 8` because its corners are already solved, and a 5-minute run
exhausted depth 15 (3.64B nodes) and timed out inside bound 16. The remaining
bounds are orders of magnitude larger.
**Decision.** Ship the oracle feature-gated and document it as partial. Do not
sum the edge tables (not admissible: a face turn moves pieces in both groups),
do not hard-code 20, and do not claim optimality after a timeout. M6 stays
"partial" rather than "done".
**Consequence.** `optimal --moves` is a useful exact prover for shallow
positions and a sanity check against Kociemba, but it cannot verify hard
positions yet. Closing M6 requires stronger disjoint/additive pattern databases
or symmetry reduction, which is a separate project.










