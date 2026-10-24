//! Dependency-free command-line front end for the colour cube.
//!
//! Commands:
//! `solve --seed <u64> | --moves "..."`, `scramble --seed <u64>`,
//! `check --seed <u64>`, and `bench`. The solver tables are loaded from
//! `$COLOR_CUBE_TABLES` (default `./tables`).

use std::process::ExitCode;
use std::time::Instant;

use color_cube::{parse_moves, Cube, Move};
use color_cube_solver::{
    kociemba, tables, thistlethwaite, Kociemba, Limits, SolveError, Solver, Thistlethwaite,
};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let Some(command) = args.get(1).map(String::as_str) else {
        eprintln!("usage: color-cube-cli <solve|scramble|check> ...");
        return ExitCode::FAILURE;
    };
    let result = match command {
        "solve" => solve(&args[2..]),
        "scramble" => scramble(&args[2..]),
        "check" => check(&args[2..]),
        "bench" => bench(&args[2..]),
        "optimal" => {
            #[cfg(feature = "optimal")]
            {
                optimal(&args[2..])
            }
            #[cfg(not(feature = "optimal"))]
            {
                Err("this binary was built without the `optimal` feature".into())
            }
        }
        "--help" | "-h" | "help" => {
            eprintln!(
                "usage:\n  color-cube-cli solve --seed <u64> [--solver thistlethwaite|kociemba] [--time-ms N] [--max-length N]\n  color-cube-cli solve --moves \"U R2 F'\" [--solver ...]\n  color-cube-cli optimal --seed <u64> | --moves \"...\" [--time-ms N] [--max-length N]\n  color-cube-cli scramble --seed <u64> [--turns N]\n  color-cube-cli check --seed <u64>\n  color-cube-cli bench [--n N] [--warmup N] [--solver thistlethwaite|kociemba|both] [--gen]"
            );
            Ok(())
        }
        other => Err(format!("unknown command: {other}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn solve(args: &[String]) -> Result<(), String> {
    let mut seed = None;
    let mut moves = None;
    let mut time_ms = None;
    let mut max_length = None;
    let mut solver_name = String::from("thistlethwaite");
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--seed" => {
                seed = Some(parse_next(args, &mut index)?);
            }
            "--moves" => {
                moves = Some(next_value(args, &mut index)?);
            }
            "--time-ms" => {
                time_ms = Some(parse_next(args, &mut index)?);
            }
            "--max-length" => {
                max_length = Some(parse_next(args, &mut index)?);
            }
            "--solver" => {
                solver_name = next_value(args, &mut index)?;
            }
            other => return Err(format!("unknown solve option: {other}")),
        }
    }
    if seed.is_some() == moves.is_some() {
        return Err("solve requires exactly one of --seed or --moves".into());
    }

    let cube = if let Some(seed) = seed {
        Cube::from_seed(seed)
    } else {
        let parsed = parse_moves(moves.as_deref().unwrap_or_default())
            .map_err(|error| format!("invalid move list: {error}"))?;
        let mut cube = Cube::SOLVED;
        cube.apply_all(parsed.as_slice());
        cube
    };

    let limits = Limits {
        max_length,
        time_ms,
    };
    match solver_name.as_str() {
        "thistlethwaite" => solve_thistlethwaite(&cube, limits),
        "kociemba" => solve_kociemba(&cube, limits),
        other => Err(format!(
            "unknown solver: {other} (expected thistlethwaite or kociemba)"
        )),
    }
}

fn solve_thistlethwaite(cube: &Cube, limits: Limits) -> Result<(), String> {
    let solver = Thistlethwaite::new();
    let started = Instant::now();
    match solver.solve_with_stats(cube, limits) {
        Ok((solution, phases, nodes)) => {
            println!("{}", format_moves(&solution.moves));
            eprintln!(
                "solver=thistlethwaite phases={} {} {} {} nodes={} elapsed_ms={:.3}",
                phases[0],
                phases[1],
                phases[2],
                phases[3],
                nodes,
                started.elapsed().as_secs_f64() * 1000.0
            );
            Ok(())
        }
        Err(SolveError::AlreadySolved) => {
            println!();
            eprintln!(
                "solver=thistlethwaite phases=0 0 0 0 nodes=0 elapsed_ms={:.3}",
                started.elapsed().as_secs_f64() * 1000.0
            );
            Ok(())
        }
        Err(SolveError::TablesMissing) => {
            if let Some(error) = solver.load_error() {
                Err(format!("solve failed: {error}"))
            } else {
                Err("solve failed: solver tables are missing or corrupt".into())
            }
        }
        Err(error) => Err(format!("solve failed: {error}")),
    }
}

