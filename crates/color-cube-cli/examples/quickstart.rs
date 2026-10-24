//! Core-only quickstart: seeded state, a move, facelets, observation, mask.
//!
//! Run with: `./scripts/docker.sh cargo run --release -p color-cube-cli --example quickstart`

use color_cube::{Cube, Move, N_MOVES, N_OBSERVATIONS};

fn main() {
    let mut cube = Cube::from_seed(42);
    println!(
        "seeded: valid={} solved={}",
        cube.is_valid(),
        cube.is_solved()
    );

    cube.apply(Move::from_index(0).expect("U is a move"));
    let facelets = cube.facelets();
    println!("first face after U: {:?}", &facelets[..9]);

    let mut observation = [0.0f32; N_OBSERVATIONS];
    cube.write_observation(&mut observation);
    println!(
        "observation: len={} sum={}",
        observation.len(),
        observation.iter().sum::<f32>()
    );

    let mut mask = [0.0f32; N_MOVES];
    cube.write_action_mask(&mut mask);
    println!("action mask sum={}", mask.iter().sum::<f32>());
}
