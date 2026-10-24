//! Kociemba solver tests: table coverage, uniform cubes, determinism, goldens,
//! limits, corruption and pruning completeness.
//!
//! The 1,000-cube and 100-cube gates run in release builds (`cfg!(debug_assertions)`
//! is false). Debug builds use small samples so `cargo test --all` stays
//! practical with the split heuristics.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

use color_cube::{parse_moves, Cube, Move};
use color_cube_solver::coords::{self, SLICE_SOLVED};
use color_cube_solver::kociemba::{
    self, Kociemba, KociembaDistanceBundle, KociembaDistanceReport, KociembaOptions,
};
use color_cube_solver::tables;
use color_cube_solver::{Limits, SolveError, Solver};

fn solver_directory() -> PathBuf {
    std::env::temp_dir().join(format!("cc-kociemba-{}", std::process::id()))
}

static REPORT: OnceLock<KociembaDistanceReport> = OnceLock::new();

fn temp_directory(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("cc-kociemba-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    path
}

fn solver() -> &'static Kociemba {
    static SOLVER: OnceLock<Kociemba> = OnceLock::new();
    SOLVER.get_or_init(|| {
        let directory = solver_directory();
        let _ = std::fs::remove_dir_all(&directory);
        let started = Instant::now();
        let moves = tables::generate();
        let (distances, timings, report) = kociemba::generate_distance_tables(&moves);
        eprintln!(
            "generated move+Kociemba tables in {:.3} s (debug={})",
            started.elapsed().as_secs_f64(),
            cfg!(debug_assertions)
        );
        for timing in &timings {
            eprintln!(
                "  id {} took {:.3} ms",
                timing.id,
                timing.elapsed.as_secs_f64() * 1000.0
            );
        }
        eprintln!("kociemba report: {report}");
        assert!(
            report.fully_covered(),
            "all five coordinate spaces must be covered"
        );
        assert_eq!(report.phase1_twist_slice_max, 9);
        assert_eq!(report.phase1_flip_slice_max, 9);
        assert_eq!(report.phase2_cp_sp_max, 14);
        assert_eq!(report.phase2_ep8_sp_max, 12);
        assert_eq!(report.phase1_twist_flip_max, 9);
        let _ = REPORT.set(report);
        let sizes = distances.payload_sizes();
        assert_eq!(
            sizes,
            [
                kociemba::PHASE1_TWIST_SLICE_ROWS,
                kociemba::PHASE1_FLIP_SLICE_ROWS,
                kociemba::PHASE2_CP_SP_ROWS,
                kociemba::PHASE2_EP8_SP_ROWS,
                kociemba::PHASE1_TWIST_FLIP_ROWS,
            ]
        );
        tables::write(&directory, &moves).expect("write move tables");
        distances.write(&directory).expect("write kociemba tables");
        let reloaded = KociembaDistanceBundle::load(&directory).expect("reload kociemba tables");
        assert_eq!(reloaded, distances);
        Kociemba::from_directory(&directory)
    })
}

fn sample_size(release: usize, debug: usize) -> usize {
    if cfg!(debug_assertions) {
        debug
    } else {
        release
    }
}

fn check_solution(cube: &Cube, moves: &[Move], phases: Option<[usize; 2]>) -> [usize; 2] {
    assert!(!moves.is_empty());
    assert!(moves.len() <= 30, "solution length {}", moves.len());
    let phases = phases.expect("kociemba reports a phase split");
    assert!(phases[0] <= 12, "phase 1 length {}", phases[0]);
    assert!(phases[1] <= 18, "phase 2 length {}", phases[1]);
    assert_eq!(phases[0] + phases[1], moves.len());
    let mut current = *cube;
    current.apply_all(&moves[..phases[0]]);
    assert_eq!(coords::twist(&current), 0);
    assert_eq!(coords::flip(&current), 0);
    assert_eq!(coords::slice(&current), SLICE_SOLVED);
    assert!(coords::is_g1(&current));
    current.apply_all(&moves[phases[0]..]);
    assert!(current.is_solved());
    phases
}

