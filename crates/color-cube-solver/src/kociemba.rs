//! Deterministic two-phase Kociemba solver (D024).
//!
//! Phase 1 searches `(twist, flip, slice)` with all 18 moves and the split
//! heuristics `dist(twist, slice)`, `dist(flip, slice)` and `dist(twist, flip)`,
//! bounding phase-1 depth at 12. Every `G1` node is offered to phase 2, which
//! searches `(cp, ep8, sp)` with `PHASE2_MOVES` and the split heuristics
//! `dist(cp, sp)` / `dist(ep8, sp)`, bounded at 18. The shortest total wins.
//!
//! The driver is deterministic: moves are tried in ascending index order, the
//! first phase-2 solution at the minimal phase-2 bound is used, and no hash
//! iteration influences output. The tables are loaded through `OnceLock`.
//!
//! A same-face pair can straddle the phase-1/phase-2 boundary because phase 2
//! starts with no previous-face context. That concatenation may be one move
//! longer than necessary; it is deliberate (the split point then matches the
//! search depths exactly).

use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use color_cube::{Cube, Move, N_MOVES};

use crate::coords::{self, N_CP, N_EP8, N_SP, N_TWIST, SLICE_SOLVED};
use crate::solver::{Limits, Solution, SolveError, Solver};
use crate::tables::{self, TableBundle, TablesError};
use crate::thistlethwaite::TableLoadError;

/// Phase-1 `(twist, slice)` rows: `2187 * 495`.
pub const PHASE1_TWIST_SLICE_ROWS: usize = N_TWIST * coords::N_SLICE;
/// Phase-1 `(flip, slice)` rows: `2048 * 495`.
pub const PHASE1_FLIP_SLICE_ROWS: usize = coords::N_FLIP * coords::N_SLICE;
/// Phase-1 `(twist, flip)` rows: `2187 * 2048`.
pub const PHASE1_TWIST_FLIP_ROWS: usize = N_TWIST * coords::N_FLIP;
/// Phase-2 `(cp, sp)` rows: `40320 * 24`.
pub const PHASE2_CP_SP_ROWS: usize = N_CP * N_SP;
/// Phase-2 `(ep8, sp)` rows: `40320 * 24`.
pub const PHASE2_EP8_SP_ROWS: usize = N_EP8 * N_SP;

/// Phase-2 moves in descending priority order, ascending by move index.
pub const PHASE2_ORDER: [usize; 10] = [0, 1, 2, 4, 7, 9, 10, 11, 13, 16];

/// Bundle magic for the Kociemba distance tables, `b"CCKM"`.
pub const KOCIEMBA_MAGIC: [u8; 4] = *b"CCKM";
/// Distance-bundle format version.
pub const KOCIEMBA_VERSION: u16 = 1;
/// Distance-bundle file name.
pub const KOCIEMBA_BUNDLE_FILE: &str = "cc-kociemba-v1.bin";

/// Frozen parameters hashed into the Kociemba distance bundle.
pub const KOCIEMBA_CONVENTIONS: &str = concat!(
    "cc-livive-net kociemba distance tables\n",
    "format=1\n",
    "phase1_twist_slice=twist*495+slice;goal=0*495+494;moves=all18\n",
    "phase1_flip_slice=flip*495+slice;goal=0*495+494;moves=all18\n",
    "phase1_twist_flip=twist*2048+flip;goal=0;moves=all18\n",
    "phase2_cp_sp=cp*24+sp;goal=0;",
    "moves=0,1,2,4,7,9,10,11,13,16\n",
    "phase2_ep8_sp=ep8*24+sp;goal=0;",
    "moves=0,1,2,4,7,9,10,11,13,16\n",
    "ids=phase1_twist_slice:1:u8,phase1_flip_slice:2:u8,",
    "phase2_cp_sp:3:u8,phase2_ep8_sp:4:u8,phase1_twist_flip:5:u8\n",
);

