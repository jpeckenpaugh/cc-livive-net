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


