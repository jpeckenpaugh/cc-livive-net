//! Deterministic Thistlethwaite solver (D019, D020).
//!
//! The group chain is `G0 -> G1 -> G2 -> G3 -> solved`:
//!
//! | Phase | Allowed moves | Coordinate |
//! |---|---|---|
//! | 1 | all 18 | `flip * 495 + slice` |
//! | 2 | `G1` (14) | `twist` |
//! | 3 | `G2` (10) | corner-tetrad rank × U/D-edge-orbit rank |
//! | 4 | `G3` (6) | `cp_rank` and `ep_full = ep8 * 24 + sp` |
//!
//! Phases 1–3 use exact BFS distance tables and deterministic greedy descent.
//! Phase 4 uses IDA* with `h = max(cp_dist, ep_full_dist)`, bounded at 15.
//! Every returned solution is replayed on a copy of the input before it is
//! handed back.

use std::collections::VecDeque;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use color_cube::{Cube, Move, N_MOVES};

use crate::coords::{self, N_CP, N_EP8, N_SP, SLICE_SOLVED};
use crate::tables::{self, TableBundle, TablesError};

/// Phase 1 rows: `2^11`.
pub const PHASE1_ROWS: usize = coords::N_FLIP;
/// Phase 2 rows: `3^7 * 495`.
pub const PHASE2_ROWS: usize = coords::N_TWIST * coords::N_SLICE;
/// Phase 3 rows: `8! * 70` (`cp_rank` × U/D edge-orbit pattern).
pub const PHASE3_ROWS: usize = N_CP * 70;
/// Phase 4 corner rows: `8!`.
pub const PHASE4_CP_ROWS: usize = N_CP;
/// Phase 4 edge rows: `8! * 4!`.
pub const PHASE4_EP_ROWS: usize = N_EP8 * N_SP;

/// The fourteen moves that keep a `G1` cube inside `G1`.
pub const G1_MOVES: [usize; 14] = [0, 1, 2, 3, 4, 5, 7, 9, 10, 11, 12, 13, 14, 16];
/// The ten moves that keep a `G2` cube inside `G2` (ascending index).
pub const G2_MOVES: [usize; 10] = [0, 1, 2, 4, 7, 9, 10, 11, 13, 16];
/// The six half turns that generate `G3` (ascending index).
pub const G3_MOVES: [usize; 6] = [1, 4, 7, 10, 13, 16];

/// Bundle magic for the distance tables, `b"CCTH"`.
pub const THISTLE_MAGIC: [u8; 4] = *b"CCTH";
/// Distance-bundle format version.
pub const THISTLE_VERSION: u16 = 1;
/// Distance-bundle file name.
pub const THISTLE_BUNDLE_FILE: &str = "cc-thistlethwaite-v1.bin";

/// Frozen parameters hashed into the distance bundle.
pub const THISTLE_CONVENTIONS: &str = concat!(
    "cc-livive-net thistlethwaite distance tables\n",
    "format=1\n",
    "phase1=flip;goal=0;moves=all18\n",
    "phase2=twist*495+slice;goal=0*495+494;",
    "moves=G1=0,1,2,3,4,5,7,9,10,11,12,13,14,16\n",
    "phase3=cp_rank*70+edge_orbit_rank;",
    "corner_tetrads=A={0,2,5,7},B={1,3,4,6};",
    "edge_orbits=A={0,2,6,4},B={1,3,7,5};goal=cp in G3-corner-subgroup,edge=49;",
    "moves=G2=0,1,2,4,7,9,10,11,13,16\n",
    "phase4_cp=lehmer8;phase4_ep=ep8*24+sp;",
    "moves=G3=1,4,7,10,13,16;rows=40320,967680\n",
    "ids=phase1:1:u8,phase2:2:u8,phase3:3:u8,phase4_cp:4:u8,phase4_ep:5:u8\n",
);

/// Table ids in payload order.
pub const ID_PHASE1: u32 = 1;
/// Table id for the phase-2 twist distance table.
pub const ID_PHASE2: u32 = 2;
/// Table id for the phase-3 quotient distance table.
pub const ID_PHASE3: u32 = 3;
/// Table id for the phase-4 corner distance table.
pub const ID_PHASE4_CP: u32 = 4;
/// Table id for the phase-4 full-edge distance table.
pub const ID_PHASE4_EP: u32 = 5;

pub use crate::solver::{Limits, Solution, SolveError, Solver};

/// The five phase distance tables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DistanceBundle {
    /// Phase-1 `(flip, slice)` distances.
    pub phase1: Vec<u8>,
    /// Phase-2 twist distances.
    pub phase2: Vec<u8>,
    /// Phase-3 quotient distances.
    pub phase3: Vec<u8>,
    /// Phase-4 corner distances.
    pub phase4_cp: Vec<u8>,
    /// Phase-4 full-edge distances.
    pub phase4_ep: Vec<u8>,
}

