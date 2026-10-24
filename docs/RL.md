# Reinforcement-learning interface

The core is a deterministic environment: a cube state, 18 legal actions, a
324-float observation, and masks. There is no Python dependency and no hidden
global state. Canonical conventions are fixed by `concept.md` §3 and
`docs/DECISIONS.md` (D006, D014, D015).

## Actions

`Move` is the action index. All 18 face turns are always legal, and action
indices are face-major in `U R F D L B` order.

| Index | Move | Inverse | Inverse index | HTM cost |
|---:|---|---|---:|---:|
| 0 | `U` | `U'` | 2 | 1 |
| 1 | `U2` | `U2` | 1 | 2 |
| 2 | `U'` | `U` | 0 | 1 |
| 3 | `R` | `R'` | 5 | 1 |
| 4 | `R2` | `R2` | 4 | 2 |
| 5 | `R'` | `R` | 3 | 1 |
| 6 | `F` | `F'` | 8 | 1 |
| 7 | `F2` | `F2` | 7 | 2 |
| 8 | `F'` | `F` | 6 | 1 |
| 9 | `D` | `D'` | 11 | 1 |
| 10 | `D2` | `D2` | 10 | 2 |
| 11 | `D'` | `D` | 9 | 1 |
| 12 | `L` | `L'` | 14 | 1 |
| 13 | `L2` | `L2` | 13 | 2 |
| 14 | `L'` | `L` | 12 | 1 |
| 15 | `B` | `B'` | 17 | 1 |
| 16 | `B2` | `B2` | 16 | 2 |
| 17 | `B'` | `B` | 15 | 1 |

`Move::from_index` is the checked constructor; `move.inverse()` and
`move.quarter_turns()` are the table above in code.

## Observation

`Cube::write_observation(&mut [f32; 324])` writes the canonical one-hot
observation (D006). For facelet `f` with colour index `c`, entry `f * 6 + c` is
`1.0`; the other five entries for that facelet are `0.0`.

- Facelets are ordered `U R F D L B`, nine per face, row-major as seen from
  outside that face.
- Colour indices are `U=0, D=1, F=2, B=3, R=4, L=5` — **not** face order
  (D009).
- Invariant: exactly 54 ones, sum `54.0`; every facelet contributes one one.

The solved facelet array is `[0; 9]`, `[4; 9]`, `[2; 9]`, `[1; 9]`, `[5; 9]`,
`[3; 9]` in face order. The compact 54-float form is an auxiliary helper only;
benchmarks and RL must use the 324-float form.

## Action masks

`Cube::write_action_mask` fills 18 ones (the default: every move is legal).
`Cube::write_canonical_mask(last, out)` is an opt-in helper that removes
immediately redundant moves (D014):

- every power of the same face as `last` is zeroed;
- for an opposite pair, only the **higher** face index survives, so `D` is
  allowed after `U`, `L` after `R`, `B` after `F`, and the reverse is pruned.

Examples:

| `last` | Zeroed indices | Legal moves |
|---|---|---:|
| `None` | none | 18 |
| `U` (0) | 0, 1, 2 | 15 |
| `D` (9) | 0, 1, 2, 9, 10, 11 | 12 |
| `R` (3) | 3, 4, 5 | 15 |
| `L` (12) | 3, 4, 5, 12, 13, 14 | 12 |

Always train against the unrestricted mask unless you have a reason not to:
canonical pruning changes the reachable action sequences and is not required for
correctness.

## Episodes and reward

These are documented constants, not enforced behaviour:

- maximum 100 moves per episode; terminate on solved;
- `SOLVED_REWARD = +1.0`, `STEP_REWARD = -0.01` per move; or the published
  alternative `-1.0` per move with `0.0` at success;
- discount, bootstrapping, and whether to reward shaping are yours.

## Curriculum

Uniform random states are about 17–18 optimal moves from solved and are a hard
starting point. To make easy states, apply `n` random moves
(`Cube::from_scramble(seed, n)`) and record `n` as a rough difficulty label.
`n` is **not** the optimal distance; the same state can be reachable in fewer
moves. Curriculum by true optimal distance requires the optional M6 oracle.

## Determinism

- `Cube::from_seed(seed)` implements the frozen 36-draw contract (D015). The
  draw order is part of the API; changing it invalidates all golden fixtures.
- `Cube::from_scramble(seed, turns)` is a pruned random walk for demos. It is
  deterministic for a given seed and turn count, but not uniform over states.
- Golden hashes in `crates/color-cube/tests/` pin seeds to facelets.
- Solver output is deterministic for a given cube, tables, and limits.

## First agent

The recipe is: observe → mask → choose an action → apply → reward → repeat.
Start from `crates/color-cube-cli/examples/rollout.rs`, which is a zero-dependency
episode rollout (it does **not** learn):

```text
cube = Cube::from_seed(seed)
observation = [f32; 324]; mask = [f32; 18]; last = None; return = 0
for step in 0..100:
    cube.write_observation(&mut observation)
    cube.write_canonical_mask(last, &mut mask)
    action = policy(observation, mask)          # your model
    cube.apply(Move::from_index(action).unwrap())
    last = Some(action)
    return += STEP_REWARD
    if cube.is_solved():
        return += SOLVED_REWARD
        break
```

Run it with
`./scripts/docker.sh cargo run --release -p color-cube-cli --example rollout`.
