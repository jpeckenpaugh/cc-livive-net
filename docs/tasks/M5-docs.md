# M5 — Documentation, licensing, benchmarks, v1 finish

**Read first:** `concept.md` §1, §8, §11, §12 (performance), §16, §17;
`docs/DECISIONS.md`; `docs/STATUS.md`; the current code and `README.md`.

**Mode:** question-first. Return questions before writing files. No git. Use
`./scripts/docker.sh`.

## Goal

An external user can clone the repo, run `./scripts/check.sh`, and train a toy
agent or solve a cube from the README alone. Licensing and attribution are
correct.

## Deliverables

### 1. `README.md` (rewrite)

- One-paragraph purpose and the "colour cube, not Rubik's" trademark note.
- Status table per milestone, with measured numbers (not aspirational ones).
- Quick start:
  - `cargo add color-cube` style usage with a 15-line example: seeded state,
    `apply`, `facelets`, `write_observation`, `solve` via the solver crate.
  - Docker build/test commands, including `./scripts/check.sh` and the browser
    smoke.
  - CLI examples for `solve`, `scramble`, `check`, `bench`.
- API contract for RL, linking `docs/RL.md`: action order, observation layout,
  masks, reward constants, curriculum.
- Solver section: what each solver guarantees, measured length/time, table
  sizes, and the known Kociemba performance miss.
- Architecture: four crates, no dependencies in the core, one state machine
  shared by native, CLI, and wasm (no JS engine).
- Build/test table and CI description.
- Attribution and dual license.

### 2. `docs/RL.md`

- 18-action table with indices, inverses, HTM cost.
- Observation: 324 floats, `facelet * 6 + color`, facelet order, one-hot
  invariant. Include a worked solved-cube snippet or checksum.
- Action masks: default all-ones; canonical mask rule (D014) with examples.
- Episode/reward constants from `concept.md` §8; curriculum by move count with
  the caveat that it is not optimal distance.
- Determinism guarantees: `from_seed` contract (D015), golden hashes, why
  `from_scramble` is not uniform.
- A short "first agent" recipe (state → action → reward loop pseudocode).

### 3. `docs/SOLVERS.md`

- Thistlethwaite phases, measured diameters (7/10/13/15), worst/median moves
  and time, table sizes, and the two spec corrections (D019–D022).
- Kociemba phases, measured 16–22 moves (mean 19.86), the 50 ms miss (D025),
  and why the browser ships Thistlethwaite only.
- Table generation and caching instructions.
- Replay-verification guarantee.

### 4. Licensing

- Add `LICENSE-MIT` and `LICENSE-APACHE` (fetch the canonical Apache 2.0 text;
  do not paraphrase). Keep the existing `LICENSE` if it is the MIT text.
- Ensure every crate inherits `license.workspace`; fix the wasm-pack warning
  about a missing license file if there is a clean way (crate-local copies or
  documented suppression).
- Add the Kociemba/Thistlethwaite attribution required by `concept.md` §16.

### 5. Benchmarks

- Add a `bench` subcommand to the CLI (no new dependencies):
  - `apply` throughput over a long move sequence (report moves/s),
  - Thistlethwaite over N seeds: min/median/p95/max and length stats,
  - Kociemba over N seeds: same, plus phase split,
  - table bundle sizes and generation timings.
- Defaults must be quick (e.g. N=100); `--n` to raise it.
- Record the numbers in `docs/SOLVERS.md` and the README, labelled with the
  machine/container context and the date.

### 6. Final pass

- `cargo doc --no-deps` for the workspace builds without warnings that are in
  our control.
- `docs/STATUS.md`: M5 done, M6 optional/not started, and a "v1 definition of
  done" checklist from `concept.md` §18 with each item's evidence.
- `docs/PROJECT_PLAN.md`: mark delivered vs deferred (PyO3, optimal oracle).
- Remove or update stale statements (e.g. "no browser test", old table sizes).

## Explicitly out of scope

Python/PyO3 bindings (deferred, `concept.md` §17.6), optimal oracle (M6),
showcase recording (D005), new solver features.

## Tests / acceptance

- `./scripts/check.sh` and `./scripts/browser-smoke.sh` pass.
- `cargo doc` clean.
- A fresh reader can follow the README quickstart; every command in it is one
  you actually ran in Docker and pasted the output of.
- Numbers in docs match a fresh `bench` run (within noise); no invented
  figures.

## Reporting

Files, commands with output, bench numbers, doc build result, any claim you
could not verify, and anything unresolved. Under 900 words.
