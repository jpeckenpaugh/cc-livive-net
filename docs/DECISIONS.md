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