impl DistanceBundle {
    /// Byte size of each table in id order.
    #[must_use]
    pub fn payload_sizes(&self) -> [usize; 5] {
        [
            self.phase1.len(),
            self.phase2.len(),
            self.phase3.len(),
            self.phase4_cp.len(),
            self.phase4_ep.len(),
        ]
    }

    /// Encodes the bundle in the `CCTH` version-1 layout.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let payloads = [
            self.phase1.clone(),
            self.phase2.clone(),
            self.phase3.clone(),
            self.phase4_cp.clone(),
            self.phase4_ep.clone(),
        ];
        tables::encode_bundle(
            THISTLE_MAGIC,
            THISTLE_VERSION,
            THISTLE_CONVENTIONS,
            &distance_entries(),
            &payloads,
        )
    }

    /// Writes the bundle atomically into `directory`.
    ///
    /// # Errors
    ///
    /// Returns the underlying I/O error.
    pub fn write(&self, directory: &Path) -> std::io::Result<PathBuf> {
        tables::write_bundle_bytes(directory, THISTLE_BUNDLE_FILE, &self.encode())
    }

    /// Loads the bundle from `directory`.
    ///
    /// # Errors
    ///
    /// [`TablesError::Missing`] when absent, [`TablesError::TableCorrupt`] for
    /// any structural or checksum failure.
    pub fn load(directory: &Path) -> Result<Self, TablesError> {
        let data = tables::read_bundle_bytes(directory, THISTLE_BUNDLE_FILE)?;
        Self::decode(&data)
    }

    /// Decodes a `CCTH` version-1 bundle.
    ///
    /// # Errors
    ///
    /// [`TablesError::TableCorrupt`] on any mismatch.
    pub fn decode(data: &[u8]) -> Result<Self, TablesError> {
        let payloads = tables::decode_bundle(
            data,
            THISTLE_MAGIC,
            THISTLE_VERSION,
            THISTLE_CONVENTIONS,
            &distance_entries(),
        )?;
        Ok(Self {
            phase1: payloads[0].clone(),
            phase2: payloads[1].clone(),
            phase3: payloads[2].clone(),
            phase4_cp: payloads[3].clone(),
            phase4_ep: payloads[4].clone(),
        })
    }
}

fn distance_entries() -> [(u32, u32, u32, u8); 5] {
    [
        (ID_PHASE1, PHASE1_ROWS as u32, 1, 1),
        (ID_PHASE2, PHASE2_ROWS as u32, 1, 1),
        (ID_PHASE3, PHASE3_ROWS as u32, 1, 1),
        (ID_PHASE4_CP, PHASE4_CP_ROWS as u32, 1, 1),
        (ID_PHASE4_EP, PHASE4_EP_ROWS as u32, 1, 1),
    ]
}

/// Per-table generation time, in table id order.
#[derive(Clone, Copy, Debug)]
pub struct DistanceTiming {
    /// Table id.
    pub id: u32,
    /// Wall time spent generating the table.
    pub elapsed: Duration,
}

/// Measured maxima of the generated distance tables.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DistanceReport {
    /// Largest phase-1 distance.
    pub phase1_max: u8,
    /// Largest phase-2 distance.
    pub phase2_max: u8,
    /// Largest phase-3 distance.
    pub phase3_max: u8,
    /// Largest finite phase-4 corner distance.
    pub phase4_cp_max: u8,
    /// Largest finite phase-4 full-edge distance.
    pub phase4_ep_max: u8,
}

/// A load failure with the bundle name preserved for diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableLoadError {
    /// File name of the failing bundle.
    pub bundle: &'static str,
    /// Underlying loader error.
    pub error: TablesError,
}

impl fmt::Display for TableLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to load {}: {}", self.bundle, self.error)
    }
}

impl std::error::Error for TableLoadError {}

/// The Thistlethwaite solver with lazily loaded tables.
#[derive(Debug)]
pub struct Thistlethwaite {
    directory: PathBuf,
    loaded: OnceLock<Result<Arc<LoadedTables>, TableLoadError>>,
}

#[derive(Debug)]
struct LoadedTables {
    moves: TableBundle,
    distances: DistanceBundle,
}

impl Default for Thistlethwaite {
    fn default() -> Self {
        Self::new()
    }
}

impl Thistlethwaite {
    /// Creates a solver that loads both bundles from the default directory.
    ///
    /// Loading is deferred to the first access; a missing or corrupt bundle is
    /// reported by [`solve`](Solver::solve) as [`SolveError::TablesMissing`].
    #[must_use]
    pub fn new() -> Self {
        Self::from_directory(tables::default_directory())
    }

