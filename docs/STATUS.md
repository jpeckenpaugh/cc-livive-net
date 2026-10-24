# Status ledger

Updated by the PM at every commit. `Done` means the milestone gate passed in
Docker and was committed to `main`.

| Milestone | Scope | State | Commit | Notes |
|---|---|---|---|---|
| M0 | Workspace skeleton, core state machine, 18 moves, invariants | Done | `feat(core)` | Zero deps, `no_std`, Kociemba cross-check green |
| M1 | Seeded scrambles, notation, observations, action mask | Not started | — | |
| M2 | Coordinates, move tables, Thistlethwaite | Not started | — | |
| M3 | Kociemba two-phase, table manifest | Not started | — | |
| M4 | WASM wrapper, browser demo, Pages | Not started | — | Browser default: Thistlethwaite (D004) |
| M5 | README, RL docs, benchmarks, optional PyO3 | Not started | — | |
| M6 | Optional optimal oracle | Not started | — | |

## Gate log

_(append-only, newest first)_

| Date | Gate | Result |
|---|---|---|
| 2026-10-24 | M0 check | pass: fmt, clippy, `no_std` build, 19 tests incl. 10k round-trips |
| 2026-10-24 | scaffolding | docs + Docker wrapper added |
