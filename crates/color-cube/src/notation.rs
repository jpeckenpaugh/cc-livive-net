//! Allocation-free Singmaster notation formatting and parsing.
//!
//! Formatting is all-or-nothing (D016): a short buffer produces
//! [`NotationError::BufferTooSmall`] and is not modified. Tokens are separated
//! by single spaces with no trailing space.
//!
//! Parsing is uppercase-only and splits on ASCII whitespace. Empty or
//! whitespace-only input is an empty list; any unrecognised token is an error,
//! never a skipped token.

use crate::{Move, N_MOVES};

/// Maximum number of moves [`parse_moves`] can return.
pub const MAX_PARSED_MOVES: usize = 64;

/// A fixed-capacity list of parsed moves.
///
/// The fields are private so `len` can never disagree with the buffer. Use
/// [`FixedMoves::as_slice`] to read the moves.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FixedMoves {
    moves: [Move; MAX_PARSED_MOVES],
    len: u8,
}

impl FixedMoves {
    /// The parsed moves, at most [`MAX_PARSED_MOVES`] long.
    #[must_use]
    pub fn as_slice(&self) -> &[Move] {
        &self.moves[..self.len as usize]
    }

    /// The number of parsed moves.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Whether no moves were parsed.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Why a move list could not be written as Singmaster notation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NotationError {
    /// The destination slice is too small for the complete output.
    ///
    /// Nothing was written when this is returned.
    BufferTooSmall,
}

impl core::fmt::Display for NotationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BufferTooSmall => f.write_str("destination buffer is too small for notation"),
        }
    }
}

/// Why a string is not a valid whitespace-separated move list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseError {
    /// A token was not one of the 18 Singmaster move names.
    InvalidToken,
    /// More than [`MAX_PARSED_MOVES`] tokens were present.
    TooManyMoves,
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidToken => f.write_str("invalid Singmaster move token"),
            Self::TooManyMoves => f.write_str("too many moves to fit in FixedMoves"),
        }
    }
}

/// Writes moves as single-space-separated Singmaster notation into `out`.
///
/// An empty slice writes nothing and returns `Ok(0)`. On a short buffer the
/// output is left untouched and [`NotationError::BufferTooSmall`] is returned.
///
/// # Errors
///
/// Returns [`NotationError::BufferTooSmall`] when `out` cannot hold the full
/// notation.
///
/// # Panics
///
/// Panics if any move index is outside `0..18` (D010).
pub fn write_singmaster(moves: &[Move], out: &mut [u8]) -> Result<usize, NotationError> {
    let mut required = 0usize;
    for (index, &mv) in moves.iter().enumerate() {
        required += mv.to_singmaster().len();
        if index + 1 < moves.len() {
            required += 1;
        }
    }
    if out.len() < required {
        return Err(NotationError::BufferTooSmall);
    }

    let mut position = 0;
    for (index, &mv) in moves.iter().enumerate() {
        for &byte in mv.to_singmaster().as_bytes() {
            out[position] = byte;
            position += 1;
        }
        if index + 1 < moves.len() {
            out[position] = b' ';
            position += 1;
        }
    }
    Ok(required)
}

/// Parses a whitespace-separated Singmaster move list.
///
/// Uppercase tokens only. Empty or whitespace-only input yields an empty
/// [`FixedMoves`]. At most [`MAX_PARSED_MOVES`] tokens are accepted; a 65th
/// token is [`ParseError::TooManyMoves`] rather than a panic.
///
/// # Errors
///
/// Returns [`ParseError::InvalidToken`] for an unrecognised token and
/// [`ParseError::TooManyMoves`] when the list exceeds the fixed capacity.
pub fn parse_moves(input: &str) -> Result<FixedMoves, ParseError> {
    let mut moves = [Move(0); MAX_PARSED_MOVES];
    let mut len = 0usize;

    for token in input.split_ascii_whitespace() {
        if len == MAX_PARSED_MOVES {
            return Err(ParseError::TooManyMoves);
        }
        let mv = parse_token(token).ok_or(ParseError::InvalidToken)?;
        moves[len] = mv;
        len += 1;
    }

    Ok(FixedMoves {
        moves,
        len: len as u8,
    })
}

/// Parses exactly one move token.
fn parse_token(token: &str) -> Option<Move> {
    let mv = match token {
        "U" => 0,
        "U2" => 1,
        "U'" => 2,
        "R" => 3,
        "R2" => 4,
        "R'" => 5,
        "F" => 6,
        "F2" => 7,
        "F'" => 8,
        "D" => 9,
        "D2" => 10,
        "D'" => 11,
        "L" => 12,
        "L2" => 13,
        "L'" => 14,
        "B" => 15,
        "B2" => 16,
        "B'" => 17,
        _ => return None,
    };
    debug_assert!((mv as usize) < N_MOVES);
    Some(Move(mv))
}
