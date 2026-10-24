//! Solver quickstart: solve a seeded cube and replay the answer.
//!
//! Requires generated tables (`./scripts/gen-tables.sh`).
//! Run with: `./scripts/docker.sh cargo run --release -p color-cube-cli --example solve`

use color_cube::{Cube, Move};
use color_cube_solver::{Limits, Solver, Thistlethwaite};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cube = Cube::from_seed(42);
    let solver = Thistlethwaite::new();
    let solution = solver.solve(&cube, Limits::default())?;

    let text: Vec<&str> = solution
        .moves
        .iter()
        .map(|mv: &Move| mv.to_singmaster())
        .collect();
    println!("moves={} solution={}", solution.moves.len(), text.join(" "));

    let mut replay = cube;
    replay.apply_all(&solution.moves);
    println!("replay solved={}", replay.is_solved());
    Ok(())
}
