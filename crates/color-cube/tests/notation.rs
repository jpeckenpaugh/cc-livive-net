//! Gates 6 and 7: Singmaster formatting, parsing and capacity errors.

use color_cube::{
    parse_moves, write_singmaster, Cube, FixedMoves, Move, NotationError, ParseError, N_MOVES,
};

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next_u64() % bound
    }
}

fn format(moves: &[Move]) -> String {
    let mut buffer = [0u8; 256];
    let len = write_singmaster(moves, &mut buffer).unwrap();
    core::str::from_utf8(&buffer[..len]).unwrap().to_owned()
}

#[test]
fn every_move_round_trips() {
    for mv in Move::ALL {
        let text = mv.to_singmaster();
        let parsed = parse_moves(text).unwrap();
        assert_eq!(parsed.as_slice(), &[mv]);
        let mut buffer = [0u8; 4];
        let len = write_singmaster(&[mv], &mut buffer).unwrap();
        assert_eq!(&buffer[..len], text.as_bytes());
    }
}

#[test]
fn random_lists_round_trip() {
    let mut rng = SplitMix64::new(0x5EED);
    for _ in 0..1_000 {
        let count = rng.below(65) as usize;
        let mut moves = [Move(0); 64];
        for slot in moves.iter_mut().take(count) {
            *slot = Move(rng.below(N_MOVES as u64) as u8);
        }
        let moves = &moves[..count];
        let text = format(moves);
        let parsed = parse_moves(&text).unwrap();
        assert_eq!(parsed.as_slice(), moves);
        assert_eq!(parsed.len(), count);
        assert_eq!(parsed.is_empty(), count == 0);
    }
}

#[test]
fn formatting_is_all_or_nothing() {
    let moves = [Move(0), Move(1), Move(2)];
    let expected = *b"U U2 U'";

    let mut exact = [0u8; 7];
    assert_eq!(write_singmaster(&moves, &mut exact), Ok(7));
    assert_eq!(exact, expected);

    let mut short = [0xAAu8; 6];
    assert_eq!(
        write_singmaster(&moves, &mut short),
        Err(NotationError::BufferTooSmall)
    );
    assert_eq!(short, [0xAA; 6], "short buffers must not be modified");

    let mut empty = [];
    assert_eq!(write_singmaster(&[], &mut empty), Ok(0));
    assert_eq!(format(&[]), "");
}

#[test]
fn parsing_rejects_malformed_tokens() {
    for token in ["U3", "X", "U2'", "U''", "u", "R3", "M", "U '", ""] {
        let result = parse_moves(token).map(|_| ());
        let expected = if token.is_empty() {
            Ok(())
        } else {
            Err(ParseError::InvalidToken)
        };
        assert_eq!(result, expected);
    }

    // Empty and whitespace-only inputs are empty lists, not errors.
    assert!(parse_moves("").unwrap().is_empty());
    assert!(parse_moves("   \t\n ").unwrap().is_empty());

    // Two valid tokens are two moves, not one malformed token.
    let parsed = parse_moves("U U'").unwrap();
    assert_eq!(parsed.as_slice(), &[Move(0), Move(2)]);
}

#[test]
fn parser_capacity_is_a_clean_boundary() {
    let sixty_four = "U ".repeat(63) + "U";
    let parsed = parse_moves(&sixty_four).unwrap();
    assert_eq!(parsed.len(), 64);

    let sixty_five = "U ".repeat(64) + "U";
    assert_eq!(parse_moves(&sixty_five), Err(ParseError::TooManyMoves));
}

#[test]
fn fixed_moves_accessors_agree() {
    // Compile-time shape check: the capacity constant and accessor agree.
    let parsed = parse_moves("R2").unwrap();
    assert_eq!(parsed.as_slice().len(), 1);
    let _: FixedMoves = parsed;
}

#[test]
fn solved_cube_notation_round_trips_through_a_sequence() {
    let moves = [Move(0), Move(4), Move(8), Move(9), Move(13), Move(17)];
    let text = format(&moves);
    assert_eq!(text, "U R2 F' D L2 B'");
    let parsed = parse_moves(&text).unwrap();
    let mut cube = Cube::SOLVED;
    cube.apply_all(parsed.as_slice());
    assert!(cube.is_valid());
}
