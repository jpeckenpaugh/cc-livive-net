//! Search-oriented coordinates over [`Cube`] (D017).
//!
//! These are the three coordinates that only need cubie arrays: corner twist,
//! edge flip and the slice combination. The permutation coordinates (`cp`,
//! `ep8`, `sp`) and every move table live in `color-cube-solver`, which depends
//! on this crate.
//!
//! # Encodings
//!
//! - `corner_twist = Σ co[i] · 3^i` for `i = 0..=6`. The eighth value is implied
//!   by `sum(co) ≡ 0 (mod 3)`; [`Cube::set_corner_twist`] writes it from the
//!   seven-digit sum. Range `0..=2186` (`3^7 - 1`).
//! - `edge_flip = Σ eo[i] · 2^i` for `i = 0..=10`, with `eo[11]` implied by
//!   `sum(eo) ≡ 0 (mod 2)`. Range `0..=2047`.
//! - `slice_combination` ranks the four positions holding the middle-slice
//!   pieces `FR, FL, BL, BR` (piece indices `8..=11`) as
//!   `C(p0,1) + C(p1,2) + C(p2,3) + C(p3,4)` for `p0 < p1 < p2 < p3`, where
//!   `C(n,k) = 0` when `n < k`. Range `0..=494`; the solved arrangement
//!   `{8,9,10,11}` ranks [`SLICE_SOLVED`] (`= 494`), **not** zero.
//!
//! # Raw setters
//!
//! The setters here are deliberately raw (D017): they write only the cubie
//! array that the coordinate describes and never repair anything else.
//!
//! - [`Cube::set_corner_twist`] and [`Cube::set_edge_flip`] touch only `co` /
//!   `eo`, so a valid cube stays valid.
//! - [`Cube::set_slice_combination`] rearranges `ep` only. It uses a stable
//!   partition, so the relative order of the four middle pieces is preserved
//!   (and therefore so is `sp`), but the rearrangement can change the parity of
//!   `ep` and leave `cp` and `ep` with mismatched parities. That produces an
//!   invalid cube; the solver's `set_slice` repairs it.

use crate::Cube;

/// Number of corner-twist values, `3^7`.
pub const N_TWIST: usize = 2187;

/// Number of edge-flip values, `2^11`.
pub const N_FLIP: usize = 2048;

/// Number of four-position slice combinations, `C(12,4)`.
pub const N_SLICE: usize = 495;

/// The slice rank of the solved middle slice, `{8,9,10,11}`.
///
/// This is `C(8,1) + C(9,2) + C(10,3) + C(11,4) = 8 + 36 + 120 + 330 = 494`.
/// Pruning searches start their BFS from this value, not from `0`.
pub const SLICE_SOLVED: u16 = 494;

impl Cube {
    /// The seven-term corner-twist coordinate, `0..=2186`.
    ///
    /// The eighth corner is implied by the mod-3 sum and is ignored here. The
    /// value is computed from `co` alone, so it is total for every state whose
    /// entries are in range.
    #[must_use]
    pub fn corner_twist(&self) -> u16 {
        let mut value = 0u16;
        let mut place = 1u16;
        for &orientation in &self.co[..7] {
            value += u16::from(orientation) * place;
            place *= 3;
        }
        value
    }

    /// Writes the seven-term corner-twist coordinate.
    ///
    /// `co[0..7]` receive the base-3 digits of `value`, most significant last;
    /// `co[7]` is set to `(3 - sum % 3) % 3` so the twist sum is a multiple of
    /// three. `cp`, `ep` and `eo` are untouched, so a valid cube stays valid.
    ///
    /// # Panics
    ///
    /// Panics if `value >= 2187`.
    pub fn set_corner_twist(&mut self, value: u16) {
        assert!(
            (value as usize) < N_TWIST,
            "corner twist out of range (must be < {N_TWIST})"
        );
        let mut rest = value;
        let mut sum = 0u8;
        for orientation in &mut self.co[..7] {
            *orientation = (rest % 3) as u8;
            sum += *orientation;
            rest /= 3;
        }
        self.co[7] = (3 - sum % 3) % 3;
    }

    /// The eleven-term edge-flip coordinate, `0..=2047`.
    ///
    /// The twelfth edge is implied by the mod-2 sum and is ignored here.
    #[must_use]
    pub fn edge_flip(&self) -> u16 {
        let mut value = 0u16;
        let mut place = 1u16;
        for &orientation in &self.eo[..11] {
            value += u16::from(orientation) * place;
            place *= 2;
        }
        value
    }

