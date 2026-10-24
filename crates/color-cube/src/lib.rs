//! A deterministic, allocation-free 3×3×3 colour-cube state machine.
//!
//! This crate is the `no_std`, zero-dependency core described in `concept.md`
//! §4. It stores the cube at the cubie level (`cp`, `co`, `ep`, `eo`), converts
//! losslessly to and from the 54 facelets of §3.2, and implements the 18 face
//! turns of §3.5. It also provides seeded scrambles, Singmaster notation and
//! the RL observation/mask surface, all without allocation. Solvers live in a
//! separate crate.
//!
//! # Conventions
//!
//! Fixed by `concept.md` §3 and not configurable:
//!
//! - Faces are indexed `U R F D L B`; facelet `f` of face `i` is `i * 9 + f`,
//!   row-major as seen from outside the face.
//! - [`Color`] discriminants follow the *colour* order of §3.2
//!   (`U=0, D=1, F=2, B=3, R=4, L=5`), which differs from face order.
//! - Cubie positions and the facelet triple/pair of each position follow
//!   Kociemba's tables, as written out in §3.3.
//! - Corner orientation is the index of the `U`/`D` sticker within the
//!   position's facelet triple; edge orientation is the index of the piece's
//!   reference sticker (`U`/`D` for `U`/`D` edges, `F`/`B` for middle-slice
//!   edges) within the position's facelet pair. §3.4 describes the common case
//!   of this rule; the index form is the one that satisfies
//!   `sum(co) % 3 == 0` and `sum(eo) % 2 == 0` for every legal move, and it is
//!   the form assumed by Kociemba's move tables.
//! - [`Move`] is a face-major action index: `0..=2` are `U`, `U2`, `U'`; `3..=5`
//!   are `R`, `R2`, `R'`; and so on through `B`.
//!
//! The move transitions are derived from the facelet net at compile time; see
//! the private `geometry` module for the method and the cross-check story.
//!
//! # Seeded construction and RL surface
//!
//! [`Cube::from_seed`] implements the frozen 36-draw uniform-state contract
//! (D015). [`Cube::from_scramble`] is a pruned, non-uniform random walk for
//! demos. [`Cube::write_observation`] emits the canonical 324-float one-hot
//! (D006), and the two mask writers expose the action surface. When a
//! canonical mask or the random walk prunes a move, the rule is D014: keep the
//! higher face index of a commuting opposite pair.

#![no_std]
#![forbid(unsafe_code)]

mod coords;
mod error;
mod geometry;
mod notation;
mod rng;
mod scramble;

pub use coords::{N_FLIP, N_SLICE, N_TWIST, SLICE_SOLVED};
pub use error::CubeError;
pub use notation::{
    parse_moves, write_singmaster, FixedMoves, NotationError, ParseError, MAX_PARSED_MOVES,
};
pub use scramble::{Scramble, MAX_SCRAMBLE_LEN};

/// Number of face turns: 3 powers for each of 6 faces.
pub const N_MOVES: usize = 18;

/// Number of facelets on a 3×3×3 cube.
pub const N_FACELETS: usize = 54;

/// Number of sticker colours, one per face.
pub const N_COLORS: usize = 6;

/// Number of floats in the canonical observation, `facelet * 6 + color`.
pub const N_OBSERVATIONS: usize = N_FACELETS * N_COLORS;

/// A sticker colour.
///
/// The discriminants are the encoding fixed by `concept.md` §3.2, i.e. colour
/// order `U D F B R L`, not face order (D009).
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Color {
    /// Up: white.
    U = 0,
    /// Down: yellow.
    D = 1,
    /// Front: green.
    F = 2,
    /// Back: blue.
    B = 3,
    /// Right: red.
    R = 4,
    /// Left: orange.
    L = 5,
}

impl Color {
    /// All six colours in discriminant order.
    pub const ALL: [Color; N_COLORS] = [Color::U, Color::D, Color::F, Color::B, Color::R, Color::L];

