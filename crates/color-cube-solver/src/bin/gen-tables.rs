//! Generates `cc-tables-v1.bin` and prints per-table timings and sizes.
//!
//! Usage: `cargo run --release -p color-cube-solver --bin gen-tables`.
//! The output directory is `$COLOR_CUBE_TABLES`, or `./tables`.

use std::process::ExitCode;

use color_cube_solver::tables;

fn main() -> ExitCode {
    let directory = tables::default_directory();
    println!("generating move tables into {}", directory.display());

    let (bundle, timings) = tables::generate_with_timings();
    let sizes = bundle.payload_sizes();
    let names = ["twist", "flip", "slice", "cp", "ep8", "sp"];
    for (slot, timing) in timings.iter().enumerate() {
        println!(
            "  {:<5} {:>8.3} ms  {:>10} bytes",
            names[slot],
            timing.elapsed.as_secs_f64() * 1000.0,
            sizes[slot]
        );
    }

    match tables::write(&directory, &bundle) {
        Ok(path) => {
            let total: usize = sizes.iter().sum();
            println!("wrote {} ({} payload bytes)", path.display(), total);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to write tables: {error}");
            ExitCode::FAILURE
        }
    }
}