    /// Writes the eleven-term edge-flip coordinate.
    ///
    /// `eo[0..11]` receive the base-2 digits of `value`; `eo[11]` is set to the
    /// sum parity. `cp`, `co` and `ep` are untouched, so a valid cube stays
    /// valid.
    ///
    /// # Panics
    ///
    /// Panics if `value >= 2048`.
    pub fn set_edge_flip(&mut self, value: u16) {
        assert!(
            (value as usize) < N_FLIP,
            "edge flip out of range (must be < {N_FLIP})"
        );
        let mut rest = value;
        let mut sum = 0u8;
        for orientation in &mut self.eo[..11] {
            *orientation = (rest % 2) as u8;
            sum += *orientation;
            rest /= 2;
        }
        self.eo[11] = sum % 2;
    }

    /// The slice-combination coordinate, `0..=494`.
    ///
    /// Ranks the four positions that hold the middle-slice pieces (piece
    /// indices `8..=11`) with the combinadic of [`SLICE_SOLVED`]'s docs.
    ///
    /// # Panics
    ///
    /// Panics unless `ep` contains each piece exactly once. Callers holding an
    /// arbitrary [`Cube`] should check [`Cube::is_valid`] first.
    #[must_use]
    pub fn slice_combination(&self) -> u16 {
        let positions = middle_positions(&self.ep);
        let mut rank = 0u32;
        for (k, &position) in positions.iter().enumerate() {
            rank += binom(position, k + 1);
        }
        rank as u16
    }

    /// Writes the slice-combination coordinate, `0..=494`.
    ///
    /// The four target positions are recovered by descending combinadic
    /// unranking. The middle pieces already in `ep` are then moved into those
    /// positions in their existing relative order, and the remaining pieces
    /// fill the other positions in their existing relative order. This is a
    /// stable partition: it preserves the order of the middle pieces, hence
    /// `sp`, but it may flip the parity of `ep` (D017).
    ///
    /// `cp`, `co` and `eo` are untouched.
    ///
    /// # Panics
    ///
    /// Panics if `value >= 495`, or if `ep` does not contain exactly four
    /// middle-slice pieces.
    pub fn set_slice_combination(&mut self, value: u16) {
        assert!(
            (value as usize) < N_SLICE,
            "slice combination out of range (must be < {N_SLICE})"
        );
        let targets = unrank_slice(value);

        let mut middle = [0u8; 4];
        let mut other = [0u8; 8];
        let mut middle_len = 0;
        let mut other_len = 0;
        for &piece in &self.ep {
            if piece >= 8 {
                assert!(middle_len < 4, "ep is not a permutation");
                middle[middle_len] = piece;
                middle_len += 1;
            } else {
                assert!(other_len < 8, "ep is not a permutation");
                other[other_len] = piece;
                other_len += 1;
            }
        }
        assert!(middle_len == 4 && other_len == 8, "ep is not a permutation");

        let mut next_middle = 0;
        let mut next_other = 0;
        for (position, slot) in self.ep.iter_mut().enumerate() {
            if targets.contains(&position) {
                *slot = middle[next_middle];
                next_middle += 1;
            } else {
                *slot = other[next_other];
                next_other += 1;
            }
        }
    }
}

/// The sorted positions of the four middle-slice pieces in `ep`.
///
/// # Panics
///
/// Panics unless exactly four entries are `>= 8`.
fn middle_positions(ep: &[u8; 12]) -> [usize; 4] {
    let mut positions = [0usize; 4];
    let mut len = 0;
    for (position, &piece) in ep.iter().enumerate() {
        if piece >= 8 {
            assert!(len < 4, "ep is not a permutation");
            positions[len] = position;
            len += 1;
        }
    }
    assert!(len == 4, "ep is not a permutation");
    positions
}

/// The binomial coefficient `C(n, k)`, and `0` when `n < k`.
pub(crate) const fn binom(n: usize, k: usize) -> u32 {
    if k > n {
        return 0;
    }
    let mut result = 1u32;
    let mut i = 0;
    while i < k {
        result = result * (n - i) as u32 / (i + 1) as u32;
        i += 1;
    }
    result
}

/// Recovers the four target positions of a slice rank.
///
/// Descending combinadic unranking: for `k = 4, 3, 2, 1`, take the largest
/// `p` with `C(p, k) <= remaining`, record it as `p_{k-1}`, and subtract.
fn unrank_slice(rank: u16) -> [usize; 4] {
    let mut remaining = u32::from(rank);
    let mut positions = [0usize; 4];
    for k in (1..=4).rev() {
        let mut p = 11;
        while binom(p, k) > remaining {
            p -= 1;
        }
        positions[k - 1] = p;
        remaining -= binom(p, k);
    }
    positions
}