    /// The discriminant as an index in `0..6`.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// One of the 18 face turns, as a discrete action index in `0..18`.
///
/// The field is public because `concept.md` §3.5 requires an RL agent to treat
/// a move as an action index. Values `>= 18` are not moves; the operations
/// here document a precondition and panic on them (D010). Use
/// [`Move::from_index`] for a checked conversion.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Move(pub u8);

impl Move {
    /// All 18 moves in action-index order.
    pub const ALL: [Move; N_MOVES] = [
        Move(0),
        Move(1),
        Move(2),
        Move(3),
        Move(4),
        Move(5),
        Move(6),
        Move(7),
        Move(8),
        Move(9),
        Move(10),
        Move(11),
        Move(12),
        Move(13),
        Move(14),
        Move(15),
        Move(16),
        Move(17),
    ];

    /// Returns the move with the given action index, or `None` if it is not
    /// in `0..18`.
    #[must_use]
    pub const fn from_index(index: u8) -> Option<Move> {
        if index < N_MOVES as u8 {
            Some(Move(index))
        } else {
            None
        }
    }

    /// The inverse move: `X` becomes `X'`, `X2` stays `X2`.
    ///
    /// # Panics
    ///
    /// Panics if the index is not in `0..18` (D010).
    #[must_use]
    pub const fn inverse(self) -> Move {
        assert!(self.0 < N_MOVES as u8, "move index out of range");
        Move((self.0 / 3) * 3 + (2 - self.0 % 3))
    }

    /// The number of quarter turns this move represents: 1 for `X` and `X'`,
    /// 2 for `X2`.
    ///
    /// # Panics
    ///
    /// Panics if the index is not in `0..18` (D010).
    #[must_use]
    pub const fn quarter_turns(self) -> u8 {
        assert!(self.0 < N_MOVES as u8, "move index out of range");
        if self.0 % 3 == 1 {
            2
        } else {
            1
        }
    }

    /// The face index in `U R F D L B` order, `0..5`.
    ///
    /// # Panics
    ///
    /// Panics if the index is not in `0..18` (D010).
    #[must_use]
    pub const fn face(self) -> u8 {
        assert!(self.0 < N_MOVES as u8, "move index out of range");
        self.0 / 3
    }

    /// The turn power: 1 for clockwise, 2 for a half turn, 3 for
    /// counter-clockwise.
    ///
    /// # Panics
    ///
    /// Panics if the index is not in `0..18` (D010).
    #[must_use]
    pub const fn power(self) -> u8 {
        assert!(self.0 < N_MOVES as u8, "move index out of range");
        self.0 % 3 + 1
    }

    /// The move's Singmaster name: `"U"`, `"U2"`, `"U'"`, and so on.
    ///
    /// # Panics
    ///
    /// Panics if the index is not in `0..18` (D010).
    #[must_use]
    pub const fn to_singmaster(self) -> &'static str {
        const NAMES: [&str; N_MOVES] = [
            "U", "U2", "U'", "R", "R2", "R'", "F", "F2", "F'", "D", "D2", "D'", "L", "L2", "L'",
            "B", "B2", "B'",
        ];
        assert!(self.0 < N_MOVES as u8, "move index out of range");
        NAMES[self.0 as usize]
    }
}

/// A legal-or-not cubie-level cube state.
///
/// The fields are public so callers can inspect and construct states cheaply;
/// use [`Cube::is_valid`] before trusting a state that did not come from this
/// crate. The type is `Copy` and stores no counters (D007).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Cube {
    /// Piece index at each corner position.
    pub cp: [u8; 8],
    /// Twist of the piece at each corner position, `0..3`.
    pub co: [u8; 8],
    /// Piece index at each edge position.
    pub ep: [u8; 12],
    /// Flip of the piece at each edge position, `0..2`.
    pub eo: [u8; 12],
}

impl Cube {
    /// The unique solved state: identity permutations, zero orientations.
    pub const SOLVED: Cube = Cube {
        cp: [0, 1, 2, 3, 4, 5, 6, 7],
        co: [0; 8],
        ep: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        eo: [0; 12],
    };

