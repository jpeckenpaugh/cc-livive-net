//! Gates 2–7: invariants, identities, commutation, and validity rejection.

use color_cube::{Color, Cube, CubeError, Move};

/// Parses a short move list for fixtures: `"R U R' U'"`.
fn moves(text: &str) -> Vec<Move> {
    text.split_whitespace()
        .map(|token| {
            let face = match token.as_bytes()[0] {
                b'U' => 0,
                b'R' => 1,
                b'F' => 2,
                b'D' => 3,
                b'L' => 4,
                b'B' => 5,
                other => panic!("bad fixture face {other}"),
            };
            let power = if token.ends_with('2') {
                1
            } else if token.ends_with('\'') {
                2
            } else {
                0
            };
            Move(face * 3 + power)
        })
        .collect()
}

fn scrambled() -> Cube {
    let mut cube = Cube::SOLVED;
    cube.apply_all(&moves("R U2 F' L D B2 R' U F2 D' L2 B U2 R'"));
    cube
}

#[test]
fn moves_preserve_invariants() {
    for move_index in 0..18 {
        for base in [Cube::SOLVED, scrambled()] {
            let mut cube = base;
            cube.apply(Move(move_index));
            assert!(cube.is_valid(), "move {move_index} produced invalid state");
        }
    }
}

#[test]
fn quarter_turns_have_order_four_and_inverses_cancel() {
    let base = scrambled();
    for move_index in 0..18 {
        let mv = Move(move_index);

        let mut fourth = base;
        for _ in 0..4 {
            fourth.apply(mv);
        }
        assert_eq!(fourth, base, "move {move_index} is not order 4");

        let mut cancelled = base;
        cancelled.apply(mv);
        cancelled.apply_inverse(mv);
        assert_eq!(cancelled, base, "move {move_index} did not cancel");
    }
}

#[test]
fn sexy_move_sequences_have_order_six() {
    for sequence in ["R U R' U'", "U R U' R'"] {
        let mut cube = Cube::SOLVED;
        let sequence = moves(sequence);
        for _ in 0..6 {
            cube.apply_all(&sequence);
        }
        assert_eq!(cube, Cube::SOLVED, "{sequence:?} does not have order 6");
    }
}

#[test]
fn opposite_faces_commute() {
    let base = scrambled();
    for face in 0..3 {
        for first_power in 0..3 {
            for second_power in 0..3 {
                let first = Move(face * 3 + first_power);
                let opposite = Move((face + 3) * 3 + second_power);

                let mut one_way = base;
                one_way.apply(first);
                one_way.apply(opposite);

                let mut other_way = base;
                other_way.apply(opposite);
                other_way.apply(first);

                assert_eq!(
                    one_way,
                    other_way,
                    "faces {face} and {} do not commute",
                    face + 3
                );
            }
        }
    }
}

#[test]
fn solved_state_is_solved_valid_and_monochrome() {
    let cube = Cube::SOLVED;
    assert!(cube.is_solved());
    assert!(cube.is_valid());

    // Face order `U R F D L B`; colour discriminants are D009 order.
    let face_colors = [Color::U, Color::R, Color::F, Color::D, Color::L, Color::B];
    let facelets = cube.facelets();
    for (face, &color) in face_colors.iter().enumerate() {
        for offset in 0..9 {
            assert_eq!(
                facelets[face * 9 + offset],
                color,
                "face {face} offset {offset}"
            );
        }
    }
}

#[test]
fn malformed_colour_counts_are_rejected() {
    let mut facelets = Cube::SOLVED.facelets();
    facelets[0] = Color::D;
    assert_eq!(
        Cube::from_facelets(&facelets).unwrap_err(),
        CubeError::MalformedFacelets
    );
}

#[test]
fn wrong_centres_are_rejected() {
    // Swapping two centres keeps nine stickers of each colour, so this must
    // reach the centre check rather than the count check.
    let mut facelets = Cube::SOLVED.facelets();
    facelets[4] = Color::D;
    facelets[31] = Color::U;
    assert_eq!(
        Cube::from_facelets(&facelets).unwrap_err(),
        CubeError::InvalidCenter
    );
}

#[test]
fn duplicate_piece_is_rejected() {
    // Swapping a U sticker with an L sticker keeps the colour counts but
    // destroys two corner pieces.
    let mut facelets = Cube::SOLVED.facelets();
    facelets[8] = Color::L;
    facelets[36] = Color::U;
    assert_eq!(
        Cube::from_facelets(&facelets).unwrap_err(),
        CubeError::InvalidPermutation
    );
}

#[test]
fn missing_piece_is_rejected() {
    let mut facelets = Cube::SOLVED.facelets();
    facelets[9] = Color::F;
    facelets[18] = Color::R;
    assert_eq!(
        Cube::from_facelets(&facelets).unwrap_err(),
        CubeError::InvalidPermutation
    );
}

#[test]
fn twist_violation_is_rejected() {
    // Rotating one corner in place keeps every piece but breaks the twist sum.
    let mut facelets = Cube::SOLVED.facelets();
    facelets[8] = Color::R;
    facelets[9] = Color::U;
    assert_eq!(
        Cube::from_facelets(&facelets).unwrap_err(),
        CubeError::Twist
    );
}

#[test]
fn flip_violation_is_rejected() {
    // Flipping one edge in place breaks the flip sum.
    let mut facelets = Cube::SOLVED.facelets();
    facelets[5] = Color::R;
    facelets[10] = Color::U;
    assert_eq!(Cube::from_facelets(&facelets).unwrap_err(), CubeError::Flip);
}

#[test]
fn parity_violation_is_rejected() {
    // A single corner transposition has odd corner parity against even edge
    // parity. `facelets` is defined for such a state, which is how the
    // fixture is built.
    let mut cube = Cube::SOLVED;
    cube.cp.swap(0, 1);
    assert!(!cube.is_valid());
    assert_eq!(
        Cube::from_facelets(&cube.facelets()).unwrap_err(),
        CubeError::Parity
    );
}

#[test]
fn superflip_generator_flips_every_edge() {
    // `concept.md` §12's golden position: all 12 edges flipped, all corners
    // solved. This pins the edge/corner orientation sign independently of the
    // reference tables.
    let mut cube = Cube::SOLVED;
    cube.apply_all(&moves("U R2 F B R B2 R U2 L B2 R U' D' R2 F R' L B2 U2 F2"));

    assert_eq!(cube.cp, [0, 1, 2, 3, 4, 5, 6, 7], "corners moved");
    assert_eq!(cube.co, [0; 8], "corners twisted");
    assert_eq!(
        cube.ep,
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        "edges moved"
    );
    assert_eq!(cube.eo, [1; 12], "edges not all flipped");
    assert!(cube.is_valid());
    assert!(!cube.is_solved());
}

#[test]
fn out_of_range_entries_are_invalid_but_do_not_panic() {
    let mut cube = Cube::SOLVED;
    cube.cp[0] = 8;
    assert!(!cube.is_valid());

    let mut cube = Cube::SOLVED;
    cube.eo[3] = 2;
    assert!(!cube.is_valid());

    let mut cube = Cube::SOLVED;
    cube.ep[0] = 200;
    assert!(!cube.is_valid());
}
