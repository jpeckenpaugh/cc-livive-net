//! Seeded uniform states and non-uniform random-walk scrambles.
//!
//! The two constructors have different contracts:
//!
//! - [`Cube::from_seed`] samples the full state space, up to the tiny modulo
//!   bias of the private SplitMix64 `next_bounded` helper. It is the
//!   constructor benchmarks and RL evaluation must use.
//! - [`Cube::from_scramble`] performs a pruned random walk. It is for demos and
//!   human-facing scrambles and is **not** uniform.
//!
//! # Canonical pruning
//!
//! Both `from_scramble` and [`Cube::write_canonical_mask`] share one rule
//! (D014): never repeat a face, and for commuting opposite faces keep the
//! higher face index. `concept.md` §8 gives the opposite example (`D` after
//! `U` as pruned) while §9 says "allow `D` only after `U`"; §9 wins, so `D`
//! after `U` is kept and `U` after `D` is pruned.

use crate::rng::SplitMix64;
use crate::{parity, Cube, Move, N_MOVES};

/// Maximum number of moves in a [`Scramble`].
pub const MAX_SCRAMBLE_LEN: usize = 32;

/// A fixed-capacity random-walk scramble.
///
/// The first `len` entries of `moves` are live; the rest are zero. `len` is
/// always at most [`MAX_SCRAMBLE_LEN`] when the value came from this crate, but
/// the fields are public to match `concept.md` §4. Treat `len` as an invariant
/// rather than free storage: `as_slice` defensively clamps it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Scramble {
    /// The moves of the walk, in application order.
    pub moves: [Move; MAX_SCRAMBLE_LEN],
    /// Number of live entries in `moves`.
    pub len: u8,
}

impl Scramble {
    /// The live moves of the walk.
    #[must_use]
    pub fn as_slice(&self) -> &[Move] {
        let len = (self.len as usize).min(MAX_SCRAMBLE_LEN);
        &self.moves[..len]
    }

    /// Whether the walk contains no moves.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The number of live moves.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

/// The upper bound on whole-walk attempts when a draw lands on solved.
const MAX_WALK_ATTEMPTS: u64 = 1024;

/// The upper bound on redrawing the final move of one walk.
const MAX_FINAL_REDRAWS: u8 = 64;

impl Cube {
    /// Returns a state drawn from the uniform distribution (up to modulo bias).
    ///
    /// This is the frozen 36-draw contract of D015:
    ///
    /// 1. `cp`: ascending Fisher–Yates, 7 draws with bounds `8..2`;
    /// 2. `co[0..7]`: 7 draws with bound 3, then `co[7]` from the twist sum;
    /// 3. `ep`: ascending Fisher–Yates, 11 draws with bounds `12..2`;
    /// 4. `eo[0..11]`: 11 draws with bound 2, then `eo[11]` from the flip sum;
    /// 5. parity fix: swap `ep[10]` and `ep[11]` if the permutation parities
    ///    differ (no draw);
    /// 6. if solved, restart with `seed.wrapping_add(1)`.
    ///
    /// The solved state has probability about `1 / 4.3e19`; the retry is capped
    /// at 1024 attempts and then panics. In normal use it never fires.
    #[must_use]
    pub fn from_seed(seed: u64) -> Cube {
        let mut offset = 0u64;
        loop {
            let mut rng = SplitMix64::new(seed.wrapping_add(offset));
            let cube = uniform_state(&mut rng);
            if !cube.is_solved() {
                return cube;
            }
            offset += 1;
            assert!(
                offset < MAX_WALK_ATTEMPTS,
                "from_seed could not leave the solved state after 1024 attempts"
            );
        }
    }

