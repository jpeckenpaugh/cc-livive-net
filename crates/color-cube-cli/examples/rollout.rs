//! Zero-dependency episode rollout, the "first agent" recipe in docs/RL.md.
//!
//! It demonstrates the environment loop only: observation, canonical mask,
//! action, reward. The policy is a deterministic random walk; nothing here
//! learns. Run with:
//! `./scripts/docker.sh cargo run --release -p color-cube-cli --example rollout`

use color_cube::{Cube, Move, N_MOVES, N_OBSERVATIONS};

const MAX_STEPS: usize = 100;
const SOLVED_REWARD: f32 = 1.0;
const STEP_REWARD: f32 = -0.01;

fn main() {
    let mut cube = Cube::from_seed(7);
    let mut last: Option<Move> = None;
    let mut observation = [0.0f32; N_OBSERVATIONS];
    let mut mask = [0.0f32; N_MOVES];
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let mut total_reward = 0.0f32;
    let mut steps = 0;

    for _ in 0..MAX_STEPS {
        cube.write_observation(&mut observation);
        cube.write_canonical_mask(last, &mut mask);

        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let start = (state >> 33) as usize % N_MOVES;
        let action = (0..N_MOVES)
            .map(|offset| (start + offset) % N_MOVES)
            .find(|&index| mask[index] == 1.0)
            .expect("at least one move is always legal");

        let mv = Move::from_index(action as u8).expect("action is in 0..18");
        cube.apply(mv);
        last = Some(mv);
        steps += 1;
        total_reward += STEP_REWARD;

        if cube.is_solved() {
            total_reward += SOLVED_REWARD;
            break;
        }
    }

    println!(
        "rollout: steps={steps} solved={} return={total_reward:.2}",
        cube.is_solved()
    );
}
