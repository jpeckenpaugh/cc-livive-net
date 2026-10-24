//! Solver-support layer for `color-cube`: coordinates, move tables and the
//! versioned table bundle.
//!
//! Search (Thistlethwaite, Kociemba) is not implemented yet; M2a ends at the
//! coordinate and table boundary.

#![forbid(unsafe_code)]

pub mod coords;
pub mod tables;
