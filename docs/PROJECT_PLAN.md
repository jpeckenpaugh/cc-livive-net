# Project plan

Derived from `concept.md` §14. Milestones are implemented by staged sub-agents;
each milestone is split into briefs under `docs/tasks/`. A milestone is complete
only when its gate passes in Docker and the work is committed to `main`.

| Milestone | Deliverable | Gate |
|---|---|---|
| M0 | Workspace, `color-cube` core: `Cube`, `Move`, `Color`, errors, facelets, 18 derived moves, invariants | `cargo test -p color-cube` incl. 10k property round-trips and move identities |
| M1 | `from_seed`, `from_scramble`, Singmaster parse/format, observation + action mask | 10k seeds valid/unsolved/stable; observation hash fixture |
| M2 | Coordinates + move tables + Thistlethwaite | 1,000 random cubes solved, ≤45 moves, replayed |
| M3 | Kociemba two-phase + table manifest | 1,000 random cubes solved, ≤30 moves, deterministic, replayed |
| M4 | WASM wrapper + static site + Pages | headless browser solves a seeded scramble end-to-end |
| M5 | Docs polish, RL guide, benchmarks | external user can train a toy agent from the README |
| M6 | Optional optimal oracle | superflip proven at distance 20 |

## Delivered vs deferred (M5)

- **Delivered:** root README with a verified quickstart, `docs/RL.md`,
  `docs/SOLVERS.md`, dual MIT/Apache-2.0 licensing with attribution, a `bench`
  CLI subcommand, fresh measured numbers, and the v1 definition-of-done
  checklist in `docs/STATUS.md`.
- **Deferred:** Python/PyO3 bindings (`concept.md` §17.6); the optimal oracle
  (M6); the showcase recording (D005); the symmetry-reduced Kociemba phase-1
  table (D024/D025).

## Sequencing notes

- M2 and M3 share the coordinate/table layer; build it once, in M2, with a
  versioned manifest so M3 can extend it.
- M4 depends on M2 (Thistlethwaite in WASM) but not on M3.
- The `web/` tree is the only area where a parallel implementer could safely
  work beside the solvers; use a worktree if we do that.
- Table generation is expensive and lives in a Docker volume, never in git.
  `gen-tables` must be idempotent and cache by a source hash.

## Risk watchlist

Tracked from `concept.md` §15, with the checks we will actually run:

| Risk | Early warning | Check |
|---|---|---|
| Orientation convention wrong | Thistlethwaite phase assertions fail | facelet-derived moves + `concept.md` §12 cross-checks |
| Parity bug in `from_seed` | statistical drift in fixtures | explicit parity fix + 10k seed audit |
| Table size / WASM budget | image or payload grows sharply | report sizes in the table manifest and CI artifact |
| Nondeterminism | golden hash mismatch | fixed move order, no hash iteration, opt-in time limits |
| `no_std` erosion | core builds only with std | CI builds core with `--no-default-features` |
