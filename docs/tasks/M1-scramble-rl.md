# M1 — Seeded scrambles, notation, observations, action mask

**Read first:** `concept.md` §3, §4, §7, §8, §12; `docs/DECISIONS.md`; and the
M0 code in `crates/color-cube` (it is yours to extend, not to rewrite).

**Mode:** question-first. Return open questions before writing files. Do not run
git. Use `./scripts/docker.sh` for every cargo invocation.

## Goal

Make the core crate usable by an RL agent and a benchmark harness: reproducible
uniform scrambles, a non-uniform random walk for demos, Singmaster notation,
the 324-float observation, and action masks.

## Deliverables

1. `SplitMix64` in the core crate (private), byte-identical to `mine-sm`:
   ```
   state += 0x9E3779B97F4A7C15
   z = state
   z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9
   z = (z ^ (z >> 27)) * 0x94D049BB133111EB
   z ^= z >> 31
   ```
   `next_bounded(bound)` uses `next_u64() % bound`. Document the bias bound
   (`<= bound / 2^64`) and that it is not corrected. No rejection sampling is
   used anywhere: the parity fix makes rejection unnecessary.

2. `Cube::from_seed(seed: u64) -> Cube`, exactly the six-step algorithm in
   `concept.md` §7.1:
   - Fisher–Yates `cp` over `[0..8)` and `ep` over `[0..12)`, drawing indices in
     ascending `i` with `j = i + next_bounded(len - i)`;
   - `co[0..7]` uniform in `0..3`, `co[7] = (3 - sum) % 3`;
   - `eo[0..11]` uniform bits, `eo[11] = sum % 2`;
   - if `parity(cp) != parity(ep)`, swap `ep[10]` and `ep[11]`;
   - if solved, retry with `seed + 1` (bounded retry loop, documented as
     measure-zero).
   The draw order above is the spec: it is part of the stable contract.

3. `Scramble { moves: [Move; 32], len: u8 }` with `as_slice()`, `is_empty()`,
   and `Cube::from_scramble(seed: u64, turns: u8) -> (Cube, Scramble)`.
   - `turns` is clamped to 32; document it.
   - Reject a candidate move that (a) is on the same face as the previous move,
     or (b) commutes with the previous move and is in non-canonical order
     (see D014).
   - If the walk lands on solved, append/retry deterministically (use the next
     candidate; document the exact rule).

4. Notation:
   - `Move::to_singmaster(self) -> &'static str` (`"U"`, `"U2"`, `"U'"`, ...).
   - `moves_to_singmaster(&[Move]) -> [u8; N]`-free of allocation? No: `no_std`
     without `alloc`. Provide `write_singmaster(&[Move], &mut [u8]) -> Result<usize, NotationError>`
     and a `std`-only convenience in the CLI later. Keep the core allocation-free.
   - `parse_moves(&str) -> Result<FixedMoves, ParseError>` where `FixedMoves` is
     a fixed-capacity 64-move list with `as_slice()`. Whitespace-separated
     tokens only; malformed tokens are errors, never skipped.

5. RL surface:
   - `Cube::write_observation(&self, out: &mut [f32; 324])` — exactly one `1.0`
     and five `0.0` per facelet, index `facelet * 6 + color`.
   - `Cube::write_action_mask(&self, out: &mut [f32; 18])` — all `1.0`.
   - `Cube::write_canonical_mask(&self, last: Option<Move>, out: &mut [f32; 18])`
     — see D014.

## Fixed decisions for this milestone

- **D014 (canonical pruning, resolving a spec conflict).** `concept.md` §8's
  example ("`D` after `U`") and §9's rule ("allow `D` only after `U`") disagree
  about which direction to prune. Follow §9: for opposite faces, keep the move
  whose face index is **higher** and prune the lower one (face order
  `U=0, R=1, F=2, D=3, L=4, B=5`), i.e. allow `D` after `U`, `L` after `R`,
  `B` after `F`; prune `U` after `D`, `R` after `L`, `F` after `B`. Record the
  conflict in the module docs. Same-face moves are always pruned.
- Canonical observation is the 324-float one-hot (D006).
- Golden hashes use FNV-1a 64 over the 54 colour bytes, implemented in the test
  tree (not `std::hash`, which is not stable across releases).

## Test gates

1. 10,000 seeds: `is_valid()`, not solved after the retry rule, `parity(cp) ==
   parity(ep)`, `sum(co) % 3 == 0`, `sum(eo) % 2 == 0`.
2. Determinism: same seed twice → identical cube; `seed -> facelets -> FNV`
   golden values committed for 32 seeds in a fixture file under `tests/`.
3. `from_scramble`: 1,000 seeds × a few `turns` values; length matches, every
   cube valid, none solved, no same-face or non-canonical consecutive pair.
4. Observation: for 1,000 cubes, exactly 54 ones overall, one per facelet,
   correct index for a hand-checked solved facelet; solved cube's observation
   matches the monochrome pattern.
5. Action mask: 18 ones; canonical mask prunes exactly the expected set for
   representative `last` values, and never prunes a legal non-redundant move.
6. Notation: round-trip all 18 moves; parse/format round-trip for random move
   lists; reject `"U3"`, `"X"`, `"U U'"`? (the last is two moves, not an error);
   reject empty token sequences only if the input is non-empty garbage.
7. `parse_moves` on a 64-token list succeeds; 65 tokens → error, not a panic.

## Out of scope

Coordinates, tables, solvers, CLI behaviour, WASM. Do not change existing public
types except to add methods; do not change the M0 move derivation.

## Reporting

Files added/changed, commands run with output, interpretations, unresolved
items. Under 800 words. Do not commit.
