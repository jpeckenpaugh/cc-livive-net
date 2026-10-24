//! Clock shim for the solver's optional wall-clock limits (D028).
//!
//! Native builds use [`std::time::Instant`] exactly as before. On
//! `wasm32-unknown-unknown` `Instant::now` is not reliably implemented, so the
//! shim never reads a clock: [`TIME_LIMITS_ENABLED`] is `false`, [`now`]
//! returns a unit token, and [`elapsed_ms`] reports `0`. Time limits are
//! therefore disabled in the browser rather than panicking.

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;

/// Whether wall-clock limits are enforced.
#[cfg(not(target_arch = "wasm32"))]
pub const TIME_LIMITS_ENABLED: bool = true;

/// Whether wall-clock limits are enforced.
#[cfg(target_arch = "wasm32")]
pub const TIME_LIMITS_ENABLED: bool = false;

/// A placeholder clock reading on `wasm32`.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug)]
pub struct Instant;

/// Starts a clock reading.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn now() -> Instant {
    Instant::now()
}

/// Starts a clock reading. Always the placeholder on `wasm32`.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn now() -> Instant {
    Instant
}

/// Milliseconds elapsed since `started`.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn elapsed_ms(started: Instant) -> u128 {
    started.elapsed().as_millis()
}

/// Milliseconds elapsed since `started`. Always `0` on `wasm32`.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn elapsed_ms(_started: Instant) -> u128 {
    0
}
