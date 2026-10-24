//! Coordinate and move-table support for `color-cube`, plus the deterministic
//! Thistlethwaite and Kociemba solvers.
//!
//! M2b ends at the solver and CLI boundary; Kociemba and WASM integration are
//! later milestones.

#![forbid(unsafe_code)]

pub mod coords;
pub mod kociemba;
#[cfg(feature = "optimal")]
pub mod optimal;
pub mod solver;
pub mod tables;
pub mod thistlethwaite;

mod clock;

pub use kociemba::{
    Kociemba, KociembaDistanceBundle, KociembaDistanceReport, KociembaDistanceTiming,
    KociembaOptions, KociembaStats,
};
#[cfg(feature = "optimal")]
pub use optimal::{
    OptimalDistanceBundle, OptimalOracle, OptimalReport, OptimalResult, OptimalStats, OptimalTiming,
};
pub use solver::{Limits, Solution, SolveError, Solver};
pub use thistlethwaite::{
    DistanceBundle, DistanceReport, DistanceTiming, TableLoadError, Thistlethwaite,
};