fn solve_kociemba(cube: &Cube, limits: Limits) -> Result<(), String> {
    let solver = Kociemba::new();
    let started = Instant::now();
    match solver.solve_with_stats(cube, limits) {
        Ok((solution, stats)) => {
            println!("{}", format_moves(&solution.moves));
            let phases = solution.phases.unwrap_or([0, 0]);
            eprintln!(
                "solver=kociemba phases={} {} total={} phase1_nodes={} phase2_calls={} \
                 phase2_nodes={} elapsed_ms={:.3}",
                phases[0],
                phases[1],
                solution.moves.len(),
                stats.phase1_nodes,
                stats.phase2_calls,
                stats.phase2_nodes,
                started.elapsed().as_secs_f64() * 1000.0
            );
            Ok(())
        }
        Err(SolveError::AlreadySolved) => {
            println!();
            eprintln!(
                "solver=kociemba phases=0 0 total=0 elapsed_ms={:.3}",
                started.elapsed().as_secs_f64() * 1000.0
            );
            Ok(())
        }
        Err(SolveError::TablesMissing) => {
            if let Some(error) = solver.load_error() {
                Err(format!("solve failed: {error}"))
            } else {
                Err("solve failed: solver tables are missing or corrupt".into())
            }
        }
        Err(error) => Err(format!("solve failed: {error}")),
    }
}

#[cfg(feature = "optimal")]
fn optimal(args: &[String]) -> Result<(), String> {
    use color_cube_solver::OptimalOracle;

    let mut seed = None;
    let mut moves = None;
    let mut time_ms = Some(300_000u32);
    let mut max_length = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--seed" => seed = Some(parse_next(args, &mut index)?),
            "--moves" => moves = Some(next_value(args, &mut index)?),
            "--time-ms" => time_ms = Some(parse_next(args, &mut index)?),
            "--max-length" => max_length = Some(parse_next(args, &mut index)?),
            other => return Err(format!("unknown optimal option: {other}")),
        }
    }
    if seed.is_some() == moves.is_some() {
        return Err("optimal requires exactly one of --seed or --moves".into());
    }
    let cube = if let Some(seed) = seed {
        Cube::from_seed(seed)
    } else {
        let parsed = parse_moves(moves.as_deref().unwrap_or_default())
            .map_err(|error| format!("invalid move list: {error}"))?;
        let mut cube = Cube::SOLVED;
        cube.apply_all(parsed.as_slice());
        cube
    };
    let directory = tables::default_directory();
    let load_started = Instant::now();
    let oracle = OptimalOracle::load(&directory).map_err(|error| {
        format!(
            "failed to load optimal tables from {}: {error}",
            directory.display()
        )
    })?;
    eprintln!(
        "loaded optimal tables in {:.3} s (diameters corner={} edge_a={} edge_b={})",
        load_started.elapsed().as_secs_f64(),
        oracle.diameters().0,
        oracle.diameters().1,
        oracle.diameters().2
    );
    let limits = Limits {
        max_length,
        time_ms,
    };
    let started = Instant::now();
    let outcome = oracle.solve_with_stats(&cube, limits);
    let stats = &outcome.stats;
    let by_bound: Vec<String> = stats
        .nodes_by_bound
        .iter()
        .enumerate()
        .filter(|(_, &nodes)| nodes > 0)
        .map(|(bound, &nodes)| format!("{bound}:{nodes}"))
        .collect();
    let summary = format!(
        "solver=optimal h0={} bound={} deepest_exhausted={:?} nodes={} by_bound=[{}] elapsed_ms={:.3}",
        stats.initial_h,
        stats.bound,
        stats.deepest_exhausted,
        stats.nodes,
        by_bound.join(","),
        started.elapsed().as_secs_f64() * 1000.0
    );
    match &outcome.solution {
        Ok(solution) => {
            println!("{}", format_moves(&solution.moves));
            eprintln!(
                "total={} optimal={} {summary}",
                solution.moves.len(),
                solution.optimal
            );
            Ok(())
        }
        Err(SolveError::AlreadySolved) => {
            println!();
            eprintln!("{summary}");
            Ok(())
        }
        Err(SolveError::TablesMissing) => Err("solve failed: optimal tables are missing".into()),
        Err(error) => {
            eprintln!("{summary}");
            Err(format!("solve failed: {error}"))
        }
    }
}