    /// Creates a solver that loads both bundles from `directory`.
    #[must_use]
    pub fn from_directory(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            loaded: OnceLock::new(),
        }
    }

    /// Generates both bundles into `directory`, then creates a solver.
    ///
    /// # Errors
    ///
    /// Returns a [`TableLoadError`] if either bundle cannot be written or
    /// reloaded.
    pub fn generate(directory: impl AsRef<Path>) -> Result<Self, TableLoadError> {
        let directory = directory.as_ref();
        let move_tables = tables::generate();
        tables::write(directory, &move_tables).map_err(|_| TableLoadError {
            bundle: tables::BUNDLE_FILE,
            error: TablesError::TableCorrupt,
        })?;
        let (distances, _, _) = generate_distance_tables(&move_tables);
        distances.write(directory).map_err(|_| TableLoadError {
            bundle: THISTLE_BUNDLE_FILE,
            error: TablesError::TableCorrupt,
        })?;
        Ok(Self::from_directory(directory))
    }

    /// Returns the load error, if the bundles could not be loaded.
    #[must_use]
    pub fn load_error(&self) -> Option<TableLoadError> {
        self.loaded().err()
    }

    fn loaded(&self) -> Result<&Arc<LoadedTables>, TableLoadError> {
        self.loaded
            .get_or_init(|| load_tables(&self.directory))
            .as_ref()
            .map_err(|error| *error)
    }

    /// Solves `cube` and returns the per-phase move counts.
    ///
    /// # Errors
    ///
    /// Returns the matching [`SolveError`].
    pub fn solve_with_phases(&self, cube: &Cube) -> Result<(Solution, [usize; 4]), SolveError> {
        let outcome = self.solve_inner(cube, Limits::default())?;
        Ok((outcome.solution, outcome.phases))
    }

    /// Solves `cube` and also returns the phase-4 IDA* node count.
    ///
    /// # Errors
    ///
    /// Returns the matching [`SolveError`].
    pub fn solve_with_stats(
        &self,
        cube: &Cube,
        limits: Limits,
    ) -> Result<(Solution, [usize; 4], u64), SolveError> {
        let outcome = self.solve_inner(cube, limits)?;
        Ok((outcome.solution, outcome.phases, outcome.phase4_nodes))
    }

    fn solve_inner(&self, cube: &Cube, limits: Limits) -> Result<SolveOutcome, SolveError> {
        if !cube.is_valid() {
            return Err(SolveError::InvalidState);
        }
        if cube.is_solved() {
            return Err(SolveError::AlreadySolved);
        }
        let loaded = self.loaded().map_err(|_| SolveError::TablesMissing)?;
        let started = Instant::now();
        check_time(limits, started)?;

        let mut current = *cube;
        let mut moves = Vec::with_capacity(45);
        let mut phases = [0usize; 4];

        // Phase 1: all 18 moves.
        greedy_phase1(
            &mut current,
            &loaded.moves,
            &loaded.distances.phase1,
            &mut moves,
        )?;
        phases[0] = moves.len();
        debug_assert_eq!(coords::flip(&current), 0);
        check_time(limits, started)?;

        // Phase 2: G1 moves.
        let phase2_start = moves.len();
        greedy_phase2(
            &mut current,
            &loaded.moves,
            &loaded.distances.phase2,
            &mut moves,
        )?;
        phases[1] = moves.len() - phase2_start;
        debug_assert_eq!(coords::twist(&current), 0);
        debug_assert_eq!(coords::slice(&current), SLICE_SOLVED);
        check_time(limits, started)?;

        // Phase 3: G2 moves.
        let phase3_start = moves.len();
        greedy_phase3(&mut current, &loaded.distances.phase3, &mut moves)?;
        phases[2] = moves.len() - phase3_start;
        if !is_g3(&current) {
            return Err(SolveError::NoSolutionWithinLimits);
        }
        check_time(limits, started)?;

        // Phase 4: IDA* inside G3.
        let (phase4_moves, phase4_nodes) = match phase4(&current, &loaded.moves, &loaded.distances)
        {
            Some(result) => result,
            None => return Err(SolveError::NoSolutionWithinLimits),
        };
        let phase4_start = moves.len();
        moves.extend(phase4_moves);
        phases[3] = moves.len() - phase4_start;

        if phases[0] > 7 || phases[1] > 10 || phases[2] > 13 || phases[3] > 15 {
            return Err(SolveError::NoSolutionWithinLimits);
        }

        if let Some(max_length) = limits.max_length {
            if moves.len() > usize::from(max_length) {
                return Err(SolveError::NoSolutionWithinLimits);
            }
        }

        let mut replay = *cube;
        replay.apply_all(&moves);
        if !replay.is_solved() {
            return Err(SolveError::NoSolutionWithinLimits);
        }

        Ok(SolveOutcome {
            solution: Solution {
                moves,
                optimal: false,
                phases: None,
            },
            phases,
            phase4_nodes,
        })
    }
}

struct SolveOutcome {
    solution: Solution,
    phases: [usize; 4],
    phase4_nodes: u64,
}

impl Solver for Thistlethwaite {
    fn solve(&self, cube: &Cube, limits: Limits) -> Result<Solution, SolveError> {
        self.solve_inner(cube, limits)
            .map(|outcome| outcome.solution)
    }
}

fn check_time(limits: Limits, started: Instant) -> Result<(), SolveError> {
    if let Some(time_ms) = limits.time_ms {
        if started.elapsed().as_millis() >= u128::from(time_ms) {
            return Err(SolveError::Timeout);
        }
    }
    Ok(())
}

