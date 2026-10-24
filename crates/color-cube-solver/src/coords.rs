//! Coordinates over [`Cube`] for search (D017).
//!
//! Three coordinates (`twist`, `flip`, `slice`) are the core's own encodings,
//! re-exported here so callers have one import path. The other three describe
//! permutations:
//!
//! - `cp` — Lehmer rank of the eight-corner permutation, `0..=40319`.
//! - `ep8` — Lehmer rank of the eight U/D edges in positions `0..=7`,
//!   `0..=40319`. Defined only on `G1` states (a clean slice), see below.
//! - `sp` — rank of the order of the four middle-slice edges in positions
//!   `8..=11`, `0..=23`.
//!
//! # `G1` and the dirty-slice precondition
//!
//! `ep8` and `sp` only exist while the four middle-slice pieces occupy
//! positions `8..=11`. [`ep8_rank`] and [`sp_rank`] panic otherwise; callers
//! must not read them off `G1` (D018). The four setters below that build an
//! `ep` from a permutation coordinate always normalise the cube into `G1`.
//!
//! # Setter contract
//!
//! Unlike the core's raw setters, every setter here is total for in-range input
//! and produces a cube for which [`Cube::is_valid`] is true. When the requested
//! coordinate would leave `cp` and `ep` with different parities, the setter
//! swaps two entries of the permutation it was *not* asked to write, so the
//! requested coordinate is exact. The swap necessarily changes some other
//! coordinate; each setter documents which ones. [`set_cp_ep8`] realises both
//! permutation coordinates with a single repair.
//!
//! All setters panic when the input is out of range, matching the core (D010).

use color_cube::Cube;

pub use color_cube::{N_FLIP, N_SLICE, N_TWIST, SLICE_SOLVED};

/// Number of eight-element permutation ranks, `8!`.
pub const N_CP: usize = 40_320;

/// Number of eight-element permutation ranks, `8!`.
pub const N_EP8: usize = 40_320;

/// Number of four-element permutation ranks, `4!`.
pub const N_SP: usize = 24;

/// The seven-term corner-twist coordinate (see [`Cube::corner_twist`]).
#[must_use]
pub fn twist(cube: &Cube) -> u16 {
    cube.corner_twist()
}

/// Writes `twist`; `cp`, `ep` and `eo` are preserved and a valid cube stays
/// valid.
pub fn set_twist(cube: &mut Cube, value: u16) {
    cube.set_corner_twist(value);
}

/// The eleven-term edge-flip coordinate (see [`Cube::edge_flip`]).
#[must_use]
pub fn flip(cube: &Cube) -> u16 {
    cube.edge_flip()
}

/// Writes `flip`; `cp`, `co` and `ep` are preserved and a valid cube stays
/// valid.
pub fn set_flip(cube: &mut Cube, value: u16) {
    cube.set_edge_flip(value);
}

/// The slice-combination coordinate (see [`Cube::slice_combination`]).
#[must_use]
pub fn slice(cube: &Cube) -> u16 {
    cube.slice_combination()
}

/// Writes `slice` and repairs parity when needed.
///
/// The stable partition in [`Cube::set_slice_combination`] preserves `sp` but
/// can flip `ep` parity. If that happens, two middle pieces are swapped, which
/// keeps the slice subset, and therefore `slice`, exact. Consequently `sp` may
/// change; `cp`, `co`, `eo` are untouched.
pub fn set_slice(cube: &mut Cube, value: u16) {
    cube.set_slice_combination(value);
    if parity(&cube.cp) != parity(&cube.ep) {
        swap_two_middle(cube);
    }
}

/// The Lehmer rank of `cp`, `0..=40319` (identity is `0`).
#[must_use]
pub fn cp_rank(cube: &Cube) -> u16 {
    lehmer_rank(&cube.cp) as u16
}

