//! Coordinate and move-table support for `color-cube`, plus the deterministic
//! Thistlethwaite solver (D019–D021).
//!
//! M2b ends at the solver and CLI boundary; Kociemba and WASM integration are
//! later milestones.

#![forbid(unsafe_code)]

pub mod coords;
pub mod tables;
pub mod thistlethwaite;

pub use thistlethwaite::{
    DistanceBundle, DistanceReport, DistanceTiming, Limits, Solution, SolveError, Solver,
    TableLoadError, Thistlethwaite,
};
