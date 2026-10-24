//! Facelet geometry and the move transitions derived from it.
//!
//! # Derivation
//!
//! Nothing in this module is transcribed from a move table. The six 90° turns
//! are generated from the net in `concept.md` §3.2, then composed into all 18
//! moves, and finally converted to cubie transitions.
//!
//! 1. Each of the six faces gets an outward unit normal `n` and two in-face
//!    axes `u` (to the viewer's right) and `v` (up), chosen so that
//!    `u * (col - 1) + v * (1 - row)` describes the printed 3×3 grid. For the
//!    net in §3.2 that is:
//!
//!    | face | `n`     | `u`     | `v`     |
//!    |------|---------|---------|---------|
//!    | U    | `+y`    | `+x`    | `-z`    |
//!    | R    | `+x`    | `-z`    | `+y`    |
//!    | F    | `+z`    | `+x`    | `+y`    |
//!    | D    | `-y`    | `+x`    | `+z`    |
//!    | L    | `-x`    | `+z`    | `+y`    |
//!    | B    | `-z`    | `-x`    | `+y`    |
//!
//!    `U` is printed above `F`, so its rows run from the back (`U1`) to the
//!    front (`U7..U9`); `D` is printed below `F`, so its rows run from the
//!    front (`D1..D3`) to the back. `R`, `L` and `B` are printed as seen from
//!    outside those faces, which is why their `u` axes differ in sign.
//!
//! 2. A sticker is located at `3n + u*(col - 1) + v*(1 - row)`. The factor 3
//!    keeps the 54 sticker centres distinct, so a closed-form rotation can be
//!    matched back to a facelet index exactly.
//!
//! 3. A clockwise turn (as viewed from outside the face) is a −90° rotation
//!    about `n` applied to every sticker whose coordinate along `n` is
//!    positive. Applying that to all 54 stickers and looking up the resulting
//!    positions yields the four-cycles in [`FACE_CYCLES`]; each entry lists a
//!    sticker that moves to the next entry's position.
//!
//! 4. `2` and `'` are compositions of the quarter-turn permutation, and the
//!    cubie transition for each of the 18 moves is obtained by applying the
//!    facelet permutation to the solved facelets and decoding the result.
//!    Because the source state is solved, the decoded permutation *is* the
//!    source-position table and the decoded orientations *are* the deltas.
//!
//! This table was cross-checked against Kociemba's published `moveCube`
//! (`cp/co/ep/eo` for all six quarter turns); see `tests/conventions.rs`. The
//! reference tables live only in the test tree, as required by §5.

use crate::{Color, Cube};

/// Colours in facelet-face order, `U R F D L B` (`concept.md` §3.2).
pub(crate) const FACE_COLORS: [Color; 6] =
    [Color::U, Color::R, Color::F, Color::D, Color::L, Color::B];

/// `Color` discriminant of each face index, in face order `U R F D L B`.
///
/// This is deliberately a table, not arithmetic: face order and colour order
/// differ (D009).
const FACE_COLOR_INDEX: [u8; 6] = [0, 4, 2, 1, 5, 3];

/// Facelets of each corner position, in the order fixed by `concept.md` §3.3.
const CORNER_FACELETS: [[u8; 3]; 8] = [
    [8, 9, 20],   // URF: U9 R1 F3
    [6, 18, 38],  // UFL: U7 F1 L3
    [0, 36, 47],  // ULB: U1 L1 B3
    [2, 45, 11],  // UBR: U3 B1 R3
    [29, 26, 15], // DFR: D3 F9 R7
    [27, 44, 24], // DLF: D1 L9 F7
    [33, 53, 42], // DBL: D7 B9 L7
    [35, 17, 51], // DRB: D9 R9 B7
];