fn scramble(args: &[String]) -> Result<(), String> {
    let mut seed = None;
    let mut turns = 25u8;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--seed" => seed = Some(parse_next(args, &mut index)?),
            "--turns" => turns = parse_next(args, &mut index)?,
            other => return Err(format!("unknown scramble option: {other}")),
        }
    }
    let seed = seed.ok_or_else(|| "scramble requires --seed".to_string())?;
    let (_, scramble) = Cube::from_scramble(seed, turns);
    println!("{}", format_moves(scramble.as_slice()));
    Ok(())
}

fn check(args: &[String]) -> Result<(), String> {
    let mut seed = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--seed" => seed = Some(parse_next(args, &mut index)?),
            other => return Err(format!("unknown check option: {other}")),
        }
    }
    let seed = seed.ok_or_else(|| "check requires --seed".to_string())?;
    let cube = Cube::from_seed(seed);
    let valid = cube.is_valid();
    let solved = cube.is_solved();
    println!(
        "valid={valid} solved={solved} hash={:016x}",
        hash_facelets(&cube)
    );
    if valid {
        Ok(())
    } else {
        Err("cube state is invalid".into())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BenchSolver {
    Both,
    Thistlethwaite,
    Kociemba,
}

/// Benchmarks are non-gating (concept §12) and intentionally dependency-free.
///
/// Run with `./scripts/docker.sh cargo run --release -p color-cube-cli -- bench`.
/// Timings are only meaningful in release mode and on a quiet machine; the
/// Docker wrapper output is labelled "indicative" in `docs/SOLVERS.md`.
fn bench(args: &[String]) -> Result<(), String> {
    let mut n = 100usize;
    let mut warmup = 10usize;
    let mut solver = BenchSolver::Both;
    let mut generate = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--n" => n = parse_next(args, &mut index)?,
            "--warmup" => warmup = parse_next(args, &mut index)?,
            "--solver" => {
                solver = match next_value(args, &mut index)?.as_str() {
                    "both" => BenchSolver::Both,
                    "thistlethwaite" => BenchSolver::Thistlethwaite,
                    "kociemba" => BenchSolver::Kociemba,
                    other => {
                        return Err(format!(
                        "unknown bench solver: {other} (expected both, thistlethwaite or kociemba)"
                    ))
                    }
                };
            }
            "--gen" => {
                generate = true;
                index += 1;
            }
            other => return Err(format!("unknown bench option: {other}")),
        }
    }
    if n == 0 {
        return Err("bench --n must be at least 1".into());
    }

    println!("bench: release-only, n={n}, warmup={warmup}");
    report_tables(generate)?;
    bench_apply();

    if solver != BenchSolver::Kociemba {
        bench_thistlethwaite(n, warmup)?;
    }
    if solver != BenchSolver::Thistlethwaite {
        bench_kociemba(n, warmup)?;
    }
    Ok(())
}

fn report_tables(generate: bool) -> Result<(), String> {
    let directory = tables::default_directory();
    println!("tables: {}", directory.display());
    for file in [
        tables::BUNDLE_FILE,
        thistlethwaite::THISTLE_BUNDLE_FILE,
        kociemba::KOCIEMBA_BUNDLE_FILE,
    ] {
        let path = directory.join(file);
        match std::fs::metadata(&path) {
            Ok(metadata) => println!("  {file:<28} {:>10} bytes", metadata.len()),
            Err(_) => println!("  {file:<28} missing (run ./scripts/gen-tables.sh)"),
        }
    }

    let (moves, timings) = tables::generate_with_timings();
    println!("move-table generation:");
    let names = ["twist", "flip", "slice", "cp", "ep8", "sp"];
    let sizes = moves.payload_sizes();
    for (slot, timing) in timings.iter().enumerate() {
        println!(
            "  {:<12} {:>10.3} ms {:>10} bytes",
            names[slot],
            timing.elapsed.as_secs_f64() * 1000.0,
            sizes[slot]
        );
    }

    if generate {
        let (distances, timings, report) = thistlethwaite::generate_distance_tables(&moves);
        println!("thistlethwaite-table generation:");
        let names = [
            "phase1_flip",
            "phase2_twist_slice",
            "phase3_quotient",
            "phase4_cp",
            "phase4_ep_full",
        ];
        let sizes = distances.payload_sizes();
        for (slot, timing) in timings.iter().enumerate() {
            println!(
                "  {:<20} {:>10.3} ms {:>10} bytes",
                names[slot],
                timing.elapsed.as_secs_f64() * 1000.0,
                sizes[slot]
            );
        }
        println!(
            "  diameters: {} {} {} {} {}",
            report.phase1_max,
            report.phase2_max,
            report.phase3_max,
            report.phase4_cp_max,
            report.phase4_ep_max
        );

        let (distances, timings, report) = kociemba::generate_distance_tables(&moves);
        println!("kociemba-table generation:");
        let names = [
            "phase1_twist_slice",
            "phase1_flip_slice",
            "phase2_cp_sp",
            "phase2_ep8_sp",
            "phase1_twist_flip",
        ];
        let sizes = distances.payload_sizes();
        for (slot, timing) in timings.iter().enumerate() {
            println!(
                "  {:<20} {:>10.3} ms {:>10} bytes",
                names[slot],
                timing.elapsed.as_secs_f64() * 1000.0,
                sizes[slot]
            );
        }
        println!("  {report}");
    }
    Ok(())
}

