# Status ledger

Updated by the PM at every commit. `Done` means the milestone gate passed in
Docker and was committed to `main`.

| Milestone | Scope | State | Commit | Notes |
|---|---|---|---|---|
| M0 | Workspace skeleton, core state machine, 18 moves, invariants | Done | `feat(core)` | Zero deps, `no_std`, Kociemba cross-check green |
| M1 | Seeded scrambles, notation, observations, action mask | Done | `feat(core)` | Frozen 36-draw contract, 32 golden hashes |
| M2 | Coordinates, move tables, Thistlethwaite | Done | `feat(solver)` | Max 7/10/13/15, worst 41 moves, median 0.12 ms |
| M3 | Kociemba two-phase, table manifest | Done | `feat(solver)` | 16–22 moves (mean 19.86), ≤30; median 245 ms, documented miss vs 50 ms |
| M4 | WASM wrapper, browser demo, Pages | Done | `feat(wasm)` | Isometric canvas, Playwright smoke, Pages workflow |
| M5 | README, RL/solver docs, licenses, benchmarks | Done | `docs(m5)` | Bench, dual license, `cargo doc` clean |
| M6 | Optional optimal oracle | Partial | `feat(optimal)` | Feature-gated IDA* + 3 PDBs; proves shallow distances, superflip not proved in 5 min |
| A | Deterministic build, provenance, repro check | Review | — | Manifest-list pins, offline wasm-pack cache, `/work` vs `/alt` PASS, `provenance.json` panel; D030–D032 |

## v1 definition of done

From `concept.md` §18, with evidence:

- [x] A fresh clone can run `./scripts/check.sh` and pass — M0–M5 gate logs below.
- [x] The CLI solves any uniform random cube with ≤30 moves, replay-verified and
      deterministic — M3 gate (1,000 cubes, 16–22 moves, mean 19.86).
- [x] `cc.livive.net` scrambles and solves in the browser without a JavaScript
      cube implementation — M4b browser gate (15 assertions); `web/app.js` holds
      only facelets and calls the wasm exports. **Live:** HTTP 200 on
      `bbcc848`-era build, deployed `.wasm` SHA-256 matches the local build.
- [x] Documentation, licensing, and benchmarks let an external user start —
      `README.md`, `docs/RL.md`, `docs/SOLVERS.md`, `LICENSE-MIT`,
      `LICENSE-APACHE`.

## Gate log

_(append-only, newest first)_

| Date | Gate | Result |
|---|---|---|
| 2026-10-24 | A provenance | pass: image pinned (index digests, amd64+arm64), offline wasm-pack cache proved with `--network none`, wasm `dd159465…`; `repro-check.sh --allow-dirty` PASS `/work` vs `/alt` on arm64 and amd64 (same hash); clean run pending commit |
| 2026-10-24 | B check | pass: 94 Node assertions, 15 browser assertions, structure drift guard, wasm 8,103,757 B |
| 2026-10-24 | A repro | pass: clean HEAD, arm64, wasm/js/gzip/tables all MATCH |
| 2026-10-24 | CI + deploy | pass: rust, wasm, browser jobs green on `bbcc848`; Pages live, deployed wasm hash matches local |
| 2026-10-24 | M5 check | pass: `check.sh` (94 Node assertions), `browser-smoke.sh` (15), `cargo doc` clean, bench n=100 |
| 2026-10-24 | M4b browser | pass: 10 Playwright assertions, screenshot at artifacts/browser-pass.png |
| 2026-10-24 | M4a check | pass: scripts/check.sh end-to-end, 35 Node assertions, wasm 7.71 MiB raw / 2.63 MiB gzip |
| 2026-10-24 | M3 check | pass: 1000 cubes 16–22 moves (mean 19.86), phase bounds 12/18, CLI kociemba replays |
| 2026-10-24 | M2b check | pass: 1000 uniform cubes replay-verified ≤45 (worst 41), diameters 7/10/13/15, CLI smoke |
| 2026-10-24 | M2a check | pass: exhaustive coordinate round-trips, table agreement/corruption, 3.07 MB bundle in 1.2 s |
| 2026-10-24 | M1 check | pass: 10k seeds, golden hashes, masks, notation |
| 2026-10-24 | M0 check | pass: fmt, clippy, `no_std` build, 19 tests incl. 10k round-trips |
| 2026-10-24 | scaffolding | docs + Docker wrapper added |
