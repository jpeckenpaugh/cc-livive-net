//! The deterministic PRNG used by the seeded constructors.
//!
//! `SplitMix64` is deliberately tiny and dependency-free. Its output stream is
//! part of the stable `from_seed` contract (D015): changing the constants or
//! the number of draws breaks every golden fixture.

/// A private SplitMix64 generator, byte-identical to `mine-sm`'s.
pub(crate) struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// Creates a generator from a seed. A zero seed is fine: the state is only
    /// advanced by a fixed odd increment before use.
    pub(crate) const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Returns the next 64 pseudo-random bits.
    pub(crate) fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Returns a value in `0..bound` using modulo reduction.
    ///
    /// `bound` must be non-zero. Modulo reduction is not exactly uniform: each
    /// output deviates from `1 / bound` by less than `1 / 2^64`, and the total
    /// variation distance is below `bound / 2^64`. The bias is documented but
    /// not corrected; no rejection sampling is used anywhere in this crate.
    pub(crate) fn next_bounded(&mut self, bound: u64) -> u64 {
        debug_assert!(bound > 0, "bound must be non-zero");
        self.next_u64() % bound
    }
}