fn bench_apply() {
    const MOVES: usize = 1_000_000;
    let mut cube = Cube::SOLVED;
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let started = Instant::now();
    for _ in 0..MOVES {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let index = ((state >> 33) % 18) as u8;
        let mv = Move::from_index(index).expect("move index is in 0..18");
        cube.apply(mv);
    }
    let elapsed = started.elapsed();
    std::hint::black_box(cube.is_solved());
    let moves_per_second = MOVES as f64 / elapsed.as_secs_f64();
    println!(
        "apply: {MOVES} moves in {:.3} ms ({moves_per_second:.0} moves/s)",
        elapsed.as_secs_f64() * 1000.0
    );
}

struct Stats {
    min: f64,
    median: f64,
    p95: f64,
    max: f64,
    mean: f64,
}

fn summarize(values: &mut [u64]) -> Stats {
    values.sort_unstable();
    let count = values.len();
    let percentile = (count * 95).div_ceil(100).saturating_sub(1).min(count - 1);
    let sum: u64 = values.iter().sum();
    let median = if count % 2 == 1 {
        values[count / 2] as f64
    } else {
        (values[count / 2 - 1] + values[count / 2]) as f64 / 2.0
    };
    Stats {
        min: values[0] as f64,
        median,
        p95: values[percentile] as f64,
        max: values[count - 1] as f64,
        mean: sum as f64 / count as f64,
    }
}

fn print_time(label: &str, stats: &Stats) {
    println!(
        "  {label:<8} min={:.3} ms median={:.3} ms p95={:.3} ms max={:.3} ms mean={:.3} ms",
        stats.min / 1000.0,
        stats.median / 1000.0,
        stats.p95 / 1000.0,
        stats.max / 1000.0,
        stats.mean / 1000.0
    );
}

fn print_count(label: &str, stats: &Stats) {
    println!(
        "  {label:<8} min={:.0} median={:.0} p95={:.0} max={:.0} mean={:.2}",
        stats.min, stats.median, stats.p95, stats.max, stats.mean
    );
}

fn bench_thistlethwaite(n: usize, warmup: usize) -> Result<(), String> {
    let solver = Thistlethwaite::new();
    for seed in 0..warmup as u64 {
        let _ = solver.solve(&Cube::from_seed(seed), Limits::default());
    }

    let mut times = Vec::with_capacity(n);
    let mut lengths = Vec::with_capacity(n);
    let mut phase_totals = [0u64; 4];
    for seed in 0..n as u64 {
        let cube = Cube::from_seed(seed);
        let started = Instant::now();
        let result = solver.solve_with_stats(&cube, Limits::default());
        let elapsed = started.elapsed().as_micros() as u64;
        let (solution, phases, _nodes) = result.map_err(|error| {
            if error == SolveError::TablesMissing {
                format!(
                    "thistlethwaite tables unavailable: {}",
                    solver.load_error().map_or_else(
                        || "unknown load error".to_string(),
                        |error| error.to_string()
                    )
                )
            } else {
                format!("thistlethwaite failed on seed {seed}: {error}")
            }
        })?;
        times.push(elapsed);
        lengths.push(solution.moves.len() as u64);
        for (total, &phase) in phase_totals.iter_mut().zip(phases.iter()) {
            *total += phase as u64;
        }
    }

    let time_stats = summarize(&mut times);
    let length_stats = summarize(&mut lengths);
    println!("thistlethwaite: n={n} warmup={warmup}");
    print_time("time", &time_stats);
    print_count("moves", &length_stats);
    println!(
        "  phases   mean={:.2} {:.2} {:.2} {:.2}",
        phase_totals[0] as f64 / n as f64,
        phase_totals[1] as f64 / n as f64,
        phase_totals[2] as f64 / n as f64,
        phase_totals[3] as f64 / n as f64
    );
    Ok(())
}

