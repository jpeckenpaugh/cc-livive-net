//! Shared solver types (concept §9.5).
//!
//! `Limits`, `Solution`, `SolveError` and `Solver` used to live in the
//! Thistlethwaite module. They were extracted here so Kociemba can implement
//! the same trait without duplicating the public types. The old paths are kept
//! as re-exports, so this is an additive change.

use std::fmt;

use color_cube::{Cube, Move};

/// Stage limits for a solver.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Limits {
    /// Maximum combined solution length.
    pub max_length: Option<u8>,
    /// Wall-clock budget in milliseconds. `Some(0)` times out before search.
    pub time_ms: Option<u32>,
}

/// A solver result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Solution {
    /// The moves in application order.
    pub moves: Vec<Move>,
    /// Whether the solver claims optimality. Both v1 solvers always return
    /// `false`.
    pub optimal: bool,
    /// Two-phase split point; `None` for single-strategy solvers.
    pub phases: Option<[usize; 2]>,
}

/// Why a solve did not produce a solution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolveError {
    /// The input cube was already solved.
    AlreadySolved,
    /// The time budget expired before a complete solution was found.
    Timeout,
    /// The length limit was too small, or the internal search hit its bound.
    NoSolutionWithinLimits,
    /// The input cube violates a cube invariant.
    InvalidState,
    /// A table bundle is missing or corrupt.
    TablesMissing,
}

impl fmt::Display for SolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadySolved => f.write_str("cube is already solved"),
            Self::Timeout => f.write_str("solver timed out"),
            Self::NoSolutionWithinLimits => f.write_str("no solution within limits"),
            Self::InvalidState => f.write_str("cube state is invalid"),
            Self::TablesMissing => f.write_str("solver tables are missing or corrupt"),
        }
    }
}

impl std::error::Error for SolveError {}

/// A solver over [`Cube`].
pub trait Solver {
    /// Solves `cube` under `limits`.
    ///
    /// # Errors
    ///
    /// Returns the matching [`SolveError`].
    fn solve(&self, cube: &Cube, limits: Limits) -> Result<Solution, SolveError>;
}