fn load_tables(directory: &Path) -> Result<Arc<LoadedTables>, TableLoadError> {
    let moves = tables::load(directory).map_err(|error| TableLoadError {
        bundle: tables::BUNDLE_FILE,
        error,
    })?;
    let distances = DistanceBundle::load(directory).map_err(|error| TableLoadError {
        bundle: THISTLE_BUNDLE_FILE,
        error,
    })?;
    Ok(Arc::new(LoadedTables { moves, distances }))
}

fn greedy_phase1(
    cube: &mut Cube,
    moves: &TableBundle,
    distance: &[u8],
    out: &mut Vec<Move>,
) -> Result<(), SolveError> {
    let start = out.len();
    loop {
        if out.len() - start > 7 {
            return Err(SolveError::NoSolutionWithinLimits);
        }
        let flip = usize::from(coords::flip(cube));
        let d = distance[flip];
        if d == 0 {
            return Ok(());
        }
        if d == u8::MAX {
            return Err(SolveError::NoSolutionWithinLimits);
        }
        let mut chosen = None;
        for m in 0..N_MOVES {
            let next = usize::from(moves.move_flip[flip * N_MOVES + m]);
            if distance[next] < d {
                chosen = Some(m as u8);
                break;
            }
        }
        let m = chosen.ok_or(SolveError::NoSolutionWithinLimits)?;
        cube.apply(Move(m));
        out.push(Move(m));
    }
}

fn greedy_phase2(
    cube: &mut Cube,
    moves: &TableBundle,
    distance: &[u8],
    out: &mut Vec<Move>,
) -> Result<(), SolveError> {
    let start = out.len();
    loop {
        if out.len() - start > 10 {
            return Err(SolveError::NoSolutionWithinLimits);
        }
        let twist = usize::from(coords::twist(cube));
        let slice = usize::from(coords::slice(cube));
        let index = twist * coords::N_SLICE + slice;
        let d = distance[index];
        if d == 0 {
            return Ok(());
        }
        if d == u8::MAX {
            return Err(SolveError::NoSolutionWithinLimits);
        }
        let mut chosen = None;
        for &m in &G1_MOVES {
            let next_twist = usize::from(moves.move_twist[twist * N_MOVES + m]);
            let next_slice = usize::from(moves.move_slice[slice * N_MOVES + m]);
            let next = next_twist * coords::N_SLICE + next_slice;
            if distance[next] < d {
                chosen = Some(m as u8);
                break;
            }
        }
        let m = chosen.ok_or(SolveError::NoSolutionWithinLimits)?;
        cube.apply(Move(m));
        out.push(Move(m));
    }
}

fn greedy_phase3(cube: &mut Cube, distance: &[u8], out: &mut Vec<Move>) -> Result<(), SolveError> {
    let start = out.len();
    loop {
        if out.len() - start > 13 {
            return Err(SolveError::NoSolutionWithinLimits);
        }
        let quotient = quotient_index(cube);
        let d = distance[quotient];
        if d == 0 {
            return Ok(());
        }
        if d == u8::MAX {
            return Err(SolveError::NoSolutionWithinLimits);
        }
        let mut chosen = None;
        for &m in &G2_MOVES {
            let mut next = *cube;
            next.apply(Move(m as u8));
            let next_quotient = quotient_index(&next);
            if distance[next_quotient] < d {
                chosen = Some((m as u8, next));
                break;
            }
        }
        let (m, next) = chosen.ok_or(SolveError::NoSolutionWithinLimits)?;
        *cube = next;
        out.push(Move(m));
    }
}

/// The phase-3 coordinate, `cp_rank * 70 + edge_pattern`.
#[must_use]
pub fn quotient_index(cube: &Cube) -> usize {
    usize::from(coords::cp_rank(cube)) * 70 + usize::from(edge_pattern(cube))
}

/// Whether `cube` is in `G3`.
#[must_use]
pub fn is_g3(cube: &Cube) -> bool {
    cube.co.iter().all(|&orientation| orientation == 0)
        && cube.eo.iter().all(|&orientation| orientation == 0)
        && coords::slice(cube) == SLICE_SOLVED
        && corner_pattern(cube) == GOAL_CORNER_PATTERN
        && edge_pattern(cube) == GOAL_EDGE_PATTERN
        && in_corner_square_subgroup(&cube.cp)
}

fn in_corner_square_subgroup(cp: &[u8; 8]) -> bool {
    static SUBGROUP: OnceLock<Vec<[u8; 8]>> = OnceLock::new();
    let subgroup = SUBGROUP.get_or_init(|| {
        let mut seen = vec![Cube::SOLVED.cp];
        let mut queue = VecDeque::from([Cube::SOLVED]);
        while let Some(cube) = queue.pop_front() {
            for &m in &G3_MOVES {
                let mut next = cube;
                next.apply(Move(m as u8));
                if !seen.contains(&next.cp) {
                    seen.push(next.cp);
                    queue.push_back(next);
                }
            }
        }
        assert_eq!(seen.len(), 96, "G3 corner subgroup size");
        seen
    });
    subgroup.contains(cp)
}