fn bench_kociemba(n: usize, warmup: usize) -> Result<(), String> {
    let solver = Kociemba::new();
    for seed in 0..warmup as u64 {
        let _ = solver.solve(&Cube::from_seed(seed), Limits::default());
    }

    let mut times = Vec::with_capacity(n);
    let mut lengths = Vec::with_capacity(n);
    let mut phase1_lengths = Vec::with_capacity(n);
    let mut phase2_lengths = Vec::with_capacity(n);
    let mut phase1_nodes = 0u64;
    let mut phase2_calls = 0u64;
    let mut phase2_nodes = 0u64;
    for seed in 0..n as u64 {
        let cube = Cube::from_seed(seed);
        let started = Instant::now();
        let result = solver.solve_with_stats(&cube, Limits::default());
        let elapsed = started.elapsed().as_micros() as u64;
        let (solution, stats) = result.map_err(|error| {
            if error == SolveError::TablesMissing {
                format!(
                    "kociemba tables unavailable: {}",
                    solver.load_error().map_or_else(
                        || "unknown load error".to_string(),
                        |error| error.to_string()
                    )
                )
            } else {
                format!("kociemba failed on seed {seed}: {error}")
            }
        })?;
        let phases = solution.phases.unwrap_or([0, 0]);
        times.push(elapsed);
        lengths.push(solution.moves.len() as u64);
        phase1_lengths.push(phases[0] as u64);
        phase2_lengths.push(phases[1] as u64);
        phase1_nodes += stats.phase1_nodes;
        phase2_calls += stats.phase2_calls;
        phase2_nodes += stats.phase2_nodes;
    }

    let time_stats = summarize(&mut times);
    let length_stats = summarize(&mut lengths);
    let phase1_stats = summarize(&mut phase1_lengths);
    let phase2_stats = summarize(&mut phase2_lengths);
    println!("kociemba: n={n} warmup={warmup}");
    print_time("time", &time_stats);
    print_count("moves", &length_stats);
    print_count("phase1", &phase1_stats);
    print_count("phase2", &phase2_stats);
    println!(
        "  search   phase1_nodes_mean={:.0} phase2_calls_mean={:.2} phase2_nodes_mean={:.0}",
        phase1_nodes as f64 / n as f64,
        phase2_calls as f64 / n as f64,
        phase2_nodes as f64 / n as f64
    );
    Ok(())
}

fn parse_next<T: std::str::FromStr>(args: &[String], index: &mut usize) -> Result<T, String> {
    let flag = args
        .get(*index)
        .cloned()
        .ok_or_else(|| "missing command-line option".to_string())?;
    *index += 1;
    let value = args
        .get(*index)
        .cloned()
        .ok_or_else(|| format!("missing value for {flag}"))?;
    *index += 1;
    value
        .parse()
        .map_err(|_| format!("invalid value for {flag}: {value}"))
}

fn next_value(args: &[String], index: &mut usize) -> Result<String, String> {
    let flag = args
        .get(*index)
        .cloned()
        .ok_or_else(|| "missing command-line option".to_string())?;
    *index += 1;
    let value = args
        .get(*index)
        .cloned()
        .ok_or_else(|| format!("missing value for {flag}"))?;
    *index += 1;
    Ok(value)
}

fn format_moves(moves: &[color_cube::Move]) -> String {
    let mut out = Vec::with_capacity(moves.len() * 4);
    for (index, &mv) in moves.iter().enumerate() {
        if index > 0 {
            out.push(b' ');
        }
        out.extend_from_slice(mv.to_singmaster().as_bytes());
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hash_facelets(cube: &Cube) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for color in cube.facelets() {
        hash ^= color.index() as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