/// Table id for `phase1_twist_slice`.
pub const ID_PHASE1_TWIST_SLICE: u32 = 1;
/// Table id for `phase1_flip_slice`.
pub const ID_PHASE1_FLIP_SLICE: u32 = 2;
/// Table id for `phase2_cp_sp`.
pub const ID_PHASE2_CP_SP: u32 = 3;
/// Table id for `phase2_ep8_sp`.
pub const ID_PHASE2_EP8_SP: u32 = 4;
/// Table id for `phase1_twist_flip`.
pub const ID_PHASE1_TWIST_FLIP: u32 = 5;

/// The five Kociemba distance tables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KociembaDistanceBundle {
    /// `twist * 495 + slice` distances, all 18 moves.
    pub phase1_twist_slice: Vec<u8>,
    /// `flip * 495 + slice` distances, all 18 moves.
    pub phase1_flip_slice: Vec<u8>,
    /// `cp * 24 + sp` distances, phase-2 moves only.
    pub phase2_cp_sp: Vec<u8>,
    /// `ep8 * 24 + sp` distances, phase-2 moves only.
    pub phase2_ep8_sp: Vec<u8>,
    /// `twist * 2048 + flip` distances, all 18 moves.
    pub phase1_twist_flip: Vec<u8>,
}

impl KociembaDistanceBundle {
    /// Byte size of each table in id order.
    #[must_use]
    pub fn payload_sizes(&self) -> [usize; 5] {
        [
            self.phase1_twist_slice.len(),
            self.phase1_flip_slice.len(),
            self.phase2_cp_sp.len(),
            self.phase2_ep8_sp.len(),
            self.phase1_twist_flip.len(),
        ]
    }

    /// Encodes the bundle in the `CCKM` version-1 layout.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let payloads = [
            self.phase1_twist_slice.clone(),
            self.phase1_flip_slice.clone(),
            self.phase2_cp_sp.clone(),
            self.phase2_ep8_sp.clone(),
            self.phase1_twist_flip.clone(),
        ];
        tables::encode_bundle(
            KOCIEMBA_MAGIC,
            KOCIEMBA_VERSION,
            KOCIEMBA_CONVENTIONS,
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
        tables::write_bundle_bytes(directory, KOCIEMBA_BUNDLE_FILE, &self.encode())
    }

    /// Loads the bundle from `directory`.
    ///
    /// # Errors
    ///
    /// [`TablesError::Missing`] when absent, [`TablesError::TableCorrupt`] for
    /// any structural or checksum failure.
    pub fn load(directory: &Path) -> Result<Self, TablesError> {
        let data = tables::read_bundle_bytes(directory, KOCIEMBA_BUNDLE_FILE)?;
        Self::decode(&data)
    }

    /// Decodes a `CCKM` version-1 bundle.
    ///
    /// # Errors
    ///
    /// [`TablesError::TableCorrupt`] on any mismatch.
    pub fn decode(data: &[u8]) -> Result<Self, TablesError> {
        let payloads = tables::decode_bundle(
            data,
            KOCIEMBA_MAGIC,
            KOCIEMBA_VERSION,
            KOCIEMBA_CONVENTIONS,
            &distance_entries(),
        )?;
        Ok(Self {
            phase1_twist_slice: payloads[0].clone(),
            phase1_flip_slice: payloads[1].clone(),
            phase2_cp_sp: payloads[2].clone(),
            phase2_ep8_sp: payloads[3].clone(),
            phase1_twist_flip: payloads[4].clone(),
        })
    }
}

fn distance_entries() -> [(u32, u32, u32, u8); 5] {
    [
        (ID_PHASE1_TWIST_SLICE, PHASE1_TWIST_SLICE_ROWS as u32, 1, 1),
        (ID_PHASE1_FLIP_SLICE, PHASE1_FLIP_SLICE_ROWS as u32, 1, 1),
        (ID_PHASE2_CP_SP, PHASE2_CP_SP_ROWS as u32, 1, 1),
        (ID_PHASE2_EP8_SP, PHASE2_EP8_SP_ROWS as u32, 1, 1),
        (ID_PHASE1_TWIST_FLIP, PHASE1_TWIST_FLIP_ROWS as u32, 1, 1),
    ]
}

