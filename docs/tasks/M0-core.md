# M0 — Workspace skeleton and `color-cube` core

**Read first:** `concept.md` (whole file, especially §3, §4, §5, §12) and
`docs/DECISIONS.md`.

**Mode:** question-first. Read the brief and repo, then return your open
questions **before writing or modifying any file**. Do not run git.

## Goal

A greenfield Cargo workspace whose `color-cube` crate is a correct, `no_std`,
zero-dependency cube state machine: solved state, facelets, cubie conversion,
all 18 moves, and full validity checking — with the test suite from
`concept.md` §12 for the core.

## Deliverables

1. Workspace `Cargo.toml` at the repo root with members
   `crates/color-cube`, `crates/color-cube-solver`, `crates/color-cube-cli`,
   `crates/color-cube-wasm`, `resolver = "2"`, and the workspace lint tables from
   `concept.md` §11. Only `color-cube` is implemented this milestone; the other
   three are compile-only placeholders (a `lib.rs`/`main.rs` with a doc comment
   and nothing else, no dependencies).
2. `crates/color-cube`:
   - `#![no_std]`, `#![forbid(unsafe_code)]`, no dependencies of any kind
     (including dev-dependencies — see D008).
   - `Cube { cp, co, ep, eo }`, `Cube::SOLVED`.
   - `Color`, `Move(pub u8)` with `ALL`, `from_index`, `inverse`,
     `quarter_turns`, `face`, `power`.
   - `Cube::apply`, `apply_all`, `apply_inverse`, `is_solved`, `is_valid`.
   - `Cube::facelets() -> [Color; 54]` and
     `Cube::from_facelets(&[Color; 54]) -> Result<Self, CubeError>`.
   - `CubeError` (non-exhaustive) + `core::fmt::Display`, no `std`.
   - Move transitions derived from facelet geometry per `concept.md` §5.

## Explicitly out of scope

- `from_seed`, `from_scramble`, `Scramble`, Singmaster formatting/parsing,
  observations, action masks, coordinates, tables, solvers, WASM, CLI
  behaviour. Those are M1+.
- Any allocation, any dependency, any `std` usage in the core.

## Fixed conventions (do not re-litigate)

Facelet order `U R F D L B`, row-major from outside; cubie order and
orientation definitions per `concept.md` §3.3/§3.4; move indices per §3.5.
`from_facelets` must verify centers, permutation completeness, twist sum, flip
sum, and corner/edge parity, returning the matching `CubeError` variant.

## Design constraints

- **Derive, don't transcribe.** Build the six 90° turns as facelet
  permutations, compose for `2` and `'`, then convert to cubie transitions.
  If you use Kociemba's published tables at all, they belong in `tests/` as a
  cross-check, cited in a comment — never in the library path.
- `Move` is `#[repr(transparent)]` and cheap to map from action index `0..18`.
- No floating point in state logic.
- Keep `Cube` `Copy` and small; no interior mutability.
- Every public item gets a doc comment. Missing docs are a code smell, not a
  lint failure yet.

## Test gates (all must run green in Docker)

Implement as unit tests in the crate (or `tests/` inside the crate) using a
hand-written SplitMix64 for pseudo-random cases:

1. 10,000 facelet ↔ cubie round-trips over random valid states (construct random
   valid cubes directly from cubies; `facelets` → `from_facelets` → compare).
   This needs a valid-state generator that does not depend on M1's `from_seed` —
   a small local helper in tests is fine.
2. All 18 moves preserve all four invariants.
3. `X^4 = identity` and `X · X' = identity` for all 18 moves.
4. `(R U R' U')^6 = identity` and `(U R U' R')^6 = identity`.
5. Opposite faces commute (`U/D`, `R/L`, `F/B`).
6. `Cube::SOLVED.is_solved()` and `is_valid()`; solved facelets are exactly the
   six monochrome faces in `U R F D L B` order.
7. `from_facelets` rejects, with the correct variant, one case each for:
   wrong center, duplicate piece, missing piece, twist violation, flip
   violation, parity violation.
8. A move applied to a cube produces the expected permutation on a small,
   hand-checked case (e.g. `U` maps `URF` corner piece from `URF` to `UBR`), so a
   systematically wrong derivation cannot pass the identity tests alone.

## Acceptance

- `./scripts/docker.sh check` (fmt, clippy `-D warnings`, `cargo test --all`,
  `cargo build -p color-cube --no-default-features`) passes.
- No `unsafe`, no dependencies, no `std` in the core.
- The derivation method is documented in a module-level comment, including how
  the 90° facelet permutations were obtained from the net in §3.2.

## Reporting back

Return: (a) files added, (b) the commands you ran and their output, (c) any
convention you had to interpret, (d) anything you could not do. Do not commit.
