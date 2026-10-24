//! Gate 1: 10,000 facelet ↔ cubie round-trips over pseudo-random valid states.
//!
//! The generator is local to this test and independent of M1's `from_seed`, as
//! the brief requires. It uses a hand-written SplitMix64 (D008 forbids
//! dependencies).

use color_cube::Cube;

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }
}

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

/// Builds a uniformly valid cubie state directly (the same construction M1
/// will use for `from_seed`, minus the public API).
fn random_valid(rng: &mut SplitMix64) -> Cube {
    let mut cp = [0u8, 1, 2, 3, 4, 5, 6, 7];
    for i in (1..8).rev() {
        let j = rng.below(i + 1);
        cp.swap(i, j);
    }

    let mut co = [0u8; 8];
    let mut twist = 0u32;
    for slot in co.iter_mut().take(7) {
        *slot = rng.below(3) as u8;
        twist += u32::from(*slot);
    }
    co[7] = ((3 - twist % 3) % 3) as u8;

    let mut ep = [0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
    for i in (1..12).rev() {
        let j = rng.below(i + 1);
        ep.swap(i, j);
    }

    let mut eo = [0u8; 12];
    let mut flip = 0u32;
    for slot in eo.iter_mut().take(11) {
        *slot = rng.below(2) as u8;
        flip += u32::from(*slot);
    }
    eo[11] = (flip % 2) as u8;

    if parity(&cp) != parity(&ep) {
        ep.swap(10, 11);
    }

    Cube { cp, co, ep, eo }
}

#[test]
fn ten_thousand_facelet_round_trips() {
    let mut rng = SplitMix64::new(0x00C0_FFEE_1234_5678);
    for case in 0..10_000 {
        let cube = random_valid(&mut rng);
        assert!(
            cube.is_valid(),
            "generator produced invalid state at {case}"
        );

        let facelets = cube.facelets();
        let restored = Cube::from_facelets(&facelets)
            .unwrap_or_else(|error| panic!("case {case} rejected: {error}"));

        assert_eq!(restored, cube, "round-trip mismatch at case {case}");
    }
}

#[test]
fn generated_states_are_not_solved() {
    let mut rng = SplitMix64::new(7);
    for _ in 0..1_000 {
        assert!(!random_valid(&mut rng).is_solved());
    }
}