/// Per-table generation time, in table id order.
#[derive(Clone, Copy, Debug)]
pub struct KociembaDistanceTiming {
    /// Table id.
    pub id: u32,
    /// Wall time spent generating the table.
    pub elapsed: Duration,
}

/// Measured coverage and maxima for the five Kociemba tables.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KociembaDistanceReport {
    /// Largest finite `(twist, slice)` distance.
    pub phase1_twist_slice_max: u8,
    /// Reachable `(twist, slice)` coordinates.
    pub phase1_twist_slice_reached: usize,
    /// Largest finite `(flip, slice)` distance.
    pub phase1_flip_slice_max: u8,
    /// Reachable `(flip, slice)` coordinates.
    pub phase1_flip_slice_reached: usize,
    /// Largest finite `(cp, sp)` distance.
    pub phase2_cp_sp_max: u8,
    /// Reachable `(cp, sp)` coordinates.
    pub phase2_cp_sp_reached: usize,
    /// Largest finite `(ep8, sp)` distance.
    pub phase2_ep8_sp_max: u8,
    /// Reachable `(ep8, sp)` coordinates.
    pub phase2_ep8_sp_reached: usize,
    /// Largest finite `(twist, flip)` distance.
    pub phase1_twist_flip_max: u8,
    /// Reachable `(twist, flip)` coordinates.
    pub phase1_twist_flip_reached: usize,
}

impl KociembaDistanceReport {
    /// Whether all five tables covered their whole coordinate space.
    #[must_use]
    pub fn fully_covered(&self) -> bool {
        self.phase1_twist_slice_reached == PHASE1_TWIST_SLICE_ROWS
            && self.phase1_flip_slice_reached == PHASE1_FLIP_SLICE_ROWS
            && self.phase2_cp_sp_reached == PHASE2_CP_SP_ROWS
            && self.phase2_ep8_sp_reached == PHASE2_EP8_SP_ROWS
            && self.phase1_twist_flip_reached == PHASE1_TWIST_FLIP_ROWS
    }
}

/// The Kociemba solver with lazily loaded tables.
#[derive(Debug)]
pub struct Kociemba {
    directory: PathBuf,
    loaded: OnceLock<Result<Arc<LoadedTables>, TableLoadError>>,
}

#[derive(Debug)]
struct LoadedTables {
    moves: TableBundle,
    distances: KociembaDistanceBundle,
}

impl Default for Kociemba {
    fn default() -> Self {
        Self::new()
    }
}

impl Kociemba {
    /// Creates a solver that loads both bundles from the default directory.
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

    /// Generates the move tables and the Kociemba distance bundle into
    /// `directory`, then creates a solver.
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
            bundle: KOCIEMBA_BUNDLE_FILE,
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

    /// Solves `cube` and returns the per-solve search statistics.
    ///
    /// # Errors
    ///
    /// Returns the matching [`SolveError`].
    pub fn solve_with_stats(
        &self,
        cube: &Cube,
        limits: Limits,
    ) -> Result<(Solution, KociembaStats), SolveError> {
        self.solve_with_options(cube, limits, KociembaOptions::default())
    }

    /// Solves `cube` with explicit search options.
    ///
    /// `canonical_pruning: false` is used by the pruning-completeness test; it
    /// explores the same bounds without the same-face/opposite-face reductions.
    ///
    /// # Errors
    ///
    /// Returns the matching [`SolveError`].
    pub fn solve_with_options(
        &self,
        cube: &Cube,
        limits: Limits,
        options: KociembaOptions,
    ) -> Result<(Solution, KociembaStats), SolveError> {
        self.solve_inner(cube, limits, options)
    }

