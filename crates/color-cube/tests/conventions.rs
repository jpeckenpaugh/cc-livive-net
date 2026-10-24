//! Gate 8: the derived moves must match independent, hand-checked references.
//!
//! The reference tables below are the six basic quarter turns from Herbert
//! Kociemba's two-phase solver (`cubie.py`, values `cpU/coU/epU/eoU` etc.).
//! They are test fixtures only, used to catch a systematically mirrored or
//! mis-ordered derivation; `concept.md` §5 permits exactly this. The library
//! never sees them.

use color_cube::{Cube, Move};

struct Reference {
    cp: [u8; 8],
    co: [u8; 8],
    ep: [u8; 12],
    eo: [u8; 12],
}

/// Applies a reference transition to a cube, independently of the crate.
fn apply_reference(cube: &Cube, reference: &Reference) -> Cube {
    let mut out = Cube::SOLVED;
    for i in 0..8 {
        let source = reference.cp[i] as usize;
        out.cp[i] = cube.cp[source];
        out.co[i] = (cube.co[source] + reference.co[i]) % 3;
    }
    for i in 0..12 {
        let source = reference.ep[i] as usize;
        out.ep[i] = cube.ep[source];
        out.eo[i] = (cube.eo[source] + reference.eo[i]) % 2;
    }
    out
}

/// `U`, `R`, `F`, `D`, `L`, `B` in the crate's face order.
const REFERENCES: [Reference; 6] = [
    Reference {
        cp: [3, 0, 1, 2, 4, 5, 6, 7],
        co: [0; 8],
        ep: [3, 0, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11],
        eo: [0; 12],
    },
    Reference {
        cp: [4, 1, 2, 0, 7, 5, 6, 3],
        co: [2, 0, 0, 1, 1, 0, 0, 2],
        ep: [8, 1, 2, 3, 11, 5, 6, 7, 4, 9, 10, 0],
        eo: [0; 12],
    },
    Reference {
        cp: [1, 5, 2, 3, 0, 4, 6, 7],
        co: [1, 2, 0, 0, 2, 1, 0, 0],
        ep: [0, 9, 2, 3, 4, 8, 6, 7, 1, 5, 10, 11],
        eo: [0, 1, 0, 0, 0, 1, 0, 0, 1, 1, 0, 0],
    },
    Reference {
        cp: [0, 1, 2, 3, 5, 6, 7, 4],
        co: [0; 8],
        ep: [0, 1, 2, 3, 5, 6, 7, 4, 8, 9, 10, 11],
        eo: [0; 12],
    },
    Reference {
        cp: [0, 2, 6, 3, 4, 1, 5, 7],
        co: [0, 1, 2, 0, 0, 2, 1, 0],
        ep: [0, 1, 10, 3, 4, 5, 9, 7, 8, 2, 6, 11],
        eo: [0; 12],
    },
    Reference {
        cp: [0, 1, 3, 7, 4, 5, 2, 6],
        co: [0, 0, 1, 2, 0, 0, 2, 1],
        ep: [0, 1, 2, 11, 4, 5, 6, 10, 8, 9, 3, 7],
        eo: [0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 1],
    },
];

#[test]
fn quarter_turns_match_reference_tables() {
    for (face, reference) in REFERENCES.iter().enumerate() {
        let mut actual = Cube::SOLVED;
        actual.apply(Move(face as u8 * 3));
        let expected = apply_reference(&Cube::SOLVED, reference);
        assert_eq!(actual, expected, "face {face} quarter turn");
    }
}

#[test]
fn half_and_inverse_turns_match_composed_references() {
    for (face, reference) in REFERENCES.iter().enumerate() {
        let mut expected = Cube::SOLVED;
        for power in 1..=3 {
            expected = apply_reference(&expected, reference);
            let mut actual = Cube::SOLVED;
            actual.apply(Move((face * 3 + power - 1) as u8));
            assert_eq!(actual, expected, "face {face} power {power}");
        }
    }
}

#[test]
fn u_moves_urf_piece_to_ufl() {
    // Gate 8's hand check. Clockwise U sends the URF piece to UFL; the piece
    // arriving at URF is UBR (`cpU = [UBR, URF, UFL, ULB, ...]`).
    let mut cube = Cube::SOLVED;
    cube.apply(Move(0));

    assert_eq!(cube.cp, [3, 0, 1, 2, 4, 5, 6, 7]);
    assert_eq!(cube.ep, [3, 0, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11]);
    assert_eq!(cube.co, [0; 8]);
    assert_eq!(cube.eo, [0; 12]);

    // The URF piece (index 0) sits at position 1, i.e. UFL.
    assert_eq!(cube.cp.iter().position(|&piece| piece == 0), Some(1));

    // `URF -> UBR` is `U'`, not `U`.
    let mut inverse = Cube::SOLVED;
    inverse.apply(Move(2));
    assert_eq!(inverse.cp.iter().position(|&piece| piece == 0), Some(3));
}