/// Writes `cp` and repairs parity when needed.
///
/// The rank is exact. If the new corner permutation has a different parity from
/// `ep`, `ep[0]` and `ep[1]` are swapped, so `ep8`, `sp` and possibly `slice`
/// may change; `co`, `eo` are untouched.
pub fn set_cp(cube: &mut Cube, value: u16) {
    assert!(
        (value as usize) < N_CP,
        "corner permutation rank out of range"
    );
    cube.cp = lehmer_unrank(u32::from(value));
    if parity(&cube.cp) != parity(&cube.ep) {
        cube.ep.swap(0, 1);
    }
}

/// The Lehmer rank of the eight U/D edges in positions `0..=7`, `0..=40319`.
///
/// # Panics
///
/// Panics unless positions `0..=7` hold exactly the eight U/D pieces and
/// positions `8..=11` hold the four middle pieces (`G1`, D018).
#[must_use]
pub fn ep8_rank(cube: &Cube) -> u16 {
    assert!(is_g1(cube), "ep8 is undefined outside G1");
    let mut permutation = [0u8; 8];
    permutation.copy_from_slice(&cube.ep[..8]);
    lehmer_rank(&permutation) as u16
}

/// Writes `ep8` and normalises the cube into `G1`.
///
/// Positions `0..=7` receive the ranked permutation of the U/D pieces. The four
/// middle pieces are gathered in their existing positional order and written
/// into positions `8..=11`, so `slice` becomes [`SLICE_SOLVED`] (494) and `sp`
/// is preserved. If the resulting `ep` parity differs from `cp`, `cp[0]` and
/// `cp[1]` are swapped: `cp_rank` may change, while `ep8`, `slice` and `sp` are
/// exact. `co`, `eo` are untouched.
pub fn set_ep8(cube: &mut Cube, value: u16) {
    assert!((value as usize) < N_EP8, "ep8 rank out of range");
    let permutation: [u8; 8] = lehmer_unrank(u32::from(value));
    let middle = gather_middle(cube);
    cube.ep[..8].copy_from_slice(&permutation);
    cube.ep[8..].copy_from_slice(&middle);
    if parity(&cube.cp) != parity(&cube.ep) {
        cube.cp.swap(0, 1);
    }
}

/// The rank of the order of the four middle-slice pieces in `8..=11`, `0..=23`.
///
/// # Panics
///
/// Panics unless the slice is clean (`G1`, D018).
#[must_use]
pub fn sp_rank(cube: &Cube) -> u16 {
    assert!(is_g1(cube), "sp is undefined outside G1");
    let mut permutation = [0u8; 4];
    for (slot, &piece) in permutation.iter_mut().zip(&cube.ep[8..]) {
        *slot = piece - 8;
    }
    lehmer_rank(&permutation) as u16
}

/// Writes `sp` and normalises the cube into `G1`.
///
/// Positions `8..=11` receive pieces `8 + p` for the ranked permutation `p`.
/// The U/D pieces are gathered in their existing positional order and written
/// into positions `0..=7`, so `slice` becomes [`SLICE_SOLVED`] (494) and `ep8`
/// is preserved. If parity needs repairing, `cp[0]` and `cp[1]` are swapped, so
/// `cp_rank` may change. `co`, `eo` are untouched.
pub fn set_sp(cube: &mut Cube, value: u16) {
    assert!((value as usize) < N_SP, "sp rank out of range");
    let permutation: [u8; 4] = lehmer_unrank(u32::from(value));
    let mut middle = [0u8; 4];
    let mut other = [0u8; 8];
    let mut other_len = 0;
    for &piece in &cube.ep {
        if piece < 8 {
            other[other_len] = piece;
            other_len += 1;
        }
    }
    assert!(other_len == 8, "ep is not a permutation");
    for (slot, &rank) in middle.iter_mut().zip(&permutation) {
        *slot = rank + 8;
    }
    cube.ep[..8].copy_from_slice(&other);
    cube.ep[8..].copy_from_slice(&middle);
    if parity(&cube.cp) != parity(&cube.ep) {
        cube.cp.swap(0, 1);
    }
}