#[allow(dead_code)]
const GOAL_CORNER_MASK: u8 = 0b0101_1010;
#[allow(dead_code)]
const GOAL_EDGE_MASK: u8 = 0b1010_1010;
/// Rank of [`GOAL_CORNER_MASK`] under the combinadic pattern ranking.
pub const GOAL_CORNER_PATTERN: u8 = 23;
/// Rank of [`GOAL_EDGE_MASK`] under the combinadic pattern ranking.
pub const GOAL_EDGE_PATTERN: u8 = 49;
const CORNER_TETRAD_B: [bool; 8] = [false, true, false, true, true, false, true, false];
const EDGE_ORBIT_B: [bool; 12] = [
    false, true, false, true, false, true, false, true, false, false, false, false,
];

fn corner_pattern(cube: &Cube) -> u8 {
    let mut mask = 0u8;
    for (position, &piece) in cube.cp.iter().enumerate() {
        if CORNER_TETRAD_B[piece as usize] {
            mask |= 1 << position;
        }
    }
    pattern_rank(mask)
}

fn edge_pattern(cube: &Cube) -> u8 {
    let mut mask = 0u8;
    for (position, &piece) in cube.ep[..8].iter().enumerate() {
        if EDGE_ORBIT_B[piece as usize] {
            mask |= 1 << position;
        }
    }
    pattern_rank(mask)
}

fn pattern_rank(mask: u8) -> u8 {
    debug_assert_eq!(mask.count_ones(), 4);
    let mut rank = 0u32;
    let mut k = 1u32;
    for position in 0..8u32 {
        if mask & (1 << position) != 0 {
            rank += binomial(position, k);
            k += 1;
        }
    }
    rank as u8
}

fn binomial(n: u32, k: u32) -> u32 {
    if k > n {
        return 0;
    }
    let mut result = 1u32;
    for i in 0..k {
        result = result * (n - i) / (i + 1);
    }
    result
}