    fn solve_inner(
        &self,
        cube: &Cube,
        limits: Limits,
        options: KociembaOptions,
    ) -> Result<(Solution, KociembaStats), SolveError> {
        if !cube.is_valid() {
            return Err(SolveError::InvalidState);
        }
        if cube.is_solved() {
            return Err(SolveError::AlreadySolved);
        }
        let loaded = self.loaded().map_err(|_| SolveError::TablesMissing)?;
        let started = Instant::now();

        let hard_cap = usize::from(limits.max_length.unwrap_or(30).min(30));
        let mut search = Search {
            moves: &loaded.moves,
            distances: &loaded.distances,
            canonical: options.canonical_pruning,
            started,
            time_ms: limits.time_ms,
            ticks: 0,
            stats: KociembaStats::default(),
            aborted: false,
            best_len: hard_cap + 1,
            best_moves: None,
            best_p1: 0,
            hard_cap,
            visited_g1: HashSet::new(),
        };
        search.check_time();
        if !search.aborted {
            search.run(cube)?;
        }

        if let Some(moves) = search.best_moves {
            let total = moves.len();
            let p1 = search.best_p1;
            let p2 = total.saturating_sub(p1);
            if p1 > 12 || p2 > 18 || total > search.hard_cap {
                return Err(SolveError::NoSolutionWithinLimits);
            }
            let mut replay = *cube;
            replay.apply_all(&moves);
            if !replay.is_solved() {
                return Err(SolveError::NoSolutionWithinLimits);
            }
            return Ok((
                Solution {
                    moves,
                    optimal: false,
                    phases: Some([p1, p2]),
                },
                search.stats,
            ));
        }

        if search.aborted {
            Err(SolveError::Timeout)
        } else {
            Err(SolveError::NoSolutionWithinLimits)
        }
    }
}

impl Solver for Kociemba {
    fn solve(&self, cube: &Cube, limits: Limits) -> Result<Solution, SolveError> {
        self.solve_with_stats(cube, limits)
            .map(|(solution, _)| solution)
    }
}

/// Whether the two-phase search applies canonical move pruning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KociembaOptions {
    /// Prune same-face moves and non-canonical opposite-face pairs.
    pub canonical_pruning: bool,
}

impl Default for KociembaOptions {
    fn default() -> Self {
        Self {
            canonical_pruning: true,
        }
    }
}

/// Counters collected during one solve.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KociembaStats {
    /// Phase-1 IDA* nodes visited.
    pub phase1_nodes: u64,
    /// Phase-2 IDA* searches started (after G1 de-duplication).
    pub phase2_calls: u64,
    /// Phase-2 IDA* nodes visited across all calls.
    pub phase2_nodes: u64,
}

fn load_tables(directory: &Path) -> Result<Arc<LoadedTables>, TableLoadError> {
    let moves = tables::load(directory).map_err(|error| TableLoadError {
        bundle: tables::BUNDLE_FILE,
        error,
    })?;
    let distances = KociembaDistanceBundle::load(directory).map_err(|error| TableLoadError {
        bundle: KOCIEMBA_BUNDLE_FILE,
        error,
    })?;
    Ok(Arc::new(LoadedTables { moves, distances }))
}

struct Search<'a> {
    moves: &'a TableBundle,
    distances: &'a KociembaDistanceBundle,
    canonical: bool,
    started: Instant,
    time_ms: Option<u32>,
    ticks: u64,
    stats: KociembaStats,
    aborted: bool,
    best_len: usize,
    best_moves: Option<Vec<Move>>,
    best_p1: usize,
    hard_cap: usize,
    visited_g1: HashSet<u64>,
}

