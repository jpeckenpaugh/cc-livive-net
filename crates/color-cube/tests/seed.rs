//! Gate 1 and gate 2: seeded states are valid, unsolved and stable.
//!
//! The fixture was generated from the frozen 36-draw contract (D015) and must
//! only change with an explicit ADR and a re-baseline.

use color_cube::Cube;

const GOLDEN: &str = include_str!("fixtures/from_seed_fnv1a64.txt");
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv1a_facelets(cube: &Cube) -> u64 {
    let mut hash = FNV_OFFSET;
    for color in cube.facelets() {
        hash ^= u64::from(color.index() as u8);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn parity<const N: usize>(values: &[u8; N]) -> u8 {
    let mut inversions = 0u32;
    for (i, &a) in values.iter().enumerate() {
        for &b in &values[i + 1..] {
            if a > b {
                inversions += 1;
            }
        }
    }
    (inversions % 2) as u8
}

fn golden_rows() -> Vec<(u64, u64)> {
    GOLDEN
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut fields = line.split_ascii_whitespace();
            let seed = fields.next().unwrap().parse::<u64>().unwrap();
            let hash = u64::from_str_radix(fields.next().unwrap(), 16).unwrap();
            assert!(fields.next().is_none(), "extra fixture field on {line:?}");
            (seed, hash)
        })
        .collect()
}

#[test]
fn ten_thousand_seeds_are_valid_and_unsolved() {
    for seed in 0..10_000u64 {
        let cube = Cube::from_seed(seed);
        assert!(cube.is_valid(), "seed {seed} is not valid");
        assert!(!cube.is_solved(), "seed {seed} is solved");

        let twist: u32 = cube.co.iter().map(|&value| u32::from(value)).sum();
        let flip: u32 = cube.eo.iter().map(|&value| u32::from(value)).sum();
        assert_eq!(twist % 3, 0, "seed {seed} has a twist violation");
        assert_eq!(flip % 2, 0, "seed {seed} has a flip violation");
        assert_eq!(
            parity(&cube.cp),
            parity(&cube.ep),
            "seed {seed} has mismatched parity"
        );
    }
}

#[test]
fn same_seed_is_deterministic() {
    for seed in [0, 1, 42, u64::MAX, 0x0123_4567_89ab_cdef] {
        assert_eq!(Cube::from_seed(seed), Cube::from_seed(seed));
    }
}

#[test]
fn golden_hashes_match() {
    let rows = golden_rows();
    assert_eq!(rows.len(), 32, "fixture must pin exactly 32 seeds");
    for (seed, expected) in rows {
        assert_eq!(
            fnv1a_facelets(&Cube::from_seed(seed)),
            expected,
            "golden hash mismatch for seed {seed}"
        );
    }
}