/// Facelets of each edge position, in the order fixed by `concept.md` §3.3.
const EDGE_FACELETS: [[u8; 2]; 12] = [
    [5, 10],  // UR: U6 R2
    [7, 19],  // UF: U8 F2
    [3, 37],  // UL: U4 L2
    [1, 46],  // UB: U2 B2
    [32, 16], // DR: D6 R8
    [28, 25], // DF: D2 F8
    [30, 43], // DL: D4 L8
    [34, 52], // DB: D8 B8
    [23, 12], // FR: F6 R4
    [21, 41], // FL: F4 L6
    [50, 39], // BL: B6 L4
    [48, 14], // BR: B4 R6
];

/// The five four-cycles of each quarter turn, in face order `U R F D L B`.
///
/// A cycle `[a, b, c, d]` means the sticker at `a` moves to `b`, `b` to `c`,
/// `c` to `d` and `d` back to `a`. Derived as described in the module docs.
const FACE_CYCLES: [[[u8; 4]; 5]; 6] = [
    // U
    [
        [2, 8, 6, 0],
        [5, 7, 3, 1],
        [18, 36, 45, 9],
        [19, 37, 46, 10],
        [20, 38, 47, 11],
    ],
    // R
    [
        [51, 29, 20, 2],
        [48, 32, 23, 5],
        [45, 35, 26, 8],
        [11, 17, 15, 9],
        [14, 16, 12, 10],
    ],
    // F
    [
        [9, 29, 44, 6],
        [12, 28, 41, 7],
        [15, 27, 38, 8],
        [20, 26, 24, 18],
        [23, 25, 21, 19],
    ],
    // D
    [
        [51, 42, 24, 15],
        [52, 43, 25, 16],
        [53, 44, 26, 17],
        [29, 35, 33, 27],
        [32, 34, 30, 28],
    ],
    // L
    [
        [18, 27, 53, 0],
        [21, 30, 50, 3],
        [24, 33, 47, 6],
        [38, 44, 42, 36],
        [41, 43, 39, 37],
    ],
    // B
    [
        [42, 35, 11, 0],
        [39, 34, 14, 1],
        [36, 33, 17, 2],
        [47, 53, 51, 45],
        [50, 52, 48, 46],
    ],
];

/// Colour indices of each corner piece, in its home position's facelet order.
///
/// Index `0` is always the `U`/`D` colour, i.e. the piece's reference colour
/// used for corner orientation.
const CORNER_COLORS: [[u8; 3]; 8] = piece_colors_3();

/// Colour indices of each edge piece, in its home position's facelet order.
///
/// Index `0` is the reference colour: `U`/`D` for the eight `U`/`D` edges and
/// `F`/`B` for the four middle-slice edges.
const EDGE_COLORS: [[u8; 2]; 12] = piece_colors_2();

/// The solved facelet colours, as `Color` discriminant indices.
const SOLVED_COLORS: [u8; 54] = solved_colors();

/// A face turn expressed as a cubie transition.
///
/// `cp[i]`/`ep[i]` are the *source* positions of the pieces that end up in
/// position `i`; `co[i]`/`eo[i]` are orientation deltas added (modulo the
/// orientation base) to the source piece.
#[derive(Clone, Copy)]
pub(crate) struct Transition {
    /// Source position of each corner, indexed by destination position.
    pub(crate) cp: [u8; 8],
    /// Corner orientation delta per destination position.
    pub(crate) co: [u8; 8],
    /// Source position of each edge, indexed by destination position.
    pub(crate) ep: [u8; 12],
    /// Edge orientation delta per destination position.
    pub(crate) eo: [u8; 12],
}

/// The 18 moves, indexed per `concept.md` §3.5.
pub(crate) const TRANSITIONS: [Transition; 18] = build_transitions();

const EMPTY_TRANSITION: Transition = Transition {
    cp: [0; 8],
    co: [0; 8],
    ep: [0; 12],
    eo: [0; 12],
};