    /// Builds a pruned random-walk scramble and the cube it produces.
    ///
    /// `turns` is clamped to [`MAX_SCRAMBLE_LEN`]. Candidates are rejected if
    /// they repeat the previous face or, for an opposite pair, move to a lower
    /// face index than the previous move (D014). Rejection consumes another
    /// draw, so the returned scramble is always exactly `min(turns, 32)` moves.
    ///
    /// If the finished walk is solved, the final move is redrawn from the same
    /// stream, respecting the same pruning, until the result is not solved; at
    /// most 64 redraws are tried. If all 64 fail, the whole walk restarts with
    /// `seed.wrapping_add(1)`. `turns == 0` returns the solved cube and an empty
    /// scramble; it is the one case excluded from the "not solved" contract.
    #[must_use]
    pub fn from_scramble(seed: u64, turns: u8) -> (Cube, Scramble) {
        let turns = (turns as usize).min(MAX_SCRAMBLE_LEN);
        let mut offset = 0u64;

        'walk: loop {
            let mut rng = SplitMix64::new(seed.wrapping_add(offset));
            let mut cube = Cube::SOLVED;
            let mut scramble = Scramble {
                moves: [Move(0); MAX_SCRAMBLE_LEN],
                len: 0,
            };
            let mut last = None;
            let mut previous_cube = Cube::SOLVED;
            let mut previous_last = None;

            for slot in 0..turns {
                let candidate = next_accepted(&mut rng, last);
                previous_cube = cube;
                previous_last = last;
                cube.apply(candidate);
                scramble.moves[slot] = candidate;
                scramble.len += 1;
                last = Some(candidate);
            }

            if turns == 0 || !cube.is_solved() {
                return (cube, scramble);
            }

            let mut redraws = 0u8;
            while redraws < MAX_FINAL_REDRAWS {
                let candidate = next_accepted(&mut rng, previous_last);
                let mut trial = previous_cube;
                trial.apply(candidate);
                if !trial.is_solved() {
                    scramble.moves[turns - 1] = candidate;
                    return (trial, scramble);
                }
                redraws += 1;
            }

            offset += 1;
            assert!(
                offset < MAX_WALK_ATTEMPTS,
                "from_scramble could not leave the solved state after 1024 walks"
            );
            continue 'walk;
        }
    }

    /// Writes the canonical action mask: `1.0` for a legal non-redundant move.
    ///
    /// `last = None` prunes nothing. Otherwise every power of the previous face
    /// is pruned, and an opposite-face move is pruned when its face index is
    /// lower than the previous one (D014). All other entries are `1.0`.
    ///
    /// # Panics
    ///
    /// Panics if `last` is `Some(mv)` with an index outside `0..18` (D010).
    pub fn write_canonical_mask(&self, last: Option<Move>, out: &mut [f32; N_MOVES]) {
        if let Some(mv) = last {
            // D010 precondition, enforced by `face`.
            let _ = mv.face();
        }
        for (slot, index) in out.iter_mut().zip(0..N_MOVES) {
            *slot = if is_acceptable(last, Move(index as u8)) {
                1.0
            } else {
                0.0
            };
        }
    }
}

/// Whether `candidate` is allowed after `last` under the canonical rule.
pub(crate) fn is_acceptable(last: Option<Move>, candidate: Move) -> bool {
    let Some(last) = last else {
        return true;
    };
    let previous = last.face();
    let next = candidate.face();
    if previous == next {
        return false;
    }
    !(previous.abs_diff(next) == 3 && next < previous)
}

/// Draws candidates until one passes the canonical filter.
fn next_accepted(rng: &mut SplitMix64, last: Option<Move>) -> Move {
    loop {
        // The filter always leaves at least twelve legal moves, so this
        // terminates with probability one after a bounded number of draws.
        let candidate = Move(rng.next_bounded(N_MOVES as u64) as u8);
        if is_acceptable(last, candidate) {
            return candidate;
        }
    }
}

/// One full draw from the frozen `from_seed` contract.
fn uniform_state(rng: &mut SplitMix64) -> Cube {
    let mut cp = [0u8, 1, 2, 3, 4, 5, 6, 7];
    for i in 0..7 {
        let j = i + rng.next_bounded((8 - i) as u64) as usize;
        cp.swap(i, j);
    }

    let mut co = [0u8; 8];
    let mut twist = 0u8;
    for slot in co.iter_mut().take(7) {
        *slot = rng.next_bounded(3) as u8;
        twist += *slot;
    }
    co[7] = (3 - twist % 3) % 3;

    let mut ep = [0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
    for i in 0..11 {
        let j = i + rng.next_bounded((12 - i) as u64) as usize;
        ep.swap(i, j);
    }

    let mut eo = [0u8; 12];
    let mut flip = 0u8;
    for slot in eo.iter_mut().take(11) {
        *slot = rng.next_bounded(2) as u8;
        flip += *slot;
    }
    eo[11] = flip % 2;

    if parity(&cp) != parity(&ep) {
        ep.swap(10, 11);
    }

    Cube { cp, co, ep, eo }
}