fn phase4(
    cube: &Cube,
    moves: &TableBundle,
    distances: &DistanceBundle,
) -> Option<(Vec<Move>, u64)> {
    let cp = usize::from(coords::cp_rank(cube));
    let ep = usize::from(coords::ep8_rank(cube)) * N_SP + usize::from(coords::sp_rank(cube));
    let h = phase4_heuristic(cp, ep, distances);
    if h > 15 {
        return None;
    }
    let mut nodes = 0u64;
    for bound in h..=15 {
        let mut path = Vec::with_capacity(usize::from(bound));
        if search_phase4(
            cp,
            ep,
            0,
            bound,
            u8::MAX,
            moves,
            distances,
            &mut path,
            &mut nodes,
        ) {
            let path = path.into_iter().map(Move).collect();
            return Some((path, nodes));
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn search_phase4(
    cp: usize,
    ep: usize,
    depth: u8,
    bound: u8,
    previous_face: u8,
    moves: &TableBundle,
    distances: &DistanceBundle,
    path: &mut Vec<u8>,
    nodes: &mut u64,
) -> bool {
    *nodes += 1;
    let h = phase4_heuristic(cp, ep, distances);
    if h == u8::MAX || u16::from(depth) + u16::from(h) > u16::from(bound) {
        return false;
    }
    if h == 0 {
        return true;
    }
    let ep8 = ep / N_SP;
    let sp = ep % N_SP;
    for &m in &G3_MOVES {
        let face = (m / 3) as u8;
        if face == previous_face {
            continue;
        }
        let next_cp = usize::from(moves.move_cp[cp * N_MOVES + m]);
        let next_ep8 = usize::from(moves.move_ep8[ep8 * N_MOVES + m]);
        let next_sp = usize::from(moves.move_sp[sp * N_MOVES + m]);
        if next_ep8 == usize::from(u16::MAX) || next_sp == usize::from(u8::MAX) {
            continue;
        }
        path.push(m as u8);
        if search_phase4(
            next_cp,
            next_ep8 * N_SP + next_sp,
            depth + 1,
            bound,
            face,
            moves,
            distances,
            path,
            nodes,
        ) {
            return true;
        }
        path.pop();
    }
    false
}

fn phase4_heuristic(cp: usize, ep: usize, distances: &DistanceBundle) -> u8 {
    distances.phase4_cp[cp].max(distances.phase4_ep[ep])
}

/// Generates the five distance tables from an existing move-table bundle.
///
/// # Panics
///
/// Panics if a phase table does not cover its whole coordinate space or if a
/// measured diameter exceeds its published bound (7/10/13). That is a
/// normalisation or generation bug, never a runtime solver outcome.
#[must_use]
pub fn generate_distance_tables(
    move_tables: &TableBundle,
) -> (DistanceBundle, [DistanceTiming; 5], DistanceReport) {
    let started = Instant::now();
    let (phase1, phase1_max) = generate_phase1(move_tables);
    let t0 = started.elapsed();

    let started = Instant::now();
    let (phase2, phase2_max) = generate_phase2(move_tables);
    let t1 = started.elapsed();

    let started = Instant::now();
    let (phase3, phase3_max) = generate_phase3(move_tables);
    let t2 = started.elapsed();

    let started = Instant::now();
    let (phase4_cp, phase4_cp_max) = generate_phase4_cp(move_tables);
    let t3 = started.elapsed();

    let started = Instant::now();
    let (phase4_ep, phase4_ep_max) = generate_phase4_ep(move_tables);
    let t4 = started.elapsed();

    assert!(phase1_max <= 7, "phase 1 diameter {phase1_max} exceeds 7");
    assert!(phase2_max <= 10, "phase 2 diameter {phase2_max} exceeds 10");
    assert!(phase3_max <= 13, "phase 3 diameter {phase3_max} exceeds 13");

    let bundle = DistanceBundle {
        phase1,
        phase2,
        phase3,
        phase4_cp,
        phase4_ep,
    };
    let timings = [
        DistanceTiming {
            id: ID_PHASE1,
            elapsed: t0,
        },
        DistanceTiming {
            id: ID_PHASE2,
            elapsed: t1,
        },
        DistanceTiming {
            id: ID_PHASE3,
            elapsed: t2,
        },
        DistanceTiming {
            id: ID_PHASE4_CP,
            elapsed: t3,
        },
        DistanceTiming {
            id: ID_PHASE4_EP,
            elapsed: t4,
        },
    ];
    let report = DistanceReport {
        phase1_max,
        phase2_max,
        phase3_max,
        phase4_cp_max,
        phase4_ep_max,
    };
    (bundle, timings, report)
}

fn generate_phase1(move_tables: &TableBundle) -> (Vec<u8>, u8) {
    let mut distance = vec![u8::MAX; PHASE1_ROWS];
    distance[0] = 0;
    let mut queue = VecDeque::with_capacity(PHASE1_ROWS);
    queue.push_back(0usize);
    while let Some(flip) = queue.pop_front() {
        let next_distance = distance[flip] + 1;
        for m in 0..N_MOVES {
            let next = usize::from(move_tables.move_flip[flip * N_MOVES + m]);
            if distance[next] == u8::MAX {
                distance[next] = next_distance;
                queue.push_back(next);
            }
        }
    }
    let max = exhaustive_max(&distance, "phase 1");
    (distance, max)
}

fn generate_phase2(move_tables: &TableBundle) -> (Vec<u8>, u8) {
    let mut distance = vec![u8::MAX; PHASE2_ROWS];
    let goal = usize::from(SLICE_SOLVED);
    distance[goal] = 0;
    let mut queue = VecDeque::with_capacity(PHASE2_ROWS);
    queue.push_back(goal);
    while let Some(index) = queue.pop_front() {
        let next_distance = distance[index] + 1;
        let twist = index / coords::N_SLICE;
        let slice = index % coords::N_SLICE;
        for &m in &G1_MOVES {
            let next_twist = usize::from(move_tables.move_twist[twist * N_MOVES + m]);
            let next_slice = usize::from(move_tables.move_slice[slice * N_MOVES + m]);
            let next = next_twist * coords::N_SLICE + next_slice;
            if distance[next] == u8::MAX {
                distance[next] = next_distance;
                queue.push_back(next);
            }
        }
    }
    let max = exhaustive_max(&distance, "phase 2");
    (distance, max)
}

#[allow(clippy::needless_range_loop)]
fn generate_phase3(move_tables: &TableBundle) -> (Vec<u8>, u8) {
    let ep_pattern = ep8_patterns();
    let mut pattern_next = [[0u8; 10]; 70];
    let mut representatives = [0usize; 70];
    for (ep8, &pattern) in ep_pattern.iter().enumerate() {
        representatives[usize::from(pattern)] = ep8;
    }
    for (pattern, &ep8) in representatives.iter().enumerate() {
        for (slot, &m) in G2_MOVES.iter().enumerate() {
            pattern_next[pattern][slot] =
                ep_pattern[usize::from(move_tables.move_ep8[ep8 * N_MOVES + m])];
        }
    }

    let goal_cp = corner_goal_set(move_tables);
    let mut distance = vec![u8::MAX; PHASE3_ROWS];
    let mut queue = VecDeque::new();
    for cp in 0..N_CP {
        if goal_cp[cp] {
            let index = cp * 70 + usize::from(GOAL_EDGE_PATTERN);
            distance[index] = 0;
            queue.push_back(index);
        }
    }
    while let Some(index) = queue.pop_front() {
        let next_distance = distance[index] + 1;
        let cp = index / 70;
        let pattern = index % 70;
        for (slot, &m) in G2_MOVES.iter().enumerate() {
            let next_cp = usize::from(move_tables.move_cp[cp * N_MOVES + m]);
            let next_pattern = usize::from(pattern_next[pattern][slot]);
            let next = next_cp * 70 + next_pattern;
            if distance[next] == u8::MAX {
                distance[next] = next_distance;
                queue.push_back(next);
            }
        }
    }
    let max = exhaustive_max(&distance, "phase 3");
    (distance, max)
}

#[allow(clippy::needless_range_loop)]
fn ep8_patterns() -> Vec<u8> {
    let mut patterns = vec![0u8; N_EP8];
    let mut cube = Cube::SOLVED;
    for ep8 in 0..N_EP8 {
        coords::set_ep8(&mut cube, ep8 as u16);
        patterns[ep8] = edge_pattern(&cube);
    }
    patterns
}

fn corner_goal_set(move_tables: &TableBundle) -> Vec<bool> {
    let mut goal = vec![false; N_CP];
    goal[0] = true;
    let mut queue = VecDeque::new();
    queue.push_back(0usize);
    while let Some(cp) = queue.pop_front() {
        for &m in &G3_MOVES {
            let next = usize::from(move_tables.move_cp[cp * N_MOVES + m]);
            if !goal[next] {
                goal[next] = true;
                queue.push_back(next);
            }
        }
    }
    assert_eq!(
        goal.iter().filter(|&&b| b).count(),
        96,
        "G3 corner subgroup size"
    );
    goal
}
fn generate_phase4_cp(move_tables: &TableBundle) -> (Vec<u8>, u8) {
    let mut distance = vec![u8::MAX; PHASE4_CP_ROWS];
    distance[0] = 0;
    let mut queue = VecDeque::new();
    queue.push_back(0usize);
    while let Some(cp) = queue.pop_front() {
        let next_distance = distance[cp] + 1;
        for &m in &G3_MOVES {
            let next = usize::from(move_tables.move_cp[cp * N_MOVES + m]);
            if distance[next] == u8::MAX {
                distance[next] = next_distance;
                queue.push_back(next);
            }
        }
    }
    let max = finite_max(&distance);
    (distance, max)
}

fn generate_phase4_ep(move_tables: &TableBundle) -> (Vec<u8>, u8) {
    let mut distance = vec![u8::MAX; PHASE4_EP_ROWS];
    distance[0] = 0;
    let mut queue = VecDeque::new();
    queue.push_back(0usize);
    while let Some(ep) = queue.pop_front() {
        let next_distance = distance[ep] + 1;
        let ep8 = ep / N_SP;
        let sp = ep % N_SP;
        for &m in &G3_MOVES {
            let next_ep8 = usize::from(move_tables.move_ep8[ep8 * N_MOVES + m]);
            let next_sp = usize::from(move_tables.move_sp[sp * N_MOVES + m]);
            let next = next_ep8 * N_SP + next_sp;
            if distance[next] == u8::MAX {
                distance[next] = next_distance;
                queue.push_back(next);
            }
        }
    }
    let max = finite_max(&distance);
    (distance, max)
}

fn exhaustive_max(distance: &[u8], name: &str) -> u8 {
    let max = finite_max(distance);
    assert!(
        distance.iter().all(|&d| d != u8::MAX),
        "{name} BFS did not cover the whole coordinate space"
    );
    max
}

fn finite_max(distance: &[u8]) -> u8 {
    distance
        .iter()
        .copied()
        .filter(|&d| d != u8::MAX)
        .max()
        .unwrap_or(0)
}

/// Exhaustively verifies the `G3` diameter.
///
/// Returns `(reachable_states, diameter)`. The measured reachable state count
/// is `663_552`; the diameter is asserted to be at most 15 by callers.
#[must_use]
pub fn verify_g3_diameter() -> (usize, u8) {
    const CP_STATES: usize = 576;
    const EP_STATES: usize = 576 * 24;
    const TOTAL: usize = CP_STATES * EP_STATES;

    let corner_maps = corner_state_maps();
    let edge_maps = edge_state_maps();

    let mut distance = vec![u8::MAX; TOTAL];
    distance[0] = 0;
    let mut queue = VecDeque::with_capacity(3_981_312);
    queue.push_back(0usize);
    let mut reached = 1usize;
    while let Some(index) = queue.pop_front() {
        let next_distance = distance[index] + 1;
        let cp_state = index / EP_STATES;
        let ep_state = index % EP_STATES;
        for slot in 0..G3_MOVES.len() {
            let next_cp = usize::from(corner_maps[slot][cp_state]);
            let next_ep = usize::from(edge_maps[slot][ep_state]);
            let next = next_cp * EP_STATES + next_ep;
            if distance[next] == u8::MAX {
                distance[next] = next_distance;
                reached += 1;
                queue.push_back(next);
            }
        }
    }
    (reached, finite_max(&distance))
}

#[allow(clippy::needless_range_loop)]
fn corner_state_maps() -> [[u16; 576]; 6] {
    let positions_a = [0usize, 2, 5, 7];
    let positions_b = [1usize, 3, 4, 6];
    let mut maps = [[0u16; 576]; 6];
    for state in 0..576 {
        let mut cube = Cube::SOLVED;
        let rank_a = state / 24;
        let rank_b = state % 24;
        let perm_a = permutation4(rank_a as u16);
        let perm_b = permutation4(rank_b as u16);
        for (slot, &position) in positions_a.iter().enumerate() {
            cube.cp[position] = positions_a[perm_a[slot] as usize] as u8;
        }
        for (slot, &position) in positions_b.iter().enumerate() {
            cube.cp[position] = positions_b[perm_b[slot] as usize] as u8;
        }
        for (slot, &m) in G3_MOVES.iter().enumerate() {
            let mut next = cube;
            next.apply(Move(m as u8));
            maps[slot][state] = corner_state(&next);
        }
    }
    maps
}

#[allow(clippy::needless_range_loop)]
fn edge_state_maps() -> [[u16; 13_824]; 6] {
    let positions_a = [0usize, 2, 6, 4];
    let positions_b = [1usize, 3, 7, 5];
    let mut maps = [[0u16; 13_824]; 6];
    for state in 0..13_824 {
        let ep8_state = state / 24;
        let sp = state % 24;
        let rank_a = ep8_state / 24;
        let rank_b = ep8_state % 24;
        let perm_a = permutation4(rank_a as u16);
        let perm_b = permutation4(rank_b as u16);
        let sp_perm = permutation4(sp as u16);
        let mut cube = Cube::SOLVED;
        for (slot, &position) in positions_a.iter().enumerate() {
            cube.ep[position] = positions_a[perm_a[slot] as usize] as u8;
        }
        for (slot, &position) in positions_b.iter().enumerate() {
            cube.ep[position] = positions_b[perm_b[slot] as usize] as u8;
        }
        for (slot, &position) in [8usize, 9, 10, 11].iter().enumerate() {
            cube.ep[position] = 8 + sp_perm[slot];
        }
        for (slot, &m) in G3_MOVES.iter().enumerate() {
            let mut next = cube;
            next.apply(Move(m as u8));
            maps[slot][state] = edge_state(&next);
        }
    }
    maps
}

fn corner_state(cube: &Cube) -> u16 {
    let positions_a = [0usize, 2, 5, 7];
    let positions_b = [1usize, 3, 4, 6];
    let mut perm_a = [0u8; 4];
    let mut perm_b = [0u8; 4];
    for (slot, &position) in positions_a.iter().enumerate() {
        let piece = cube.cp[position] as usize;
        let index = positions_a
            .iter()
            .position(|&p| p == piece)
            .expect("corner outside its G3 tetrad");
        perm_a[slot] = index as u8;
    }
    for (slot, &position) in positions_b.iter().enumerate() {
        let piece = cube.cp[position] as usize;
        let index = positions_b
            .iter()
            .position(|&p| p == piece)
            .expect("corner outside its G3 tetrad");
        perm_b[slot] = index as u8;
    }
    (lehmer4(&perm_a) * 24 + lehmer4(&perm_b)) as u16
}

fn edge_state(cube: &Cube) -> u16 {
    let positions_a = [0usize, 2, 6, 4];
    let positions_b = [1usize, 3, 7, 5];
    let mut perm_a = [0u8; 4];
    let mut perm_b = [0u8; 4];
    for (slot, &position) in positions_a.iter().enumerate() {
        let piece = cube.ep[position] as usize;
        let index = positions_a
            .iter()
            .position(|&p| p == piece)
            .expect("edge outside its G3 orbit");
        perm_a[slot] = index as u8;
    }
    for (slot, &position) in positions_b.iter().enumerate() {
        let piece = cube.ep[position] as usize;
        let index = positions_b
            .iter()
            .position(|&p| p == piece)
            .expect("edge outside its G3 orbit");
        perm_b[slot] = index as u8;
    }
    let mut sp = [0u8; 4];
    for (slot, &piece) in cube.ep[8..12].iter().enumerate() {
        sp[slot] = piece - 8;
    }
    let ep8_state = lehmer4(&perm_a) * 24 + lehmer4(&perm_b);
    (ep8_state * 24 + lehmer4(&sp)) as u16
}

fn permutation4(rank: u16) -> [u8; 4] {
    let mut available = [0u8, 1, 2, 3];
    let mut out = [0u8; 4];
    let mut rest = u32::from(rank);
    let mut len = 4usize;
    for slot in &mut out {
        let factorial = [6u32, 2, 1, 1][4 - len];
        let index = (rest / factorial) as usize;
        rest %= factorial;
        *slot = available[index];
        available.copy_within(index + 1..len, index);
        len -= 1;
    }
    out
}

fn lehmer4(permutation: &[u8; 4]) -> u32 {
    let mut rank = 0u32;
    for i in 0..4 {
        let smaller = permutation[i + 1..]
            .iter()
            .filter(|&&value| value < permutation[i])
            .count() as u32;
        rank = rank * (4 - i) as u32 + smaller;
    }
    rank
}
