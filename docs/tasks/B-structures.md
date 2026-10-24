# B — Rust data-structures view and repository links

**Read first:** `concept.md` §10; `docs/DECISIONS.md`; `docs/tasks/A-provenance.md`;
`web/index.html`, `web/app.js`, `web/style.css`, `web/renderer.js`,
`web/tests/browser.spec.mjs`; `crates/color-cube-wasm/src/lib.rs`;
`crates/color-cube/src/lib.rs`.

**Mode:** question-first. Return questions before writing files. No git. Use
`./scripts/docker.sh` and `./scripts/browser-smoke.sh`.

## Goal

Make the page explain the actual Rust state, with live values from the current
cube, and link prominently to the source repository. This is an educational
view, not a second implementation: all moves still go through wasm.

## 1. WASM exposition (additive)

Add to `color-cube-wasm`:

```rust
cube_view(facelets: &[u8]) -> Result<CubeView, JsValue>
```

`CubeView` getters:

- `cp() -> Vec<u8>`, `co() -> Vec<u8>`, `ep() -> Vec<u8>`, `eo() -> Vec<u8>`
- `twist() -> u16`, `flip() -> u16`, `slice() -> u16`
- `cp_rank() -> u16`, `ep8_rank() -> u16`, `sp_rank() -> u8`
- `is_valid() -> bool`, `is_solved() -> bool`
- `cube_bytes() -> u32` (`core::mem::size_of::<Cube>()`)

Validation must match `solve`: wrong length or out-of-range colour → `Err`,
never a panic. `ep8_rank`/`sp_rank` are only meaningful in `G1`; return
`Option<u16>`/`Option<u8>` (or a documented sentinel) rather than panicking —
choose the JS-friendly form and document it.

Also expose `data_structure_meta() -> String` returning the contents of an
embedded JSON document (see below), so native, Node smoke, and the browser all
read the same metadata.

## 2. Structure metadata with a drift guard

- `web/structures.json` (served and embedded by the wasm crate) with entries:
  `id`, `name`, `kind`, `rust` (the declaration), `fields` (name/type/meaning),
  `invariants`, `bytes`, `source` (`file` + `symbol`).
- `scripts/check-structures.mjs`: for every entry, assert the `rust` snippet is
  a verbatim substring of the named source file, and that `bytes` matches a
  documented `size_of` (hard-code expected sizes in the check, not in the UI).
- Wire the check into `scripts/check.sh` so a refactor that changes a struct
  without updating the page fails CI.
- Cover at least: `Cube`, `Move`, `Color`, `Scramble`, `Solution`, `Limits`,
  `CubeError`, and the six coordinates (`twist`, `flip`, `slice`, `cp_rank`,
  `ep8_rank`, `sp_rank`).

## 3. UI: a second inspector tab

- The inspector panel becomes two tabs: **Live state** (current arrays and
  coordinates) and **Rust types** (structure cards).
- Use proper `role="tablist"`/`tab`/`tabpanel` semantics, arrow-key navigation,
  and visible focus states.
- Live arrays: render `cp/co/ep/eo` as indexed rows with piece numbers; update
  on every move, scramble, reset, and playback step.
- Coordinate row: twist/flip/slice/rank values with their ranges; show `—` for
  `ep8`/`sp` outside `G1` and say why.
- Cards: name, declaration in a `<pre>`, field table, invariants, byte size,
  and a source link to GitHub at the current commit when provenance is
  available (fall back to `main`).
- Add a one-line explainer that the 54 facelets are an output projection and the
  solver state is the cubie arrays.

## 4. Repository links

- Header: a visible **Source** link (opens in a new tab, `rel="noopener"`).
- Footer: repository, README, `docs/RL.md`, `docs/SOLVERS.md`,
  `docs/PROVENANCE.md`, licence links.
- Do not duplicate the provenance panel; link to it.

## 5. Tests

- Node smoke: `cube_view` on a known facelet array returns the expected arrays
  and coordinates; invalid input errors; `data_structure_meta` parses.
- Browser spec: Source link has the right href; switching to **Rust types**
  shows `Cube`; applying a move updates the live `cp` row; tab keyboard
  navigation works; no console errors; 375 px layout still has no horizontal
  overflow.
- `scripts/check.sh` runs the structures drift check.

## Constraints

- No JS implementation of cube state or moves.
- No new runtime dependencies in `web/` (plain DOM, no framework).
- Keep the wasm payload growth small (metadata is a few KB).

## Reporting

Files, wasm size delta, new exports, drift-check output, browser-test output,
screenshots, and anything unresolved. Under 800 words.
