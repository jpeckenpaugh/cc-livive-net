//! Thistlethwaite solver tests: uniform cubes, determinism, goldens, errors,
//! random walks and the exhaustive G3 diameter check.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

use color_cube::{Cube, Move};
use color_cube_solver::coords::{self, SLICE_SOLVED};
use color_cube_solver::thistlethwaite::{
    is_g3, verify_g3_diameter, Limits, SolveError, Solver, Thistlethwaite,
};

fn solver() -> &'static Thistlethwaite {
    static SOLVER: OnceLock<Thistlethwaite> = OnceLock::new();
    SOLVER.get_or_init(|| {
        let directory = solver_directory();
        let _ = std::fs::remove_dir_all(&directory);
        let started = Instant::now();
        let solver = Thistlethwaite::generate(&directory).expect("generate tables");
        eprintln!(
            "generated move+distance tables in {:.3} s (debug build)",
            started.elapsed().as_secs_f64()
        );
        solver
    })
}

fn temp_directory(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("cc-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    path
}

fn solver_directory() -> PathBuf {
    std::env::temp_dir().join(format!("cc-solver-{}", std::process::id()))
}

fn assert_subgroups(cube: &Cube, moves: &[Move], phases: [usize; 4]) {
    let mut current = *cube;
    let p0 = phases[0];
    let p1 = p0 + phases[1];
    let p2 = p1 + phases[2];
    current.apply_all(&moves[..p0]);
    assert_eq!(coords::flip(&current), 0, "phase 1 must reach G1");
    current.apply_all(&moves[p0..p1]);
    assert_eq!(coords::twist(&current), 0, "phase 2 must reach G2");
    assert_eq!(coords::slice(&current), SLICE_SOLVED);
    current.apply_all(&moves[p1..p2]);
    assert!(is_g3(&current), "phase 3 must reach G3");
    current.apply_all(&moves[p2..]);
    assert!(current.is_solved(), "phase 4 must solve");
}

fn aggregate(cubes: &[Cube], stats: bool) -> (usize, u64) {
    let solver = solver();
    let mut total = 0usize;
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut durations = Vec::with_capacity(cubes.len());
    let mut max_nodes = 0u64;
    let mut max_nodes_seed = 0u64;
    for (index, &cube) in cubes.iter().enumerate() {
        let started = Instant::now();
        let (solution, phases, nodes) = solver
            .solve_with_stats(&cube, Limits::default())
            .unwrap_or_else(|error| panic!("seed {index}: {error}"));
        durations.push(started.elapsed());
        assert!(!solution.moves.is_empty());
        assert!(solution.moves.len() <= 45, "seed {index} length");
        assert!(phases[0] <= 7 && phases[1] <= 10 && phases[2] <= 13 && phases[3] <= 15);
        assert_eq!(phases.iter().sum::<usize>(), solution.moves.len());
        assert_subgroups(&cube, &solution.moves, phases);
        if nodes > max_nodes {
            max_nodes = nodes;
            max_nodes_seed = index as u64;
        }
        total += solution.moves.len();
        for &mv in &solution.moves {
            hash ^= u64::from(mv.0);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash ^= u64::from(phases[0] as u8);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    if stats {
        durations.sort_unstable();
        let min = durations[0];
        let median = durations[durations.len() / 2];
        let max = durations[durations.len() - 1];
        eprintln!(
            "solve time: min={:.3} ms median={:.3} ms max={:.3} ms; max phase4 nodes={} (seed {})",
            min.as_secs_f64() * 1000.0,
            median.as_secs_f64() * 1000.0,
            max.as_secs_f64() * 1000.0,
            max_nodes,
            max_nodes_seed
        );
    }
    (total, hash)
}

#[test]
fn uniform_1000_are_solved_bounded_and_deterministic() {
    let cubes: Vec<Cube> = (0..1_000).map(Cube::from_seed).collect();
    let first = aggregate(&cubes, true);
    let second = aggregate(&cubes, false);
    assert_eq!(first, second, "1,000-cube aggregate changed");
}

#[test]
fn same_cube_twice_is_deterministic() {
    let solver = solver();
    let cube = Cube::from_seed(12_345);
    let a = solver.solve(&cube, Limits::default()).expect("first solve");
    let b = solver
        .solve(&cube, Limits::default())
        .expect("second solve");
    assert_eq!(a.moves, b.moves);
    assert_eq!(a.phases, b.phases);
    assert!(!a.optimal);
}

#[test]
fn golden_fixtures_are_stable() {
    let solver = solver();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("thistlethwaite_golden.txt");
    let text = std::fs::read_to_string(&path).expect("golden fixture");
    let mut count = 0;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
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
    assert_eq!(count, 16, "expected 16 golden fixtures");
}

#[test]
fn error_cases_are_explicit() {
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
                max_length: Some(1),
                time_ms: None,
            }
        ),
        Err(SolveError::NoSolutionWithinLimits)
    );
}