    /// Applies one face turn in place.
    ///
    /// # Panics
    ///
    /// Panics if `mv` is not in `0..18` (D010). Panics only for an invalid
    /// move; every cube state, valid or not, is transformed without
    /// additional checks.
    pub fn apply(&mut self, mv: Move) {
        let index = mv.0 as usize;
        assert!(index < N_MOVES, "move index out of range");
        let transition = &geometry::TRANSITIONS[index];

        let mut cp = [0u8; 8];
        let mut co = [0u8; 8];
        for (destination, (&source, &delta)) in
            transition.cp.iter().zip(transition.co.iter()).enumerate()
        {
            let source = source as usize;
            cp[destination] = self.cp[source];
            co[destination] = (self.co[source] + delta) % 3;
        }

        let mut ep = [0u8; 12];
        let mut eo = [0u8; 12];
        for (destination, (&source, &delta)) in
            transition.ep.iter().zip(transition.eo.iter()).enumerate()
        {
            let source = source as usize;
            ep[destination] = self.ep[source];
            eo[destination] = (self.eo[source] + delta) % 2;
        }

        self.cp = cp;
        self.co = co;
        self.ep = ep;
        self.eo = eo;
    }

    /// Applies a sequence of face turns in order.
    ///
    /// # Panics
    ///
    /// Panics if any move is not in `0..18` (D010).
    pub fn apply_all(&mut self, moves: &[Move]) {
        for &mv in moves {
            self.apply(mv);
        }
    }

    /// Applies the inverse of `mv` in place.
    ///
    /// # Panics
    ///
    /// Panics if `mv` is not in `0..18` (D010).
    pub fn apply_inverse(&mut self, mv: Move) {
        self.apply(mv.inverse());
    }

    /// Whether the cube is in the solved state.
    #[must_use]
    pub fn is_solved(&self) -> bool {
        self.cp.iter().enumerate().all(|(i, &p)| p as usize == i)
            && self.ep.iter().enumerate().all(|(i, &p)| p as usize == i)
            && self.co.iter().all(|&o| o == 0)
            && self.eo.iter().all(|&o| o == 0)
    }