impl Search<'_> {
    fn run(&mut self, cube: &Cube) -> Result<(), SolveError> {
        let twist = usize::from(coords::twist(cube));
        let flip = usize::from(coords::flip(cube));
        let slice = usize::from(coords::slice(cube));
        let lower = self.phase1_h(twist, flip, slice);
        if lower == u8::MAX || usize::from(lower) > 12 {
            return Err(SolveError::NoSolutionWithinLimits);
        }
        let mut path = Vec::with_capacity(12);
        let mut bound = usize::from(lower);
        while bound <= 12 {
            if bound + 1 >= self.best_len {
                break;
            }
            self.check_time();
            if self.aborted {
                break;
            }
            self.phase1_dfs(*cube, twist, flip, slice, 0, bound, u8::MAX, &mut path);
            if self.aborted {
                break;
            }
            bound += 1;
        }
        Ok(())
    }

    fn phase1_h(&self, twist: usize, flip: usize, slice: usize) -> u8 {
        let a = self.distances.phase1_twist_slice[twist * coords::N_SLICE + slice];
        let b = self.distances.phase1_flip_slice[flip * coords::N_SLICE + slice];
        let c = self.distances.phase1_twist_flip[twist * coords::N_FLIP + flip];
        a.max(b).max(c)
    }

    fn phase2_h(&self, cp: usize, ep8: usize, sp: usize) -> u8 {
        let a = self.distances.phase2_cp_sp[cp * N_SP + sp];
        let b = self.distances.phase2_ep8_sp[ep8 * N_SP + sp];
        a.max(b)
    }

    #[allow(clippy::too_many_arguments)]
    fn phase1_dfs(
        &mut self,
        cube: Cube,
        twist: usize,
        flip: usize,
        slice: usize,
        depth: usize,
        bound: usize,
        last_face: u8,
        path: &mut Vec<u8>,
    ) {
        self.stats.phase1_nodes += 1;
        self.tick();
        if self.aborted {
            return;
        }
        let h = self.phase1_h(twist, flip, slice);
        if h == u8::MAX || depth + usize::from(h) > bound {
            return;
        }
        // Even reaching G1 at the heuristic distance cannot beat the incumbent:
        // phase 2 needs at least zero further moves, so equality is not enough.
        if depth + usize::from(h) >= self.best_len {
            return;
        }
        if h == 0 {
            self.try_phase2(&cube, depth, path);
            if self.aborted {
                return;
            }
        }
        if depth == bound {
            return;
        }
        for m in 0..N_MOVES {
            if !self.move_allowed(m, last_face) {
                continue;
            }
            let next_twist = usize::from(self.moves.move_twist[twist * N_MOVES + m]);
            let next_flip = usize::from(self.moves.move_flip[flip * N_MOVES + m]);
            let next_slice = usize::from(self.moves.move_slice[slice * N_MOVES + m]);
            let mut next_cube = cube;
            next_cube.apply(Move(m as u8));
            path.push(m as u8);
            self.phase1_dfs(
                next_cube,
                next_twist,
                next_flip,
                next_slice,
                depth + 1,
                bound,
                (m / 3) as u8,
                path,
            );
            path.pop();
            if self.aborted {
                return;
            }
        }
    }

    fn try_phase2(&mut self, cube: &Cube, p1_depth: usize, p1_path: &[u8]) {
        if p1_depth + 1 >= self.best_len {
            return;
        }
        let cp = usize::from(coords::cp_rank(cube));
        let ep8 = usize::from(coords::ep8_rank(cube));
        let sp = usize::from(coords::sp_rank(cube));
        let key = ((cp as u64) << 21) | ((ep8 as u64) << 5) | (sp as u64);
        if !self.visited_g1.insert(key) {
            return;
        }
        let h = self.phase2_h(cp, ep8, sp);
        if h == u8::MAX || p1_depth + usize::from(h) >= self.best_len {
            return;
        }
        let limit = (self.best_len - p1_depth - 1).min(18);
        if usize::from(h) > limit {
            return;
        }
        self.stats.phase2_calls += 1;
        let mut path = Vec::with_capacity(limit);
        for bound in usize::from(h)..=limit {
            if self.phase2_dfs(cp, ep8, sp, 0, bound, u8::MAX, &mut path) {
                let total = p1_depth + path.len();
                if total < self.best_len {
                    let mut moves = Vec::with_capacity(total);
                    moves.extend(p1_path.iter().map(|&m| Move(m)));
                    moves.extend(path.iter().map(|&m| Move(m)));
                    self.best_len = total;
                    self.best_p1 = p1_depth;
                    self.best_moves = Some(moves);
                }
                return;
            }
            if self.aborted {
                return;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn phase2_dfs(
        &mut self,
        cp: usize,
        ep8: usize,
        sp: usize,
        depth: usize,
        bound: usize,
        last_face: u8,
        path: &mut Vec<u8>,
    ) -> bool {
        self.stats.phase2_nodes += 1;
        self.tick();
        if self.aborted {
            return false;
        }
        let h = self.phase2_h(cp, ep8, sp);
        if h == u8::MAX || depth + usize::from(h) > bound {
            return false;
        }
        if h == 0 {
            return true;
        }
        if depth == bound {
            return false;
        }
        for &m in &PHASE2_ORDER {
            if !self.move_allowed(m, last_face) {
                continue;
            }
            let next_cp = usize::from(self.moves.move_cp[cp * N_MOVES + m]);
            let next_ep8 = self.moves.move_ep8[ep8 * N_MOVES + m];
            let next_sp = self.moves.move_sp[sp * N_MOVES + m];
            if next_ep8 == u16::MAX || next_sp == u8::MAX {
                continue;
            }
            path.push(m as u8);
            if self.phase2_dfs(
                next_cp,
                usize::from(next_ep8),
                usize::from(next_sp),
                depth + 1,
                bound,
                (m / 3) as u8,
                path,
            ) {
                return true;
            }
            path.pop();
            if self.aborted {
                return false;
            }
        }
        false
    }

    fn move_allowed(&self, m: usize, last_face: u8) -> bool {
        if !self.canonical || last_face == u8::MAX {
            return true;
        }
        let face = (m / 3) as u8;
        if face == last_face {
            return false;
        }
        // D014: opposite faces commute; keep the higher face index last.
        if face.abs_diff(last_face) == 3 && face < last_face {
            return false;
        }
        true
    }

    fn tick(&mut self) {
        self.ticks += 1;
        if self.ticks.is_multiple_of(4096) {
            self.check_time();
        }
    }

    fn check_time(&mut self) {
        if let Some(time_ms) = self.time_ms {
            if self.started.elapsed().as_millis() >= u128::from(time_ms) {
                self.aborted = true;
            }
        }
    }
}

/// Generates the five Kociemba distance tables from an existing move bundle.
#[must_use]
pub fn generate_distance_tables(
    move_tables: &TableBundle,
) -> (
    KociembaDistanceBundle,
    [KociembaDistanceTiming; 5],
    KociembaDistanceReport,
) {
    let started = Instant::now();
    let (phase1_twist_slice, twist_max, twist_reached) = generate_phase1_twist_slice(move_tables);
    let t0 = started.elapsed();

    let started = Instant::now();
    let (phase1_flip_slice, flip_max, flip_reached) = generate_phase1_flip_slice(move_tables);
    let t1 = started.elapsed();

    let started = Instant::now();
    let (phase2_cp_sp, cp_max, cp_reached) = generate_phase2_cp_sp(move_tables);
    let t2 = started.elapsed();

    let started = Instant::now();
    let (phase2_ep8_sp, ep8_max, ep8_reached) = generate_phase2_ep8_sp(move_tables);
    let t3 = started.elapsed();

    let started = Instant::now();
    let (phase1_twist_flip, twist_flip_max, twist_flip_reached) =
        generate_phase1_twist_flip(move_tables);
    let t4 = started.elapsed();

    let bundle = KociembaDistanceBundle {
        phase1_twist_slice,
        phase1_flip_slice,
        phase2_cp_sp,
        phase2_ep8_sp,
        phase1_twist_flip,
    };
    let timings = [
        KociembaDistanceTiming {
            id: ID_PHASE1_TWIST_SLICE,
            elapsed: t0,
        },
        KociembaDistanceTiming {
            id: ID_PHASE1_FLIP_SLICE,
            elapsed: t1,
        },
        KociembaDistanceTiming {
            id: ID_PHASE2_CP_SP,
            elapsed: t2,
        },
        KociembaDistanceTiming {
            id: ID_PHASE2_EP8_SP,
            elapsed: t3,
        },
        KociembaDistanceTiming {
            id: ID_PHASE1_TWIST_FLIP,
            elapsed: t4,
        },
    ];
    let report = KociembaDistanceReport {
        phase1_twist_slice_max: twist_max,
        phase1_twist_slice_reached: twist_reached,
        phase1_flip_slice_max: flip_max,
        phase1_flip_slice_reached: flip_reached,
        phase2_cp_sp_max: cp_max,
        phase2_cp_sp_reached: cp_reached,
        phase2_ep8_sp_max: ep8_max,
        phase2_ep8_sp_reached: ep8_reached,
        phase1_twist_flip_max: twist_flip_max,
        phase1_twist_flip_reached: twist_flip_reached,
    };
    (bundle, timings, report)
}

fn generate_phase1_twist_slice(move_tables: &TableBundle) -> (Vec<u8>, u8, usize) {
    let mut distance = vec![u8::MAX; PHASE1_TWIST_SLICE_ROWS];
    let goal = usize::from(SLICE_SOLVED);
    distance[goal] = 0;
    let mut queue = Vec::with_capacity(PHASE1_TWIST_SLICE_ROWS);
    queue.push(goal as u32);
    let mut head = 0usize;
    let mut reached = 1usize;
    while head < queue.len() {
        let index = queue[head] as usize;
        head += 1;
        let next_distance = distance[index].saturating_add(1);
        let twist = index / coords::N_SLICE;
        let slice = index % coords::N_SLICE;
        for m in 0..N_MOVES {
            let next_twist = usize::from(move_tables.move_twist[twist * N_MOVES + m]);
            let next_slice = usize::from(move_tables.move_slice[slice * N_MOVES + m]);
            let next = next_twist * coords::N_SLICE + next_slice;
            if distance[next] == u8::MAX {
                distance[next] = next_distance;
                reached += 1;
                queue.push(next as u32);
            }
        }
    }
    let max = finite_max(&distance);
    (distance, max, reached)
}

fn generate_phase1_flip_slice(move_tables: &TableBundle) -> (Vec<u8>, u8, usize) {
    let mut distance = vec![u8::MAX; PHASE1_FLIP_SLICE_ROWS];
    let goal = usize::from(SLICE_SOLVED);
    distance[goal] = 0;
    let mut queue = Vec::with_capacity(PHASE1_FLIP_SLICE_ROWS);
    queue.push(goal as u32);
    let mut head = 0usize;
    let mut reached = 1usize;
    while head < queue.len() {
        let index = queue[head] as usize;
        head += 1;
        let next_distance = distance[index].saturating_add(1);
        let flip = index / coords::N_SLICE;
        let slice = index % coords::N_SLICE;
        for m in 0..N_MOVES {
            let next_flip = usize::from(move_tables.move_flip[flip * N_MOVES + m]);
            let next_slice = usize::from(move_tables.move_slice[slice * N_MOVES + m]);
            let next = next_flip * coords::N_SLICE + next_slice;
            if distance[next] == u8::MAX {
                distance[next] = next_distance;
                reached += 1;
                queue.push(next as u32);
            }
        }
    }
    let max = finite_max(&distance);
    (distance, max, reached)
}

fn generate_phase1_twist_flip(move_tables: &TableBundle) -> (Vec<u8>, u8, usize) {
    let mut distance = vec![u8::MAX; PHASE1_TWIST_FLIP_ROWS];
    distance[0] = 0;
    let mut queue = Vec::with_capacity(PHASE1_TWIST_FLIP_ROWS);
    queue.push(0u32);
    let mut head = 0usize;
    let mut reached = 1usize;
    while head < queue.len() {
        let index = queue[head] as usize;
        head += 1;
        let next_distance = distance[index].saturating_add(1);
        let twist = index / coords::N_FLIP;
        let flip = index % coords::N_FLIP;
        for m in 0..N_MOVES {
            let next_twist = usize::from(move_tables.move_twist[twist * N_MOVES + m]);
            let next_flip = usize::from(move_tables.move_flip[flip * N_MOVES + m]);
            let next = next_twist * coords::N_FLIP + next_flip;
            if distance[next] == u8::MAX {
                distance[next] = next_distance;
                reached += 1;
                queue.push(next as u32);
            }
        }
    }
    let max = finite_max(&distance);
    (distance, max, reached)
}

fn generate_phase2_cp_sp(move_tables: &TableBundle) -> (Vec<u8>, u8, usize) {
    let mut distance = vec![u8::MAX; PHASE2_CP_SP_ROWS];
    distance[0] = 0;
    let mut queue = Vec::with_capacity(PHASE2_CP_SP_ROWS);
    queue.push(0u32);
    let mut head = 0usize;
    let mut reached = 1usize;
    while head < queue.len() {
        let index = queue[head] as usize;
        head += 1;
        let next_distance = distance[index].saturating_add(1);
        let cp = index / N_SP;
        let sp = index % N_SP;
        for &m in &tables::PHASE2_MOVES {
            let next_cp = usize::from(move_tables.move_cp[cp * N_MOVES + m]);
            let next_sp = move_tables.move_sp[sp * N_MOVES + m];
            if next_sp == u8::MAX {
                continue;
            }
            let next = next_cp * N_SP + usize::from(next_sp);
            if distance[next] == u8::MAX {
                distance[next] = next_distance;
                reached += 1;
                queue.push(next as u32);
            }
        }
    }
    let max = finite_max(&distance);
    (distance, max, reached)
}

fn generate_phase2_ep8_sp(move_tables: &TableBundle) -> (Vec<u8>, u8, usize) {
    let mut distance = vec![u8::MAX; PHASE2_EP8_SP_ROWS];
    distance[0] = 0;
    let mut queue = Vec::with_capacity(PHASE2_EP8_SP_ROWS);
    queue.push(0u32);
    let mut head = 0usize;
    let mut reached = 1usize;
    while head < queue.len() {
        let index = queue[head] as usize;
        head += 1;
        let next_distance = distance[index].saturating_add(1);
        let ep8 = index / N_SP;
        let sp = index % N_SP;
        for &m in &tables::PHASE2_MOVES {
            let next_ep8 = move_tables.move_ep8[ep8 * N_MOVES + m];
            let next_sp = move_tables.move_sp[sp * N_MOVES + m];
            if next_ep8 == u16::MAX || next_sp == u8::MAX {
                continue;
            }
            let next = usize::from(next_ep8) * N_SP + usize::from(next_sp);
            if distance[next] == u8::MAX {
                distance[next] = next_distance;
                reached += 1;
                queue.push(next as u32);
            }
        }
    }
    let max = finite_max(&distance);
    (distance, max, reached)
}

fn finite_max(distance: &[u8]) -> u8 {
    distance
        .iter()
        .copied()
        .filter(|&d| d != u8::MAX)
        .max()
        .unwrap_or(0)
}

impl fmt::Display for KociembaDistanceReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "twist_slice={}/{} (d={}) flip_slice={}/{} (d={}) \
             cp_sp={}/{} (d={}) ep8_sp={}/{} (d={})",
            self.phase1_twist_slice_reached,
            PHASE1_TWIST_SLICE_ROWS,
            self.phase1_twist_slice_max,
            self.phase1_flip_slice_reached,
            PHASE1_FLIP_SLICE_ROWS,
            self.phase1_flip_slice_max,
            self.phase2_cp_sp_reached,
            PHASE2_CP_SP_ROWS,
            self.phase2_cp_sp_max,
            self.phase2_ep8_sp_reached,
            PHASE2_EP8_SP_ROWS,
            self.phase2_ep8_sp_max,
        )?;
        write!(
            f,
            " twist_flip={}/{} (d={})",
            self.phase1_twist_flip_reached, PHASE1_TWIST_FLIP_ROWS, self.phase1_twist_flip_max
        )
    }
}