#[test]
fn random_walks_are_solved_and_bounded() {
    let solver = solver();
    for seed in 0..100u64 {
        let (cube, scramble) = Cube::from_scramble(seed, 25);
        assert!(!scramble.is_empty());
        let (solution, phases) = solver.solve_with_phases(&cube).expect("walk solve");
        assert!(solution.moves.len() <= 45);
        assert!(phases[0] <= 7 && phases[1] <= 10 && phases[2] <= 13 && phases[3] <= 15);
        assert_subgroups(&cube, &solution.moves, phases);
        let mut replay = cube;
        replay.apply_all(&solution.moves);
        assert!(replay.is_solved());
    }
}

#[test]
fn missing_and_corrupt_tables_map_to_tables_missing() {
    let missing = temp_directory("missing");
    let solver = Thistlethwaite::from_directory(&missing);
    assert_eq!(
        solver.solve(&Cube::from_seed(1), Limits::default()),
        Err(SolveError::TablesMissing)
    );
    assert!(solver.load_error().is_some());

    let directory = temp_directory("corrupt");
    let generated = Thistlethwaite::generate(&directory).expect("generate");
    assert!(generated.load_error().is_none());
    let path = directory.join("cc-thistlethwaite-v1.bin");
    let mut bytes = std::fs::read(&path).expect("read bundle");
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    std::fs::write(&path, bytes).expect("write corrupt bundle");
    let corrupt = Thistlethwaite::from_directory(&directory);
    assert_eq!(
        corrupt.solve(&Cube::from_seed(1), Limits::default()),
        Err(SolveError::TablesMissing)
    );
    let error = corrupt.load_error().expect("loader error");
    assert_eq!(
        error.error,
        color_cube_solver::tables::TablesError::TableCorrupt
    );
}

#[test]
fn phase3_coordinate_has_deterministic_transitions() {
    let _ = solver();
    // The coordinate is (cp_rank, edge_orbit_pattern). cp_rank transitions are
    // the move_cp table; the edge pattern is a function of ep8_rank, so two
    // cubes with the same coordinate have the same successor for every move.
    let tables = color_cube_solver::tables::load(&solver_directory()).expect("tables");
    let g2 = [0usize, 1, 2, 4, 7, 9, 10, 11, 13, 16];
    for seed in 0..200usize {
        let mut cube = Cube::SOLVED;
        for step in 0..20 {
            cube.apply(Move(g2[(seed + step) % g2.len()] as u8));
        }
        let cp = coords::cp_rank(&cube) as usize;
        let ep8 = coords::ep8_rank(&cube) as usize;
        for &m in &g2 {
            let next_cp = tables.move_cp[cp * 18 + m] as usize;
            let next_ep8 = tables.move_ep8[ep8 * 18 + m] as usize;
            let mut next_cube = cube;
            next_cube.apply(Move(m as u8));
            assert_eq!(next_cp, coords::cp_rank(&next_cube) as usize);
            assert_eq!(next_ep8, coords::ep8_rank(&next_cube) as usize);
        }
    }
}

#[test]
#[ignore = "exhaustive 663,552-state G3 BFS; run in release"]
fn g3_diameter_is_15() {
    let (states, diameter) = verify_g3_diameter();
    eprintln!("G3 reachable={states} diameter={diameter}");
    assert_eq!(states, 663_552);
    assert_eq!(diameter, 15);
}