/// Writes `cp` and `ep8` together with one parity repair.
///
/// This is the phase-2 realisation: both permutation coordinates are exact.
/// The middle pieces are gathered in their existing positional order and placed
/// in `8..=11`, so `slice = 494` and `sp` is preserved. If `cp` and `ep8` have
/// different parities, two middle pieces are swapped; that keeps `cp`, `ep8`
/// and `slice` exact but changes `sp`. `co`, `eo` are untouched.
pub fn set_cp_ep8(cube: &mut Cube, cp: u16, ep8: u16) {
    assert!((cp as usize) < N_CP, "corner permutation rank out of range");
    assert!((ep8 as usize) < N_EP8, "ep8 rank out of range");
    cube.cp = lehmer_unrank(u32::from(cp));
    let permutation: [u8; 8] = lehmer_unrank(u32::from(ep8));
    let middle = gather_middle(cube);
    cube.ep[..8].copy_from_slice(&permutation);
    cube.ep[8..].copy_from_slice(&middle);
    if parity(&cube.cp) != parity(&cube.ep) {
        swap_two_middle(cube);
    }
}

/// Whether the four middle pieces occupy positions `8..=11`.
#[must_use]
pub fn is_g1(cube: &Cube) -> bool {
    cube.ep[..8].iter().all(|&piece| piece < 8) && cube.ep[8..].iter().all(|&piece| piece >= 8)
}

/// Parity of a permutation: `0` for even, `1` for odd.
fn parity<const N: usize>(values: &[u8; N]) -> u8 {
    let mut inversions = 0u32;
    for (i, &a) in values.iter().enumerate() {
        for &b in &values[i + 1..] {
            if a > b {
                inversions += 1;
            }
        }
    }
    (inversions % 2) as u8
}

/// Swaps the pieces at the first two middle-slice positions.
///
/// The position set is unchanged (both entries stay `>= 8`), so `slice` is
/// preserved while `ep` parity flips and `sp` changes. Requires four middle
/// pieces.
fn swap_two_middle(cube: &mut Cube) {
    let mut first = None;
    for (position, &piece) in cube.ep.iter().enumerate() {
        if piece >= 8 {
            match first {
                None => first = Some(position),
                Some(other) => {
                    cube.ep.swap(other, position);
                    return;
                }
            }
        }
    }
    unreachable!("a valid ep always has four middle pieces");
}

/// Gathers the four middle pieces in positional order.
fn gather_middle(cube: &Cube) -> [u8; 4] {
    let mut middle = [0u8; 4];
    let mut len = 0;
    for &piece in &cube.ep {
        if piece >= 8 {
            assert!(len < 4, "ep is not a permutation");
            middle[len] = piece;
            len += 1;
        }
    }
    assert!(len == 4, "ep is not a permutation");
    middle
}

/// Lehmer rank of a permutation of `0..N`, identity `0`.
fn lehmer_rank<const N: usize>(permutation: &[u8; N]) -> u32 {
    let mut rank = 0u32;
    for (i, &value) in permutation.iter().enumerate() {
        let mut smaller = 0u32;
        for &later in &permutation[i + 1..] {
            if later < value {
                smaller += 1;
            }
        }
        rank = rank * (N - i) as u32 + smaller;
    }
    rank
}

/// Inverse of [`lehmer_rank`].
fn lehmer_unrank<const N: usize>(mut rank: u32) -> [u8; N] {
    let mut available = [0u8; N];
    for (slot, value) in available.iter_mut().enumerate() {
        *value = slot as u8;
    }
    let mut out = [0u8; N];
    let mut len = N;
    for slot in out.iter_mut() {
        let factorial = factorial(len - 1);
        let index = (rank / factorial) as usize;
        rank %= factorial;
        *slot = available[index];
        available.copy_within(index + 1..len, index);
        len -= 1;
    }
    out
}

/// `n!` for the small `n` used by the permutation unrankers.
const fn factorial(n: usize) -> u32 {
    let mut result = 1u32;
    let mut i = 2;
    while i <= n {
        result *= i as u32;
        i += 1;
    }
    result
}