/// Builds the 18 transitions from [`FACE_CYCLES`] at compile time.
const fn build_transitions() -> [Transition; 18] {
    let mut out = [EMPTY_TRANSITION; 18];
    let mut face = 0;
    while face < 6 {
        let quarter = quarter_perm(face);
        let half = compose(&quarter, &quarter);
        let inverse = compose(&half, &quarter);
        out[face * 3] = transition(&quarter);
        out[face * 3 + 1] = transition(&half);
        out[face * 3 + 2] = transition(&inverse);
        face += 1;
    }
    out
}

/// Builds the facelet permutation of one clockwise quarter turn.
///
/// `perm[destination] = source`, matching [`Transition`].
const fn quarter_perm(face: usize) -> [u8; 54] {
    let mut perm = [0u8; 54];
    let mut i = 0;
    while i < 54 {
        perm[i] = i as u8;
        i += 1;
    }
    let mut cycle_index = 0;
    while cycle_index < 5 {
        let cycle = FACE_CYCLES[face][cycle_index];
        let mut k = 0;
        while k < 4 {
            let from = cycle[k];
            let to = cycle[(k + 1) % 4];
            perm[to as usize] = from;
            k += 1;
        }
        cycle_index += 1;
    }
    perm
}

/// Composition of two facelet permutations: applies `first`, then `second`.
const fn compose(first: &[u8; 54], second: &[u8; 54]) -> [u8; 54] {
    let mut out = [0u8; 54];
    let mut i = 0;
    while i < 54 {
        out[i] = first[second[i] as usize];
        i += 1;
    }
    out
}

/// Converts a facelet permutation of the solved cube into a cubie transition.
const fn transition(perm: &[u8; 54]) -> Transition {
    let mut colours = [0u8; 54];
    let mut i = 0;
    while i < 54 {
        colours[i] = SOLVED_COLORS[perm[i] as usize];
        i += 1;
    }
    let (cube, _) = decode(&colours);
    Transition {
        cp: cube.cp,
        co: cube.co,
        ep: cube.ep,
        eo: cube.eo,
    }
}

/// Decodes facelet colour indices into a cube.
///
/// The returned flag is `false` when a corner or edge piece is missing,
/// unrecognisable or duplicated. Orientation values for such a state are
/// arbitrary; callers must not use them when the flag is `false`.
pub(crate) const fn decode(colours: &[u8; 54]) -> (Cube, bool) {
    let mut cube = Cube::SOLVED;
    let mut ok = true;

    let mut seen_corners = [false; 8];
    let mut i = 0;
    while i < 8 {
        let facelets = CORNER_FACELETS[i];
        let observed = [
            colours[facelets[0] as usize],
            colours[facelets[1] as usize],
            colours[facelets[2] as usize],
        ];
        let mut piece = 8u8;
        let mut j = 0;
        while j < 8 {
            if same_set_3(observed, CORNER_COLORS[j]) {
                piece = j as u8;
            }
            j += 1;
        }
        if piece == 8 || seen_corners[piece as usize] {
            ok = false;
        } else {
            seen_corners[piece as usize] = true;
            cube.cp[i] = piece;
            cube.co[i] = reference_index_3(observed, CORNER_COLORS[piece as usize][0]);
        }
        i += 1;
    }

    let mut seen_edges = [false; 12];
    i = 0;
    while i < 12 {
        let facelets = EDGE_FACELETS[i];
        let observed = [colours[facelets[0] as usize], colours[facelets[1] as usize]];
        let mut piece = 12u8;
        let mut j = 0;
        while j < 12 {
            if same_set_2(observed, EDGE_COLORS[j]) {
                piece = j as u8;
            }
            j += 1;
        }
        if piece == 12 || seen_edges[piece as usize] {
            ok = false;
        } else {
            seen_edges[piece as usize] = true;
            cube.ep[i] = piece;
            cube.eo[i] = reference_index_2(observed, EDGE_COLORS[piece as usize][0]);
        }
        i += 1;
    }

    (cube, ok)
}

/// Position of the reference colour inside a three-colour corner reading.
const fn reference_index_3(observed: [u8; 3], reference: u8) -> u8 {
    let mut k = 0;
    while k < 3 {
        if observed[k] == reference {
            return k as u8;
        }
        k += 1;
    }
    0
}

