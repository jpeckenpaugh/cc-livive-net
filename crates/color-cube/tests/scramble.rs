//! Gate 3: random-walk scrambles are valid, unsolved and canonically pruned.

use color_cube::{Cube, Move, Scramble, MAX_SCRAMBLE_LEN};

fn opposite(a: u8, b: u8) -> bool {
    a.abs_diff(b) == 3
}

fn assert_canonical_pair(previous: Move, next: Move) {
    let (a, b) = (previous.face(), next.face());
    assert_ne!(a, b, "same-face pair {previous:?} then {next:?}");
    if opposite(a, b) {
        assert!(
            b > a,
            "non-canonical opposite pair {previous:?} then {next:?}"
        );
    }
}

#[test]
fn scrambles_are_valid_unsolved_and_pruned() {
    let turn_counts = [0u8, 1, 2, 5, 25, 30, 32, 33, 255];
    for seed in 0..1_000u64 {
        for &turns in &turn_counts {
            let (cube, scramble) = Cube::from_scramble(seed, turns);
            let expected_len = usize::from(turns).min(MAX_SCRAMBLE_LEN);
            assert_eq!(scramble.len as usize, expected_len, "seed {seed}, {turns}");
            assert_eq!(scramble.as_slice().len(), expected_len);
            assert_eq!(scramble.is_empty(), expected_len == 0);

            assert!(cube.is_valid(), "seed {seed}, turns {turns}");
            if expected_len > 0 {
                assert!(!cube.is_solved(), "seed {seed}, turns {turns}");
            } else {
                assert_eq!(cube, Cube::SOLVED);
            }

            for pair in scramble.as_slice().windows(2) {
                assert_canonical_pair(pair[0], pair[1]);
            }

            let mut replay = Cube::SOLVED;
            replay.apply_all(scramble.as_slice());
            assert_eq!(replay, cube, "seed {seed}, turns {turns}");
        }
    }
}

#[test]
fn scramble_capacity_is_clamped() {
    let (_, scramble) = Cube::from_scramble(7, 200);
    assert_eq!(scramble.len as usize, MAX_SCRAMBLE_LEN);
    assert_eq!(scramble.as_slice().len(), MAX_SCRAMBLE_LEN);
}

#[test]
fn scramble_is_deterministic() {
    for seed in [0, 1, 9, u64::MAX] {
        assert_eq!(Cube::from_scramble(seed, 25), Cube::from_scramble(seed, 25));
    }
}

#[test]
fn scramble_can_be_constructed_from_public_fields() {
    let scramble = Scramble {
        moves: [Move(0); MAX_SCRAMBLE_LEN],
        len: 2,
    };
    assert_eq!(scramble.as_slice().len(), 2);
    assert!(!scramble.is_empty());
}
