//! Gates 4 and 5: observation encoding and action masks.

use color_cube::{Cube, Move, N_MOVES, N_OBSERVATIONS};

/// Colour index of each face in face order `U R F D L B` (D009).
const FACE_COLOR: [usize; 6] = [0, 4, 2, 1, 5, 3];

#[test]
fn observation_is_one_hot_per_facelet() {
    for seed in 0..1_000u64 {
        let mut observation = [f32::NAN; N_OBSERVATIONS];
        Cube::from_seed(seed).write_observation(&mut observation);

        let mut ones = 0usize;
        for facelet in 0..54 {
            let mut facelet_ones = 0usize;
            for color in 0..6 {
                let value = observation[facelet * 6 + color];
                assert!(value == 0.0 || value == 1.0, "seed {seed} is not one-hot");
                if value == 1.0 {
                    facelet_ones += 1;
                }
            }
            assert_eq!(facelet_ones, 1, "seed {seed}, facelet {facelet}");
            ones += facelet_ones;
        }
        assert_eq!(ones, 54);
    }
}

#[test]
fn solved_observation_matches_monochrome_pattern() {
    let mut observation = [f32::NAN; N_OBSERVATIONS];
    Cube::SOLVED.write_observation(&mut observation);

    for (face, &face_color) in FACE_COLOR.iter().enumerate() {
        for offset in 0..9 {
            let facelet = face * 9 + offset;
            for color in 0..6 {
                let expected = if color == face_color { 1.0 } else { 0.0 };
                assert_eq!(observation[facelet * 6 + color], expected);
            }
        }
    }

    // Hand-checked facelets: U1 is U, R1 is R, F3 is F.
    assert_eq!(observation[0], 1.0);
    assert_eq!(observation[9 * 6 + 4], 1.0);
    assert_eq!(observation[20 * 6 + 2], 1.0);
}

#[test]
fn default_action_mask_is_all_ones() {
    let mut mask = [0.0f32; N_MOVES];
    Cube::SOLVED.write_action_mask(&mut mask);
    assert!(mask.iter().all(|&value| value == 1.0));
}

#[test]
fn canonical_mask_matches_the_pruning_rule() {
    let mut mask = [f32::NAN; N_MOVES];

    Cube::SOLVED.write_canonical_mask(None, &mut mask);
    assert!(mask.iter().all(|&value| value == 1.0));

    for last in Move::ALL {
        Cube::SOLVED.write_canonical_mask(Some(last), &mut mask);
        let previous = last.face();
        let expected_zeros = match previous {
            0..=2 => 3,
            3..=5 => 6,
            _ => unreachable!(),
        };
        assert_eq!(
            mask.iter().filter(|&&value| value == 0.0).count(),
            expected_zeros,
            "last = {last:?}"
        );

        for (index, &value) in mask.iter().enumerate() {
            let candidate = Move(index as u8);
            let next = candidate.face();
            let allowed = previous != next && !(previous.abs_diff(next) == 3 && next < previous);
            assert_eq!(
                value == 1.0,
                allowed,
                "last {last:?}, candidate {candidate:?}"
            );
        }
    }

    // D014 spot checks: keep the higher face of an opposite pair.
    Cube::SOLVED.write_canonical_mask(Some(Move(0)), &mut mask);
    assert_eq!(mask[9], 1.0, "D after U must be allowed");
    assert_eq!(mask[12], 1.0, "L after U must be allowed");
    Cube::SOLVED.write_canonical_mask(Some(Move(9)), &mut mask);
    assert_eq!(mask[0], 0.0, "U after D must be pruned");
    Cube::SOLVED.write_canonical_mask(Some(Move(3)), &mut mask);
    assert_eq!(mask[12], 1.0, "L after R must be allowed");
    Cube::SOLVED.write_canonical_mask(Some(Move(12)), &mut mask);
    assert_eq!(mask[3], 0.0, "R after L must be pruned");
}
