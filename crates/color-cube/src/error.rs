//! Errors produced when converting facelets into a legal cube state.

/// Why a facelet array cannot describe a legal 3×3 colour cube.
///
/// The variants are checked in this order by
/// [`Cube::from_facelets`](crate::Cube::from_facelets): malformed colour
/// counts, centres, permutation completeness, twist, flip, parity. The enum is
/// `#[non_exhaustive]` so new variants can be added without a breaking change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CubeError {
    /// The 54 facelets do not contain exactly nine stickers of each colour.
    MalformedFacelets,
    /// The six centre stickers are not the fixed solved centres.
    InvalidCenter,
    /// A corner or edge piece is missing, unrecognisable, or duplicated.
    InvalidPermutation,
    /// The corner twist sum is not divisible by three.
    Twist,
    /// The edge flip sum is not even.
    Flip,
    /// Corner and edge permutation parities differ.
    Parity,
}

impl core::fmt::Display for CubeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let message = match self {
            Self::MalformedFacelets => "facelets do not contain nine of each colour",
            Self::InvalidCenter => "centre facelets do not match the fixed colour scheme",
            Self::InvalidPermutation => "corner or edge pieces are duplicated or missing",
            Self::Twist => "corner twist sum is not divisible by three",
            Self::Flip => "edge flip sum is not even",
            Self::Parity => "corner and edge permutation parities differ",
        };
        f.write_str(message)
    }
}
