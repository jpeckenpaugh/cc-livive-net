# M3 — Kociemba two-phase solver

**Read first:** `concept.md` §6, §9.2, §9.5, §12 (solver tests), §13;
`docs/DECISIONS.md` (especially D014 pruning, D024 split heuristics, D018
phase-2 move set); the M2a/M2b code.

**Mode:** question-first. Return questions before writing files. No git. Use
`./scripts/docker.sh`.

## Goal

A deterministic two-phase Kociemba solver: typical ~19–21 moves, ≤30 guaranteed
within limits, native/CLI only (the browser keeps Thistlethwaite, D004).

## Phases

`G1 = <U,D,R2,L2,F2,B2>` (same subgroup Thistlethwaite calls `G1`).

- **Phase 1**: IDA* over `(twist, flip, slice)` with all 18 moves, bound ≤12.
  Goal `twist == 0 && flip == 0 && slice == SLICE_SOLVED`.
- **Phase 2**: from a `G1` node, IDA* over `(cp, ep8, sp)` using only
  `PHASE2_MOVES` (`U,U2,U',D,D2,D',R2,L2,F2,B2`), bound ≤18.

Driver:

1. Iterative deepening on phase-1 depth from the lower bound to 12.
2. At every node with phase-1 `h == 0`, run phase 2 with limit
   `best_len - depth - 1` (cap 18).
3. Keep the shortest total; stop when phase-1 depth + 1 cannot beat it.
4. Deterministic: moves ascending, fixed tie-breaks, no hash iteration.
5. `Limits { max_length, time_ms }` respected; on timeout return the best found
   if it is shorter than `max_length`, else `Timeout`.

## Heuristics (D024)

- `h1 = dist(twist, slice)` — 2187 × 495 = 1,082,565 entries
- `h2 = dist(flip, slice)` — 2048 × 495 = 1,013,760 entries
- phase-1 `h = max(h1, h2)`
- `dist(cp, sp)` and `dist(ep8, sp)` — 967,680 entries each
- phase-2 `h = max(d1, d2)`

All are `u8` distance tables with `0xFF = unvisited`, computed by backward BFS
from the solved coordinate. Do not materialise a 1M × 18 pair-move table;
compose transitions from the existing `move_*` tables at lookup time.

## Tables

Third bundle `tables/cc-kociemba-v1.bin`, magic `b"CCKM"`, version 1, reusing
the factored header/checksum/atomic-write code:

- id 1 `phase1_twist_slice` (1,082,565 B)
- id 2 `phase1_flip_slice` (1,013,760 B)
- id 3 `phase2_cp_sp` (967,680 B)
- id 4 `phase2_ep8_sp` (967,680 B)

`gen-tables` writes it too and prints timings/sizes. Generation must stay in
single-digit seconds in release; report debug too.

## API

`Kociemba` implements `Solver` (concept §9.5), `Send + Sync`, tables via
`OnceLock`, `new()` loads only, `generate(dir)` separate. `Solution.phases =
Some([p1_len, p2_len])`. Replay-verify every solution on a copy of the input
before returning. `AlreadySolved`, `InvalidState`, `TablesMissing`,
`Timeout`, `NoSolutionWithinLimits` behave as in M2b.

Add `--solver thistlethwaite|kociemba` to the CLI (default thistlethwaite);
`kociemba` prints both phase lengths and total.

## Tests

1. 1,000 uniform `from_seed` cubes: solved, replayed, ≤30 total, phase 1 ≤12,
   phase 2 ≤18. Report the length histogram (want 19–21 typical).
2. Determinism: same cube → same list; aggregate hash stable across two runs.
3. Golden: 16 `(seed -> solution)` fixtures, regression only.
4. Limits: `max_length` below the solution length → `NoSolutionWithinLimits`;
   `time_ms = 0` → `Timeout`; a tiny nonzero limit returns best-found or
   `Timeout` without panicking or returning an invalid list.
5. Table corruption/missing → `TablesMissing`.
6. Pruning completeness: solving with canonical pruning enabled yields the same
   total length distribution as with it disabled on a 100-cube sample (guards
   against over-pruning that silently worsens length).
7. CLI: `solve --seed 42 --solver kociemba` replays.

## Out of scope

Symmetry-reduced phase-1 tables, optimality claims, WASM, browser. Do not change
M0–M2 public APIs except additions.

## Reporting

Files, commands with output, table sizes/times, length histogram, solve-time
distribution, phase-1/phase-2 node counts, interpretations, unresolved items.
Under 900 words. If ≤30 cannot be met within the bounds, stop and report.
