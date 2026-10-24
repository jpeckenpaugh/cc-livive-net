//! Generates `cc-tables-v1.bin` and `cc-thistlethwaite-v1.bin`.
//!
//! Usage: `cargo run --release -p color-cube-solver --bin gen-tables [--verify]`.
//! The output directory is `$COLOR_CUBE_TABLES`, or `./tables`.
//! `--verify` additionally exhaustively BFSes the 3.98M-element `G3` space.

use std::process::ExitCode;

use color_cube_solver::tables;
use color_cube_solver::thistlethwaite;

fn main() -> ExitCode {
    let directory = tables::default_directory();
    let verify = std::env::args().any(|arg| arg == "--verify");
    println!("generating tables into {}", directory.display());

    let (moves, move_timings) = tables::generate_with_timings();
    let move_sizes = moves.payload_sizes();
    let move_names = ["twist", "flip", "slice", "cp", "ep8", "sp"];
    for (slot, timing) in move_timings.iter().enumerate() {
        println!(
            "  {:<12} {:>8.3} ms  {:>10} bytes",
            move_names[slot],
            timing.elapsed.as_secs_f64() * 1000.0,
            move_sizes[slot]
        );
    }

    let (distances, distance_timings, report) = thistlethwaite::generate_distance_tables(&moves);
    let distance_sizes = distances.payload_sizes();
    let distance_names = [
        "phase1_flip",
        "phase2_twist_slice",
        "phase3_quotient",
        "phase4_cp",
        "phase4_ep_full",
    ];
    for (slot, timing) in distance_timings.iter().enumerate() {
        println!(
            "  {:<20} {:>8.3} ms  {:>10} bytes",
            distance_names[slot],
            timing.elapsed.as_secs_f64() * 1000.0,
            distance_sizes[slot]
        );
    }
    println!(
        "diameters: phase1={} phase2={} phase3={} phase4_cp={} phase4_ep={}",
        report.phase1_max,
        report.phase2_max,
        report.phase3_max,
        report.phase4_cp_max,
        report.phase4_ep_max
    );

    if verify {
        let (states, diameter) = thistlethwaite::verify_g3_diameter();
        println!("G3: reachable={states} diameter={diameter}");
        if states != 663_552 || diameter > 15 {
            eprintln!("G3 verification failed: need reachable=663552 and diameter<=15");
            return ExitCode::FAILURE;
        }
    }

    match tables::write(&directory, &moves) {
        Ok(path) => println!("wrote {}", path.display()),
        Err(error) => {
            eprintln!("failed to write move tables: {error}");
            return ExitCode::FAILURE;
        }
    }
    match distances.write(&directory) {
        Ok(path) => println!("wrote {}", path.display()),
        Err(error) => {
            eprintln!("failed to write distance tables: {error}");
            return ExitCode::FAILURE;
        }
    }

    let move_total: usize = move_sizes.iter().sum();
    let distance_total: usize = distance_sizes.iter().sum();
    println!(
        "payloads: moves={move_total} bytes distances={distance_total} bytes total={} bytes",
        move_total + distance_total
    );
    ExitCode::SUCCESS
}
