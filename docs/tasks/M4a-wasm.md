# M4a — WASM wrapper, tooling, CI, Pages

**Read first:** `concept.md` §10, §11, §13; `docs/DECISIONS.md` (D002, D004,
D005, D023, D025); the existing crates and `scripts/docker.sh`.

**Mode:** question-first. Return questions before writing files. No git. Use
`./scripts/docker.sh`.

## Goal

A typed `wasm-bindgen` module over the Rust core + Thistlethwaite, a Node smoke
test that exercises it, and the build/CI/Pages plumbing. The browser ships
**Thistlethwaite only** (D004); Kociemba stays native.

## Decisions already made

- **D026: Node smoke, no Playwright.** `concept.md` §13 explicitly allows a Node
  `wasm-bindgen` smoke instead of a browser test. No Chromium in the image for
  v1. Drop the browser layer from D003.
- **Tables are embedded** with `include_bytes!` from `tables/`, generated before
  the wasm build by `gen-tables`. No runtime fetch in v1. Document the build
  order and measure the raw and gzipped `.wasm` size.
- **The site is stateless**: no mutable cube lives in JS. Every call takes
  facelets or moves and returns a new result. No second cube engine in JS.

## Exported API (`crates/color-cube-wasm`)

```rust
scramble(seed: u32) -> String              // 25-move walk, Singmaster
solve(facelets: &[u8]) -> String           // Thistlethwaite moves, "" if solved
solve_report(facelets: &[u8]) -> SolveReport
facelets_after(moves: &str) -> Vec<u8>     // 54 colour indices
observe(facelets: &[u8]) -> Vec<f32>       // 324 one-hot floats
action_mask() -> Vec<f32>                  // 18 ones
canonical_mask(last: Option<u8>) -> Vec<f32>
check() -> ConformanceReport
solved_facelets() -> Vec<u8>
move_names() -> Vec<String>                // the 18 labels in action order
```

`SolveReport` getters: `moves`, `phase1`, `phase2`, `elapsed_ms`, `solver`.
`ConformanceReport` getters: `passed`, `failed`, `details` (multi-line text).

- Invalid facelets return an error/empty result with a readable message; do not
  panic across the boundary. Use `Result<_, JsValue>` where it helps.
- Colour indices are the core `Color::index()` values, `0..6`.
- `solve` must never panic on a malformed array; validate length first.

## WASM clock caveat (important)

`color-cube-solver` uses `std::time::Instant` for limits. On
`wasm32-unknown-unknown` this is not reliably implemented. Gate it with
`#[cfg(target_arch = "wasm32")]`: treat time limits as disabled (no panic, no
op), and set `elapsed_ms` from a `js_sys::Date::now()` measurement in the wasm
crate (or report 0 and let the JS layer time the call — pick one and document).
Core logic must not change for native.

## Build outputs

- `web/pkg/` — `wasm-pack build crates/color-cube-wasm --target web`.
- `web/pkg-node/` — the same crate built `--target nodejs` for the smoke test.
- Both are gitignored or built in CI; do not commit generated wasm.
- Add `web/CNAME` containing `cc.livive.net`.

## Tooling

- `scripts/check.sh`: build image if needed, generate tables if missing
  (`./scripts/docker.sh cargo run --release -p color-cube-solver --bin gen-tables`),
  run `fmt`, `clippy`, `test`, the `no_std` build, build both wasm targets, then
  the Node smoke. Non-zero exit on any failure, readable section headers.
- `scripts/smoke.mjs`: import `web/pkg-node`, scramble a known seed, solve,
  apply the solution via `facelets_after`, assert solved, assert `observe`
  length 324 and one-hot sums, and print a one-line PASS summary.
- `package.json` at repo root only if needed for `"type": "module"`; otherwise
  use `.mjs`. No npm dependencies.

## CI / Pages

- `.github/workflows/ci.yml`: `rust` job (image build cached, fmt/clippy/test,
  table cache keyed on `crates/color-cube-solver/src` + bundle version) and a
  `wasm` job (build, Node smoke, upload artifact). Independent, parallel.
- `.github/workflows/pages.yml`: build tables and wasm, copy `web/`, deploy on
  `main` with `actions/deploy-pages`. Include `CNAME`.
- Do not commit table binaries; cache them.

## Tests

- Node smoke covers: scramble → facelets → solve → replay → solved; malformed
  facelets rejected; `check()` reports 0 failures; observation shape and
  one-hot sums.
- Rust-side unit test for the wrapper's validation (length, unknown colours).
- `scripts/check.sh` must pass end-to-end in a clean volume (tables absent).

## Reporting

Files, commands, raw/gzipped wasm sizes, smoke output, CI YAML summary,
interpretations, unresolved items. Under 800 words.
