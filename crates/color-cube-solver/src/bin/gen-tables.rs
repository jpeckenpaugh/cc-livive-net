//! Generates `cc-tables-v1.bin`, `cc-thistlethwaite-v1.bin` and
//! `cc-kociemba-v1.bin`.
//!
//! Usage: `cargo run --release -p color-cube-solver --bin gen-tables [--verify]`.
//! The output directory is `$COLOR_CUBE_TABLES`, or `./tables`.
//! `--verify` additionally exhaustively BFSes the 3.98M-element `G3` space.

use std::process::ExitCode;

use color_cube_solver::kociemba;
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

    let (kociemba_distances, kociemba_timings, kociemba_report) =
        kociemba::generate_distance_tables(&moves);
    let kociemba_sizes = kociemba_distances.payload_sizes();
    let kociemba_names = [
        "phase1_twist_slice",
        "phase1_flip_slice",
        "phase2_cp_sp",
        "phase2_ep8_sp",
        "phase1_twist_flip",
    ];
    for (slot, timing) in kociemba_timings.iter().enumerate() {
        println!(
            "  {:<20} {:>8.3} ms  {:>10} bytes",
            kociemba_names[slot],
            timing.elapsed.as_secs_f64() * 1000.0,
            kociemba_sizes[slot]
        );
    }
    println!("kociemba: {kociemba_report}");
    println!(
        "kociemba coverage full={} reached={} total={}",
        kociemba_report.fully_covered(),
        kociemba_report.phase1_twist_slice_reached
            + kociemba_report.phase1_flip_slice_reached
            + kociemba_report.phase2_cp_sp_reached
            + kociemba_report.phase2_ep8_sp_reached
            + kociemba_report.phase1_twist_flip_reached,
        kociemba::PHASE1_TWIST_SLICE_ROWS
            + kociemba::PHASE1_FLIP_SLICE_ROWS
            + kociemba::PHASE2_CP_SP_ROWS
            + kociemba::PHASE2_EP8_SP_ROWS
            + kociemba::PHASE1_TWIST_FLIP_ROWS
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
    match kociemba_distances.write(&directory) {
        Ok(path) => println!("wrote {}", path.display()),
        Err(error) => {
            eprintln!("failed to write kociemba distance tables: {error}");
            return ExitCode::FAILURE;
        }
    }

    let move_total: usize = move_sizes.iter().sum();
    let distance_total: usize = distance_sizes.iter().sum();
    let kociemba_total: usize = kociemba_sizes.iter().sum();
    println!(
        "payloads: moves={move_total} bytes distances={distance_total} bytes \
         kociemba={kociemba_total} bytes total={} bytes",
        move_total + distance_total + kociemba_total
    );
    if let Some(peak) = peak_rss_kb() {
        println!("peak_rss_kb={peak}");
    }
    ExitCode::SUCCESS
}

fn peak_rss_kb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}
