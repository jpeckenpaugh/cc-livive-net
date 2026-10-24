//! CLI smoke tests. The binary is run as a child process with a generated
//! table directory so malformed input and stdout/stderr stay honest.

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use color_cube::{parse_moves, Cube};
use color_cube_solver::{Kociemba, Thistlethwaite};

fn directory() -> &'static PathBuf {
    static DIRECTORY: OnceLock<PathBuf> = OnceLock::new();
    DIRECTORY.get_or_init(|| {
        let path = std::env::temp_dir().join(format!("cc-cli-tables-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        Thistlethwaite::generate(&path).expect("generate CLI tables");
        Kociemba::generate(&path).expect("generate CLI kociemba tables");
        path
    })
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_color-cube-cli"))
        .args(args)
        .env("COLOR_CUBE_TABLES", directory())
        .output()
        .expect("run color-cube-cli")
}

#[test]
fn solve_seed_prints_a_replayable_solution() {
    let output = run(&["solve", "--seed", "42"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let parsed = parse_moves(stdout.trim()).expect("parse solution");
    let mut cube = Cube::from_seed(42);
    cube.apply_all(parsed.as_slice());
    assert!(cube.is_solved());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("phases="), "stderr: {stderr}");
}

#[test]
fn solve_seed_with_kociemba_replays_and_prints_phases() {
    let output = run(&["solve", "--seed", "42", "--solver", "kociemba"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let parsed = parse_moves(stdout.trim()).expect("parse solution");
    let mut cube = Cube::from_seed(42);
    cube.apply_all(parsed.as_slice());
    assert!(cube.is_solved());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("solver=kociemba"), "stderr: {stderr}");
    assert!(stderr.contains("phases="), "stderr: {stderr}");
    assert!(stderr.contains("total="), "stderr: {stderr}");
}

#[test]
fn malformed_moves_and_timeout_exit_non_zero() {
    let bad = run(&["solve", "--moves", "U Q"]);
    assert!(!bad.status.success());
    assert!(!String::from_utf8_lossy(&bad.stderr).is_empty());

    let timeout = run(&["solve", "--seed", "42", "--time-ms", "0"]);
    assert!(!timeout.status.success());
    assert!(String::from_utf8_lossy(&timeout.stderr).contains("timed out"));
}

#[test]
fn already_solved_prints_empty_line_and_exits_zero() {
    let output = run(&["solve", "--moves", ""]);
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "\n");
}

#[test]
fn scramble_and_check_work() {
    let scramble = run(&["scramble", "--seed", "7"]);
    assert!(scramble.status.success());
    let text = String::from_utf8_lossy(&scramble.stdout);
    let parsed = parse_moves(text.trim()).expect("scramble parses");
    assert!(!parsed.is_empty());

    let check = run(&["check", "--seed", "7"]);
    assert!(check.status.success());
    let text = String::from_utf8_lossy(&check.stdout);
    assert!(text.contains("valid=true"));
    assert!(text.contains("hash="));
}
