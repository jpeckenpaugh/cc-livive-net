# Solvers

Two deterministic solvers share the core state machine and the generated move
tables. Both replay every returned solution on a copy of the input before
handing it back; a failed replay is an error, not a warning.

Numbers below are measured, not targets. They come from the `bench` command on
a Docker Desktop arm64 VM on an Apple M4 host, Docker 29.8.2, 2026-10-24,
`--release`, 10 discarded warmups, `n = 100`, tables loaded once. They are
**indicative** and not comparable across machines; the M2/M3 gate runs that used
1,000 cubes are quoted separately where useful.

## Thistlethwaite

Group chain (`G0 = <U,R,F,D,L,B>`):

| Phase | Moves | Coordinate | Table rows | Measured diameter |
|---|---|---|---:|---:|
| 1 | all 18 | `flip` | 2,048 | 7 |
| 2 | `G1` (14) | `(twist, slice)` | 1,082,565 | 10 |
| 3 | `G2` (10) | `cp × U/D-edge orbit` | 2,822,400 | 13 |
| 4 | `G3` (6) | `cp` (IDA*) and `ep_full = ep8 × 24 + sp` | 40,320 / 967,680 | 15 |

`concept.md` §9.1 originally stated a different `G1` and a
slice-in-phase-1 split. Measurements corrected both: `G1 = <U,D,L,R,F2,B2>` does
not preserve the E slice, so phase 1 is `flip` only and phase 2 is the
`(twist, slice)` pair (D019, D021). Phase 3's assumed 4,900-state quotient was
not a congruence of `G2`; the exact `(cp_rank, edge-orbit)` coordinate is used
instead (D022). The contract bound remains 45 moves; the measured worst case is
41.

Fresh default `bench` run (`--gen` off):

```text
thistlethwaite: n=100 warmup=10
  time     min=0.010 ms median=0.249 ms p95=2.416 ms max=10.512 ms mean=0.752 ms
  moves    min=22 median=32 p95=35 max=37 mean=31.57
  phases   mean=4.67 7.95 8.91 10.04
```

The M2 gate over 1,000 uniform cubes measured median `0.118 ms`, worst
`39.3 ms` (seed 833, 7.18M phase-4 IDA* nodes), and worst length 41. Phase 4's
worst-case search is not wall-clock bounded inside a phase; `time_ms` is checked
between phases (D023).

## Kociemba

Two-phase IDA*. Phase 1 searches `(twist, flip, slice)` with all 18 moves and
`h = max(dist(twist,slice), dist(flip,slice), dist(twist,flip))`, bounded at 12.
Every `G1` node is offered to phase 2, which searches `(cp, ep8, sp)` with the
ten `G1` moves and `h = max(dist(cp,sp), dist(ep8,sp))`, bounded at 18. The
shortest total wins.

Fresh default `bench` run (`--gen` off):

```text
kociemba: n=100 warmup=10
  time     min=157.273 ms median=302.288 ms p95=510.452 ms max=710.050 ms mean=321.910 ms
  moves    min=18 median=20 p95=21 max=22 mean=19.85
  phase1   min=8 median=11 p95=12 max=12 mean=11.13
  phase2   min=6 median=9 p95=11 max=13 mean=8.72
  search   phase1_nodes_mean=8272863 phase2_calls_mean=1396.30 phase2_nodes_mean=711778
```

The M3 gate over 1,000 cubes measured 16–22 moves (mean 19.86) and median
`245 ms`, p95 `496 ms`, max `854 ms`. The `concept.md` §12 target is `<50 ms`
typical with warm tables; this is a **known miss**, accepted and recorded in
D025. Kociemba is native/CLI only: its tables are not in the browser bundle.

## Apply throughput

The `bench` apply phase walks a deterministic 1,000,000-move sequence over one
cube in release mode. Fresh run: 27.036 ms, or **36,987,850 moves/s**. The
`concept.md` §12 non-gating floor is ≥10M moves/s.

## Tables

| Bundle | File | Bytes |
|---|---|---:|
| Move tables | `cc-tables-v1.bin` | 3,073,864 |
| Thistlethwaite distances | `cc-thistlethwaite-v1.bin` | 4,915,112 |
| Kociemba distances | `cc-kociemba-v1.bin` | 8,510,760 |

Thistlethwaite payloads: phase 1 `2,048`, phase 2 `1,082,565`, phase 3
`2,822,400`, phase 4 corner `40,320`, phase 4 edge `967,680` bytes.
Kociemba payloads: `1,082,565`, `1,013,760`, `967,680`, `967,680`, `4,478,976`
bytes. The move bundle's six tables are `78,732`, `73,728`, `17,820`,
`1,451,520`, `1,451,520`, `432` bytes.

Fresh generation timings (same container, `bench --gen`):

```text
move tables:          twist 3.756 ms, flip 2.030 ms, slice 0.752 ms,
                      cp 84.285 ms, ep8 77.749 ms, sp 0.014 ms
thistlethwaite:       phase1_flip 0.217 ms, phase2_twist_slice 266.123 ms,
                      phase3_quotient 413.828 ms, phase4_cp 0.085 ms,
                      phase4_ep_full 2.158 ms
kociemba:             phase1_twist_slice 185.514 ms, phase1_flip_slice 140.806 ms,
                      phase2_cp_sp 141.469 ms, phase2_ep8_sp 153.504 ms,
                      phase1_twist_flip 1420.809 ms
```

Generation is deterministic and versioned: each bundle carries a magic number,
format version, parameter hash, and payload checksum. Generate with
`./scripts/gen-tables.sh`. The output directory is `$COLOR_CUBE_TABLES` or
`./tables`; files are gitignored. CI caches `tables/` under a key derived from
the solver/core sources and `Cargo.lock`. `./scripts/gen-tables.sh --verify`
also exhaustively checks the `G3` space (663,552 reachable states, diameter 15).

Table generation must precede compilation because the WASM crate embeds both
bundles with `include_bytes!` (D027). `scripts/check.sh` generates them when
missing. The browser bundle contains the move tables and Thistlethwaite
distances only: 8,088,272 bytes raw / 2,762,690 bytes gzip (7.71 MiB / 2.63 MiB)
as measured by `scripts/smoke.mjs` in the M5 gate.

## Replay guarantee

Both solvers apply their move list to a copy of the input and return
`SolveError::NoSolutionWithinLimits` if the result is not solved. The CLI and
the WASM smoke test replay independently, so a solution shown to a user is
always a solved cube.