    /// Whether the state satisfies all four invariants of `concept.md` §3.6.
    ///
    /// Range-checks every entry before using it as an index, so arbitrary
    /// bytes never panic.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        if self.cp.iter().any(|&p| p as usize >= 8)
            || self.ep.iter().any(|&p| p as usize >= 12)
            || self.co.iter().any(|&o| o >= 3)
            || self.eo.iter().any(|&o| o >= 2)
        {
            return false;
        }
        if !is_permutation(&self.cp) || !is_permutation(&self.ep) {
            return false;
        }
        let twist: u32 = self.co.iter().map(|&o| u32::from(o)).sum();
        if !twist.is_multiple_of(3) {
            return false;
        }
        let flip: u32 = self.eo.iter().map(|&o| u32::from(o)).sum();
        if !flip.is_multiple_of(2) {
            return false;
        }
        parity(&self.cp) == parity(&self.ep)
    }

    /// Writes the canonical 324-float one-hot observation (D006).
    ///
    /// For facelet `f` with colour index `c`, index `f * 6 + c` is `1.0`; the
    /// other five entries for that facelet are `0.0`. This overwrites every
    /// slot of `out`.
    ///
    /// # Panics
    ///
    /// Panics if the cubie entries are out of range, as [`Cube::facelets`] does.
    pub fn write_observation(&self, out: &mut [f32; N_OBSERVATIONS]) {
        for slot in out.iter_mut() {
            *slot = 0.0;
        }
        for (facelet, color) in self.facelets().iter().enumerate() {
            out[facelet * N_COLORS + color.index()] = 1.0;
        }
    }

    /// Writes the default action mask: every one of the 18 moves is legal.
    ///
    /// This overwrites `out` with `1.0`. Canonical pruning is opt-in through
    /// [`Cube::write_canonical_mask`].
    pub fn write_action_mask(&self, out: &mut [f32; N_MOVES]) {
        for slot in out.iter_mut() {
            *slot = 1.0;
        }
    }

    /// Writes the 54 facelet colours in `U R F D L B` order.
    ///
    /// Defined for any state whose entries are in range (`cp < 8`, `ep < 12`,
    /// `co < 3`, `eo < 2`); the state need not satisfy the permutation,
    /// twist, flip or parity invariants. Centres always carry the fixed solved
    /// colours.
    ///
    /// # Panics
    ///
    /// Panics if an entry is out of range.
    #[must_use]
    pub fn facelets(&self) -> [Color; N_FACELETS] {
        let mut colors = [0u8; N_FACELETS];

        for (face, &color) in geometry::FACE_COLORS.iter().enumerate() {
            colors[face * 9 + 4] = color.index() as u8;
        }

        for (position, (&piece, &orientation)) in self.cp.iter().zip(self.co.iter()).enumerate() {
            let piece_colors = geometry::corner_colors(piece as usize);
            let facelets = geometry::corner_facelets(position);
            let orientation = orientation as usize;
            for (k, &color) in piece_colors.iter().enumerate() {
                colors[facelets[(k + orientation) % 3] as usize] = color;
            }
        }

        for (position, (&piece, &orientation)) in self.ep.iter().zip(self.eo.iter()).enumerate() {
            let piece_colors = geometry::edge_colors(piece as usize);
            let facelets = geometry::edge_facelets(position);
            let orientation = orientation as usize;
            for (k, &color) in piece_colors.iter().enumerate() {
                colors[facelets[(k + orientation) % 2] as usize] = color;
            }
        }

        let mut out = [Color::U; N_FACELETS];
        for (slot, &index) in out.iter_mut().zip(colors.iter()) {
            *slot = geometry::color_from_index(index);
        }
        out
    }

    /// Builds a cube from 54 facelet colours.
    ///
    /// Checks, in order: colour counts, centres, permutation completeness,
    /// twist sum, flip sum, and corner/edge parity. The first failing check
    /// determines the [`CubeError`] variant.
    ///
    /// # Errors
    ///
    /// Returns the matching variant when the facelets are not a legal cube.
    pub fn from_facelets(facelets: &[Color; N_FACELETS]) -> Result<Self, CubeError> {
        let mut counts = [0u8; N_COLORS];
        for color in facelets {
            counts[color.index()] += 1;
        }
        if counts.iter().any(|&count| count != 9) {
            return Err(CubeError::MalformedFacelets);
        }

        for (face, &expected) in geometry::FACE_COLORS.iter().enumerate() {
            if facelets[face * 9 + 4] != expected {
                return Err(CubeError::InvalidCenter);
            }
        }

        let mut colors = [0u8; N_FACELETS];
        for (slot, &color) in colors.iter_mut().zip(facelets.iter()) {
            *slot = color.index() as u8;
        }

        let (cube, permutation_ok) = geometry::decode(&colors);
        if !permutation_ok {
            return Err(CubeError::InvalidPermutation);
        }

        let twist: u32 = cube.co.iter().map(|&o| u32::from(o)).sum();
        if !twist.is_multiple_of(3) {
            return Err(CubeError::Twist);
        }

        let flip: u32 = cube.eo.iter().map(|&o| u32::from(o)).sum();
        if !flip.is_multiple_of(2) {
            return Err(CubeError::Flip);
        }

        if parity(&cube.cp) != parity(&cube.ep) {
            return Err(CubeError::Parity);
        }

        Ok(cube)
    }
}

/// Whether every value in `0..N` appears exactly once.
fn is_permutation<const N: usize>(values: &[u8; N]) -> bool {
    let mut seen = [false; N];
    for &value in values {
        let value = value as usize;
        if value >= N || seen[value] {
            return false;
        }
        seen[value] = true;
    }
    true
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
