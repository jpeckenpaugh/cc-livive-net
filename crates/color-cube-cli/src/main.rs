//! Dependency-free command-line front end for the colour cube.
//!
//! Commands:
//! `solve --seed <u64> | --moves "..."`, `scramble --seed <u64>`, and
//! `check --seed <u64>`. The solver tables are loaded from
//! `$COLOR_CUBE_TABLES` (default `./tables`).

use std::process::ExitCode;
use std::time::Instant;

use color_cube::{parse_moves, Cube};
use color_cube_solver::{Limits, SolveError, Thistlethwaite};

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
        "--help" | "-h" | "help" => {
            eprintln!(
                "usage:\n  color-cube-cli solve --seed <u64> [--time-ms N] [--max-length N]\n  color-cube-cli solve --moves \"U R2 F'\"\n  color-cube-cli scramble --seed <u64> [--turns N]\n  color-cube-cli check --seed <u64>"
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

    let solver = Thistlethwaite::new();
    let started = Instant::now();
    let limits = Limits {
        max_length,
        time_ms,
    };
    match solver.solve_with_stats(&cube, limits) {
        Ok((solution, phases, nodes)) => {
            println!("{}", format_moves(&solution.moves));
            eprintln!(
                "phases={} {} {} {} nodes={} elapsed_ms={:.3}",
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
                "phases=0 0 0 0 nodes=0 elapsed_ms={:.3}",
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