fn aggregate(count: usize, stats: bool) -> (usize, u64) {
    let solver = solver();
    let mut lengths = Vec::with_capacity(count);
    let mut durations = Vec::with_capacity(count);
    let mut phase1_nodes = 0u64;
    let mut phase2_calls = 0u64;
    let mut phase2_nodes = 0u64;
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for index in 0..count {
        let cube = Cube::from_seed(index as u64);
        let started = Instant::now();
        let (solution, search) = solver
            .solve_with_stats(&cube, Limits::default())
            .unwrap_or_else(|error| panic!("seed {index}: {error}"));
        durations.push(started.elapsed());
        let phases = check_solution(&cube, &solution.moves, solution.phases);
        lengths.push(solution.moves.len());
        phase1_nodes += search.phase1_nodes;
        phase2_calls += search.phase2_calls;
        phase2_nodes += search.phase2_nodes;
        for &mv in &solution.moves {
            hash ^= u64::from(mv.0);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash ^= phases[0] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    if stats {
        let mut histogram = std::collections::BTreeMap::new();
        for &len in &lengths {
            *histogram.entry(len).or_insert(0usize) += 1;
        }
        durations.sort_unstable();
        let percentile = |p: usize| {
            let index = (durations.len() - 1) * p / 100;
            durations[index].as_secs_f64() * 1000.0
        };
        let total: usize = lengths.iter().sum();
        eprintln!(
            "kociemba {} cubes: avg={:.2} min={} max={} histogram={:?}",
            count,
            total as f64 / count as f64,
            lengths.iter().min().unwrap(),
            lengths.iter().max().unwrap(),
            histogram
        );
        eprintln!(
            "solve time: p50={:.3} ms p95={:.3} ms max={:.3} ms; \
             phase1_nodes={} phase2_calls={} phase2_nodes={}",
            percentile(50),
            percentile(95),
            durations[durations.len() - 1].as_secs_f64() * 1000.0,
            phase1_nodes,
            phase2_calls,
            phase2_nodes
        );
    }
    (lengths.iter().sum(), hash)
}

#[test]
fn tables_cover_their_space_with_expected_diameters() {
    let _ = solver();
    let report = REPORT.get().expect("report");
    assert!(report.fully_covered());
    assert_eq!(report.phase1_twist_slice_max, 9);
    assert_eq!(report.phase1_flip_slice_max, 9);
    assert_eq!(report.phase2_cp_sp_max, 14);
    assert_eq!(report.phase2_ep8_sp_max, 12);
    assert_eq!(report.phase1_twist_flip_max, 9);
}

#[test]
fn uniform_cubes_are_solved_bounded_and_deterministic() {
    let count = sample_size(1_000, 4);
    let first = aggregate(count, true);
    let second = aggregate(count, false);
    assert_eq!(first, second, "aggregate changed between runs");
}

#[test]
fn same_cube_twice_is_deterministic() {
    let solver = solver();
    for seed in 0..sample_size(32, 2) as u64 {
        let cube = Cube::from_seed(seed);
        let a = solver.solve(&cube, Limits::default()).expect("first solve");
        let b = solver
            .solve(&cube, Limits::default())
            .expect("second solve");
        assert_eq!(a, b, "seed {seed}");
        assert!(!a.optimal);
    }
}

#[test]
fn golden_fixtures_are_stable() {
    let solver = solver();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("kociemba_golden.txt");
    let text = std::fs::read_to_string(&path).expect("golden fixture");
    let mut count = 0;
    let limit = sample_size(16, 4);
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if count == limit {
            break;
        }
        let (seed, expected) = line.split_once('\t').expect("seed<TAB>solution");
        let seed: u64 = seed.parse().expect("seed");
        let solution = solver
            .solve(&Cube::from_seed(seed), Limits::default())
            .expect("golden solve");
        let actual = solution
            .moves
            .iter()
            .map(|m| m.to_singmaster())
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(actual, expected, "seed {seed}");
        count += 1;
    }
    assert_eq!(count, limit, "expected {limit} golden fixtures");
}

#[test]
fn error_cases_and_limits_are_explicit() {
    let solver = solver();
    assert_eq!(
        solver.solve(&Cube::SOLVED, Limits::default()),
        Err(SolveError::AlreadySolved)
    );

    let mut invalid = Cube::SOLVED;
    invalid.cp[1] = 0;
    assert!(!invalid.is_valid());
    assert_eq!(
        solver.solve(&invalid, Limits::default()),
        Err(SolveError::InvalidState)
    );

    assert_eq!(
        solver.solve(
            &Cube::from_seed(7),
            Limits {
                max_length: None,
                time_ms: Some(0),
            }
        ),
        Err(SolveError::Timeout)
    );

    let cube = Cube::from_seed(7);
    assert_eq!(
        solver.solve(
            &cube,
            Limits {
                max_length: Some(5),
                time_ms: None,
            }
        ),
        Err(SolveError::NoSolutionWithinLimits)
    );

    // A tiny nonzero budget either returns a complete solution or times out;
    // it must never panic or return an invalid list.
    match solver.solve(
        &cube,
        Limits {
            max_length: None,
            time_ms: Some(1),
        },
    ) {
        Ok(solution) => {
            let mut replay = cube;
            replay.apply_all(&solution.moves);
            assert!(replay.is_solved());
        }
        Err(SolveError::Timeout | SolveError::NoSolutionWithinLimits) => {}
        Err(other) => panic!("unexpected error: {other}"),
    }
}

#[test]
fn missing_and_corrupt_tables_map_to_tables_missing() {
    let _ = solver();
    let source = solver_directory();
    let missing = temp_directory("missing");
    let solver = Kociemba::from_directory(&missing);
    assert_eq!(
        solver.solve(&Cube::from_seed(1), Limits::default()),
        Err(SolveError::TablesMissing)
    );
    assert!(solver.load_error().is_some());

    let directory = temp_directory("corrupt");
    std::fs::create_dir_all(&directory).expect("create corrupt dir");
    std::fs::copy(
        source.join(tables::BUNDLE_FILE),
        directory.join(tables::BUNDLE_FILE),
    )
    .expect("copy move tables");
    std::fs::copy(
        source.join(kociemba::KOCIEMBA_BUNDLE_FILE),
        directory.join(kociemba::KOCIEMBA_BUNDLE_FILE),
    )
    .expect("copy kociemba tables");
    let path = directory.join(kociemba::KOCIEMBA_BUNDLE_FILE);
    let mut bytes = std::fs::read(&path).expect("read bundle");
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    std::fs::write(&path, bytes).expect("write corrupt bundle");
    let corrupt = Kociemba::from_directory(&directory);
    assert_eq!(
        corrupt.solve(&Cube::from_seed(1), Limits::default()),
        Err(SolveError::TablesMissing)
    );
    let error = corrupt.load_error().expect("loader error");
    assert_eq!(error.error, tables::TablesError::TableCorrupt);
}

#[test]
fn canonical_pruning_preserves_length_distribution() {
    let solver = solver();
    let count = sample_size(100, 2);
    for seed in 0..count as u64 {
        let cube = Cube::from_seed(seed);
        let (with_pruning, _) = solver
            .solve_with_stats(&cube, Limits::default())
            .expect("canonical solve");
        let (without_pruning, _) = solver
            .solve_with_options(
                &cube,
                Limits::default(),
                KociembaOptions {
                    canonical_pruning: false,
                },
            )
            .expect("unpruned solve");
        assert_eq!(
            with_pruning.moves.len(),
            without_pruning.moves.len(),
            "seed {seed}"
        );
        let mut replay = cube;
        replay.apply_all(&without_pruning.moves);
        assert!(replay.is_solved());
    }
}

#[test]
fn cli_style_moves_round_trip() {
    let solver = solver();
    let text = "R U R' U'";
    let moves = parse_moves(text).expect("parse");
    let mut cube = Cube::SOLVED;
    cube.apply_all(moves.as_slice());
    let solution = solver.solve(&cube, Limits::default()).expect("solve");
    let parsed = parse_moves(
        &solution
            .moves
            .iter()
            .map(|m| m.to_singmaster())
            .collect::<Vec<_>>()
            .join(" "),
    )
    .expect("parse solution");
    let mut replay = cube;
    replay.apply_all(parsed.as_slice());
    assert!(replay.is_solved());
}