/// Position of the reference colour inside a two-colour edge reading.
const fn reference_index_2(observed: [u8; 2], reference: u8) -> u8 {
    let mut k = 0;
    while k < 2 {
        if observed[k] == reference {
            return k as u8;
        }
        k += 1;
    }
    0
}

/// Whether two three-colour readings describe the same piece.
///
/// Piece colour sets are distinct, so membership of every observed colour is
/// sufficient and avoids sorting or allocation.
const fn same_set_3(a: [u8; 3], b: [u8; 3]) -> bool {
    contains_3(a, b[0]) && contains_3(a, b[1]) && contains_3(a, b[2])
}

/// Whether two two-colour readings describe the same piece.
const fn same_set_2(a: [u8; 2], b: [u8; 2]) -> bool {
    contains_2(a, b[0]) && contains_2(a, b[1])
}

/// Membership test for a three-element colour reading.
const fn contains_3(haystack: [u8; 3], needle: u8) -> bool {
    let mut i = 0;
    while i < 3 {
        if haystack[i] == needle {
            return true;
        }
        i += 1;
    }
    false
}

/// Membership test for a two-element colour reading.
const fn contains_2(haystack: [u8; 2], needle: u8) -> bool {
    let mut i = 0;
    while i < 2 {
        if haystack[i] == needle {
            return true;
        }
        i += 1;
    }
    false
}

/// Colour index of a facelet, from the face that owns it.
const fn colour_of(facelet: u8) -> u8 {
    FACE_COLOR_INDEX[(facelet / 9) as usize]
}

/// Corner piece colours, derived from [`CORNER_FACELETS`].
const fn piece_colors_3() -> [[u8; 3]; 8] {
    let mut out = [[0u8; 3]; 8];
    let mut i = 0;
    while i < 8 {
        let facelets = CORNER_FACELETS[i];
        out[i] = [
            colour_of(facelets[0]),
            colour_of(facelets[1]),
            colour_of(facelets[2]),
        ];
        i += 1;
    }
    out
}

/// Edge piece colours, derived from [`EDGE_FACELETS`].
const fn piece_colors_2() -> [[u8; 2]; 12] {
    let mut out = [[0u8; 2]; 12];
    let mut i = 0;
    while i < 12 {
        let facelets = EDGE_FACELETS[i];
        out[i] = [colour_of(facelets[0]), colour_of(facelets[1])];
        i += 1;
    }
    out
}

/// The solved facelet colours: a monochrome face per face in `U R F D L B`.
const fn solved_colors() -> [u8; 54] {
    let mut out = [0u8; 54];
    let mut i = 0;
    while i < 54 {
        out[i] = FACE_COLOR_INDEX[i / 9];
        i += 1;
    }
    out
}

/// Maps a colour index back to a [`Color`].
///
/// Callers only pass values produced by this module, i.e. `0..6`.
pub(crate) const fn color_from_index(index: u8) -> Color {
    match index {
        0 => Color::U,
        1 => Color::D,
        2 => Color::F,
        3 => Color::B,
        4 => Color::R,
        _ => Color::L,
    }
}

/// Facelet indices for each corner position, for the public facelet writer.
pub(crate) const fn corner_facelets(position: usize) -> [u8; 3] {
    CORNER_FACELETS[position]
}

/// Facelet indices for each edge position, for the public facelet writer.
pub(crate) const fn edge_facelets(position: usize) -> [u8; 2] {
    EDGE_FACELETS[position]
}

/// Colour indices for each corner piece, for the public facelet writer.
pub(crate) const fn corner_colors(piece: usize) -> [u8; 3] {
    CORNER_COLORS[piece]
}

/// Colour indices for each edge piece, for the public facelet writer.
pub(crate) const fn edge_colors(piece: usize) -> [u8; 2] {
    EDGE_COLORS[piece]
}
