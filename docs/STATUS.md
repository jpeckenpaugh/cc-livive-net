# Status ledger

Updated by the PM at every commit. `Done` means the milestone gate passed in
Docker and was committed to `main`.

| Milestone | Scope | State | Commit | Notes |
|---|---|---|---|---|
| M0 | Workspace skeleton, core state machine, 18 moves, invariants | Done | `feat(core)` | Zero deps, `no_std`, Kociemba cross-check green |
| M1 | Seeded scrambles, notation, observations, action mask | Done | `feat(core)` | Frozen 36-draw contract, 32 golden hashes |
| M2 | Coordinates, move tables, Thistlethwaite | Done | `feat(solver)` | Max 7/10/13/15, worst 41 moves, median 0.12 ms |
| M3 | Kociemba two-phase, table manifest | Not started | — | |
| M4 | WASM wrapper, browser demo, Pages | Not started | — | Browser default: Thistlethwaite (D004) |
| M5 | README, RL docs, benchmarks, optional PyO3 | Not started | — | |
| M6 | Optional optimal oracle | Not started | — | |

## Gate log

_(append-only, newest first)_

| Date | Gate | Result |
|---|---|---|
| 2026-10-24 | M2b check | pass: 1000 uniform cubes replay-verified ≤45 (worst 41), diameters 7/10/13/15, CLI smoke |
| 2026-10-24 | M2a check | pass: exhaustive coordinate round-trips, table agreement/corruption, 3.07 MB bundle in 1.2 s |
| 2026-10-24 | M1 check | pass: 10k seeds, golden hashes, masks, notation |
| 2026-10-24 | M0 check | pass: fmt, clippy, `no_std` build, 19 tests incl. 10k round-trips |
| 2026-10-24 | scaffolding | docs + Docker wrapper added |
