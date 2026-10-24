//! Optional optimal oracle: IDA* over three disjoint pattern databases (M6).
//!
//! This module is behind the `optimal` cargo feature and is native/CLI-only.
//! It is never compiled into the default build or the wasm bundle.
//!
//! # Coordinates
//!
//! Three admissible heuristics are built:
//!
//! 1. `corner = cp_rank * 2187 + twist` — 88,179,840 entries.
//! 2. `edge_a` — pieces `0..=5` (UR, UF, UL, UB, DR, DF) and their
//!    orientations in any six of the twelve positions.
//! 3. `edge_b` — the complementary pieces `6..=11`.
//!
//! The edge split is contiguous and **frozen**: piece indices are the core's
//! `EDGE_FACELETS` order. A group coordinate is
//!
//! ```text
//! placement * 64 + orientation
//! ```
//!
//! where `placement` is a mixed-radix rank of the six destination slots in
//! piece order, with radices `12, 11, 10, 9, 8, 7` (`12P6 = 665,280`), and
//! `orientation` has bit `p` set when group piece `p` is flipped. Orientation
//! is attached to the piece, not the slot, so a transition only xor-s the
//! destination flip mask; bits never move between pieces.
//!
//! Placement transitions are generated as a transient `665,280 × 18` table
//! (`u32`, low 20 bits placement, high 6 bits the per-piece flip mask). The
//! full `42,577,920 × 18` edge transition table (about 3 GB) is never built.
//!
//! # Heuristic and admissibility
//!
//! `h = max(corner, edge_a, edge_b)`. The edge tables are **not** summed: one
//! face turn can move pieces from both groups, so the sum would double-count
//! moves and is not admissible. Each individual table is a lower bound because
//! it is the exact distance in a quotient of the cube group.
//!
//! # Optimality claims
//!
//! The oracle iteratively deepens from `h` to the depth cap (20 inclusive,
//! or `Limits::max_length` if smaller). [`Solution::optimal`] is set only when
//! every depth below the found length was exhausted (or excluded by the
//! admissible lower bound), and only after the move list is replayed on a copy
//! of the input cube. A wall-clock expiry during a depth returns [`SolveError::Timeout`]
//! and never claims optimality. The depth 20 cap is a limit, not an answer:
//! nothing about superflip is hard-coded.

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use color_cube::{Cube, Move, N_MOVES};

use crate::clock;
use crate::coords;
use crate::solver::{Limits, Solution, SolveError, Solver};
use crate::tables::{self, TableBundle, TablesError};

/// Optimal bundle magic, `b"CCOP"`.
pub const OPTIMAL_MAGIC: [u8; 4] = *b"CCOP";
/// Optimal bundle format version.
pub const OPTIMAL_VERSION: u16 = 1;
/// Bundle file name inside the table directory.
pub const OPTIMAL_BUNDLE_FILE: &str = "cc-optimal-v1.bin";

/// Entries in the corner table (`8! * 3^7`).
pub const CORNER_ROWS: usize = 88_179_840;
/// Entries in each edge-group table (`12P6 * 2^6`).
pub const EDGE_ROWS: usize = 42_577_920;
/// Ordered placements of six pieces in twelve slots (`12P6`).
pub const N_PLACEMENTS: usize = 665_280;
/// Pieces tracked by one edge table.
pub const EDGE_GROUP_PIECES: usize = 6;
/// Orientation combinations of one edge group.
pub const EDGE_ORIENTATIONS: usize = 64;
/// Hard depth cap in half turns, inclusive.
pub const MAX_OPTIMAL_DEPTH: u8 = 20;

const PLACEMENT_BITS: u32 = 20;
const PLACEMENT_MASK: u32 = (1 << PLACEMENT_BITS) - 1;
const FLIP_SHIFT: u32 = PLACEMENT_BITS;
const FLIP_MASK: u32 = 0x3F;

/// Table id for the corner database.
pub const ID_CORNER: u32 = 1;
/// Table id for the first edge group.
pub const ID_EDGE_A: u32 = 2;
/// Table id for the second edge group.
pub const ID_EDGE_B: u32 = 3;

/// Frozen conventions hashed into the `CCOP` bundle.
pub const OPTIMAL_CONVENTIONS: &str = concat!(
    "cc-livive-net optimal pattern databases\n",
    "format=1\n",
    "corner=cp*2187+twist;goal=0;u8\n",
    "edge_split=pieces 0..5 | 6..11;contiguous\n",
    "placement=12P6,mixed-radix(12,11,10,9,8,7),piece order\n",
    "goal_a=piece p at slot p;goal_b=piece 6+p at slot 6+p\n",
    "orientation=6 bits by piece;eo'=eo^flip[move][dest_slot]\n",
    "edge_index=placement*64+orientation\n",
    "moves=U,U2,U',R,R2,R',F,F2,F',D,D2,D',L,L2,L',B,B2,B'\n",
);

/// One generated distance table's wall time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptimalTiming {
    /// Table id (`1..=3`).
    pub id: u32,
    /// Wall time spent generating the table.
    pub elapsed: Duration,
}

/// Coverage and diameter report for one generation run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptimalReport {
    /// Diameter of the corner database.
    pub corner_max: u8,
    /// Diameter of the first edge group.
    pub edge_a_max: u8,
    /// Diameter of the second edge group.
    pub edge_b_max: u8,
    /// Reachable corner states.
    pub corner_reached: usize,
    /// Reachable states in the first edge group.
    pub edge_a_reached: usize,
    /// Reachable states in the second edge group.
    pub edge_b_reached: usize,
}

impl OptimalReport {
    /// Whether every table row was reached.
    #[must_use]
    pub fn fully_covered(&self) -> bool {
        self.corner_reached == CORNER_ROWS
            && self.edge_a_reached == EDGE_ROWS
            && self.edge_b_reached == EDGE_ROWS
    }
}

/// The three u8 pattern databases.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptimalDistanceBundle {
    /// `cp * 2187 + twist` distances.
    pub corner: Vec<u8>,
    /// `placement * 64 + orientation` distances for pieces `0..=5`.
    pub edge_a: Vec<u8>,
    /// `placement * 64 + orientation` distances for pieces `6..=11`.
    pub edge_b: Vec<u8>,
}

impl OptimalDistanceBundle {
    /// Payload size in bytes.
    #[must_use]
    pub fn payload_size(&self) -> usize {
        self.corner.len() + self.edge_a.len() + self.edge_b.len()
    }

    /// Loads the `CCOP` bundle by streaming it; peak memory is the payload.
    ///
    /// # Errors
    ///
    /// [`TablesError::Missing`] when absent, [`TablesError::TableCorrupt`] for
    /// any structural, hash or checksum failure.
    pub fn load(directory: &Path) -> Result<Self, TablesError> {
        let path = directory.join(OPTIMAL_BUNDLE_FILE);
        let file = File::open(&path).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                TablesError::Missing
            } else {
                TablesError::TableCorrupt
            }
        })?;
        let file_len = file
            .metadata()
            .map_err(|_| TablesError::TableCorrupt)?
            .len();
        let mut reader = io::BufReader::new(file);
        let mut header = [0u8; tables::HEADER_LEN];
        reader
            .read_exact(&mut header)
            .map_err(|_| TablesError::TableCorrupt)?;
        if header[..4] != OPTIMAL_MAGIC
            || read_u16(&header, 4) != OPTIMAL_VERSION
            || read_u32(&header, 6) != 3
        {
            return Err(TablesError::TableCorrupt);
        }
        let payload_len = read_u64(&header, 10) as usize;
        let param_hash = read_u64(&header, 18);
        let checksum = read_u64(&header, 26);
        if param_hash != tables::fnv1a64_update(tables::FNV_OFFSET, OPTIMAL_CONVENTIONS.as_bytes())
        {
            return Err(TablesError::TableCorrupt);
        }
        if payload_len != CORNER_ROWS + 2 * EDGE_ROWS {
            return Err(TablesError::TableCorrupt);
        }
        let directory_len = 3 * tables::DIRECTORY_ENTRY_LEN;
        let expected_len = (tables::HEADER_LEN + directory_len + payload_len) as u64;
        if file_len != expected_len {
            return Err(TablesError::TableCorrupt);
        }
        let expected = [
            (ID_CORNER, CORNER_ROWS),
            (ID_EDGE_A, EDGE_ROWS),
            (ID_EDGE_B, EDGE_ROWS),
        ];
        let mut entries = [0u8; 3 * tables::DIRECTORY_ENTRY_LEN];
        reader
            .read_exact(&mut entries)
            .map_err(|_| TablesError::TableCorrupt)?;
        for (slot, &(id, rows)) in expected.iter().enumerate() {
            let offset = slot * tables::DIRECTORY_ENTRY_LEN;
            if read_u32(&entries, offset) != id
                || read_u32(&entries, offset + 4) != rows as u32
                || read_u32(&entries, offset + 8) != 1
                || entries[offset + 12] != 1
            {
                return Err(TablesError::TableCorrupt);
            }
        }

        let mut hash = tables::FNV_OFFSET;
        let corner = read_payload(&mut reader, CORNER_ROWS, &mut hash)?;
        let edge_a = read_payload(&mut reader, EDGE_ROWS, &mut hash)?;
        let edge_b = read_payload(&mut reader, EDGE_ROWS, &mut hash)?;
        if hash != checksum {
            return Err(TablesError::TableCorrupt);
        }
        Ok(Self {
            corner,
            edge_a,
            edge_b,
        })
    }
}

fn read_payload(
    reader: &mut impl Read,
    size: usize,
    hash: &mut u64,
) -> Result<Vec<u8>, TablesError> {
    let mut data = vec![0u8; size];
    reader
        .read_exact(&mut data)
        .map_err(|_| TablesError::TableCorrupt)?;
    *hash = tables::fnv1a64_update(*hash, &data);
    Ok(data)
}

/// Generates the three databases one at a time and streams them to disk.
///
/// Each table is written and dropped before the next is generated, so peak
/// resident memory is one distance table plus one BFS frontier plus, for edge
/// groups, one `665,280 × 18` placement table.
///
/// # Errors
///
/// Returns the underlying I/O error if the file cannot be written.
pub fn generate_and_write(
    moves: &TableBundle,
    directory: &Path,
) -> io::Result<(OptimalReport, [OptimalTiming; 3])> {
    fs::create_dir_all(directory)?;
    let target = directory.join(OPTIMAL_BUNDLE_FILE);
    let temp = directory.join(format!(".{OPTIMAL_BUNDLE_FILE}.{}.tmp", std::process::id()));
    let mut sink = BundleSink::create(&temp)?;

    let started = Instant::now();
    let (corner, corner_max, corner_reached) = generate_corner(moves);
    let t0 = started.elapsed();
    sink.push(&corner)?;
    drop(corner);

    let started = Instant::now();
    let (edge_a, edge_a_max, edge_a_reached) = generate_edge(moves, 0);
    let t1 = started.elapsed();
    sink.push(&edge_a)?;
    drop(edge_a);

    let started = Instant::now();
    let (edge_b, edge_b_max, edge_b_reached) = generate_edge(moves, 6);
    let t2 = started.elapsed();
    sink.push(&edge_b)?;
    drop(edge_b);

    sink.finish(&target)?;
    let report = OptimalReport {
        corner_max,
        edge_a_max,
        edge_b_max,
        corner_reached,
        edge_a_reached,
        edge_b_reached,
    };
    let timings = [
        OptimalTiming {
            id: ID_CORNER,
            elapsed: t0,
        },
        OptimalTiming {
            id: ID_EDGE_A,
            elapsed: t1,
        },
        OptimalTiming {
            id: ID_EDGE_B,
            elapsed: t2,
        },
    ];
    Ok((report, timings))
}

/// Incremental `CCOP` writer: header first, checksum patched at the end.
struct BundleSink {
    file: File,
    temp: PathBuf,
    hash: u64,
    payload_len: usize,
    written: usize,
}

impl BundleSink {
    fn create(temp: &Path) -> io::Result<Self> {
        let mut file = File::create(temp)?;
        let payload_len = CORNER_ROWS + 2 * EDGE_ROWS;
        let mut header = Vec::with_capacity(tables::HEADER_LEN + 3 * tables::DIRECTORY_ENTRY_LEN);
        header.extend_from_slice(&OPTIMAL_MAGIC);
        header.extend_from_slice(&OPTIMAL_VERSION.to_le_bytes());
        header.extend_from_slice(&3u32.to_le_bytes());
        header.extend_from_slice(&(payload_len as u64).to_le_bytes());
        header.extend_from_slice(
            &tables::fnv1a64_update(tables::FNV_OFFSET, OPTIMAL_CONVENTIONS.as_bytes())
                .to_le_bytes(),
        );
        header.extend_from_slice(&0u64.to_le_bytes());
        for &(id, rows) in &[
            (ID_CORNER, CORNER_ROWS),
            (ID_EDGE_A, EDGE_ROWS),
            (ID_EDGE_B, EDGE_ROWS),
        ] {
            header.extend_from_slice(&id.to_le_bytes());
            header.extend_from_slice(&(rows as u32).to_le_bytes());
            header.extend_from_slice(&1u32.to_le_bytes());
            header.push(1);
        }
        file.write_all(&header)?;
        Ok(Self {
            file,
            temp: temp.to_path_buf(),
            hash: tables::FNV_OFFSET,
            payload_len,
            written: 0,
        })
    }

    fn push(&mut self, payload: &[u8]) -> io::Result<()> {
        self.hash = tables::fnv1a64_update(self.hash, payload);
        self.file.write_all(payload)?;
        self.written += payload.len();
        Ok(())
    }

    fn finish(mut self, target: &Path) -> io::Result<()> {
        debug_assert_eq!(self.written, self.payload_len);
        self.file.flush()?;
        self.file
            .seek(SeekFrom::Start(tables::CHECKSUM_OFFSET as u64))?;
        self.file.write_all(&self.hash.to_le_bytes())?;
        self.file.flush()?;
        drop(self.file);
        fs::rename(&self.temp, target)
    }
}

fn generate_corner(moves: &TableBundle) -> (Vec<u8>, u8, usize) {
    let mut distance = vec![u8::MAX; CORNER_ROWS];
    distance[0] = 0;
    let mut current: Vec<u32> = Vec::with_capacity(1 << 20);
    current.push(0);
    let mut next: Vec<u32> = Vec::new();
    let mut depth = 0u8;
    let mut reached = 1usize;
    while !current.is_empty() {
        depth = depth.saturating_add(1);
        next.clear();
        for &index in &current {
            let index = index as usize;
            let cp = index / coords::N_TWIST;
            let twist = index % coords::N_TWIST;
            for move_index in 0..N_MOVES {
                let next_cp = usize::from(moves.move_cp[cp * N_MOVES + move_index]);
                let next_twist = usize::from(moves.move_twist[twist * N_MOVES + move_index]);
                let next_index = next_cp * coords::N_TWIST + next_twist;
                if distance[next_index] == u8::MAX {
                    distance[next_index] = depth;
                    next.push(next_index as u32);
                    reached += 1;
                }
            }
        }
        std::mem::swap(&mut current, &mut next);
    }
    let max = distance
        .iter()
        .copied()
        .filter(|&d| d != u8::MAX)
        .max()
        .unwrap_or(0);
    (distance, max, reached)
}

fn generate_edge(moves: &TableBundle, first_piece: u8) -> (Vec<u8>, u8, usize) {
    let placement = build_placement_table(moves, first_piece);
    let goal_slots: [u8; 6] = std::array::from_fn(|p| first_piece + p as u8);
    let goal = place_rank(&goal_slots) as usize * EDGE_ORIENTATIONS;
    let mut distance = vec![u8::MAX; EDGE_ROWS];
    distance[goal] = 0;
    let mut current: Vec<u32> = Vec::with_capacity(1 << 19);
    current.push(goal as u32);
    let mut next: Vec<u32> = Vec::new();
    let mut depth = 0u8;
    let mut reached = 1usize;
    while !current.is_empty() {
        depth = depth.saturating_add(1);
        next.clear();
        for &index in &current {
            let index = index as usize;
            let rank = index / EDGE_ORIENTATIONS;
            let orientation = (index % EDGE_ORIENTATIONS) as u32;
            for move_index in 0..N_MOVES {
                let packed = placement[rank * N_MOVES + move_index];
                let next_rank = (packed & PLACEMENT_MASK) as usize;
                let next_orientation =
                    (orientation ^ ((packed >> FLIP_SHIFT) & FLIP_MASK)) as usize;
                let next_index = next_rank * EDGE_ORIENTATIONS + next_orientation;
                if distance[next_index] == u8::MAX {
                    distance[next_index] = depth;
                    next.push(next_index as u32);
                    reached += 1;
                }
            }
        }
        std::mem::swap(&mut current, &mut next);
    }
    let max = distance
        .iter()
        .copied()
        .filter(|&d| d != u8::MAX)
        .max()
        .unwrap_or(0);
    (distance, max, reached)
}

/// Builds the transient `665,280 × 18` placement/flip table for one group.
fn build_placement_table(moves: &TableBundle, first_piece: u8) -> Vec<u32> {
    let first = usize::from(first_piece);
    let mut complement = [0u8; 6];
    let mut complement_len = 0;
    for piece in 0..12 {
        if piece < first || piece >= first + EDGE_GROUP_PIECES {
            complement[complement_len] = piece as u8;
            complement_len += 1;
        }
    }

    let mut table = vec![0u32; N_PLACEMENTS * N_MOVES];
    let mut cube = Cube::SOLVED;
    let mut slots = [0u8; EDGE_GROUP_PIECES];
    for rank in 0..N_PLACEMENTS {
        place_unrank(rank as u32, &mut slots);
        let mut used = [false; 12];
        for (p, &slot) in slots.iter().enumerate() {
            used[slot as usize] = true;
            cube.ep[slot as usize] = first_piece + p as u8;
        }
        let mut fill = 0;
        for (slot, &is_used) in used.iter().enumerate() {
            if !is_used {
                cube.ep[slot] = complement[fill];
                fill += 1;
            }
        }
        for move_index in 0..N_MOVES {
            let mut moved = cube;
            moved.apply(Move(move_index as u8));
            let mut next_slots = [0u8; EDGE_GROUP_PIECES];
            for (slot, &piece) in moved.ep.iter().enumerate() {
                let piece = usize::from(piece);
                if piece >= first && piece < first + EDGE_GROUP_PIECES {
                    next_slots[piece - first] = slot as u8;
                }
            }
            let next_rank = place_rank(&next_slots);
            let flip = full_flip_mask(moves, move_index);
            let mut flip_bits = 0u32;
            for (p, &slot) in next_slots.iter().enumerate() {
                flip_bits |= (u32::from((flip >> slot) & 1)) << p;
            }
            table[rank * N_MOVES + move_index] = next_rank | (flip_bits << FLIP_SHIFT);
        }
    }
    table
}

/// The twelve destination flip bits of a move, including the implied bit 11.
fn full_flip_mask(moves: &TableBundle, move_index: usize) -> u16 {
    let low = moves.move_flip[move_index];
    let parity = if low.count_ones() & 1 == 0 {
        0x0000
    } else {
        0x0800
    };
    low | parity
}

/// Mixed-radix rank of six destination slots, in piece order.
fn place_rank(slots: &[u8; EDGE_GROUP_PIECES]) -> u32 {
    const SUFFIX: [u32; EDGE_GROUP_PIECES] = [55_440, 5_040, 504, 56, 7, 1];
    let mut available = [true; 12];
    let mut rank = 0u32;
    for (position, &slot) in slots.iter().enumerate() {
        let mut smaller = 0u32;
        for &is_available in available.iter().take(usize::from(slot)) {
            if is_available {
                smaller += 1;
            }
        }
        rank += smaller * SUFFIX[position];
        available[usize::from(slot)] = false;
    }
    rank
}

/// Inverse of [`place_rank`].
fn place_unrank(rank: u32, slots: &mut [u8; EDGE_GROUP_PIECES]) {
    const RADICES: [u32; EDGE_GROUP_PIECES] = [12, 11, 10, 9, 8, 7];
    const SUFFIX: [u32; EDGE_GROUP_PIECES] = [55_440, 5_040, 504, 56, 7, 1];
    let mut available = [0u8; 12];
    for (position, slot) in available.iter_mut().enumerate() {
        *slot = position as u8;
    }
    let mut len = 12;
    for position in 0..EDGE_GROUP_PIECES {
        let index = ((rank / SUFFIX[position]) % RADICES[position]) as usize;
        slots[position] = available[index];
        available.copy_within(index + 1..len, index);
        len -= 1;
    }
}

/// Reads the two edge-group coordinates of a cube.
fn edge_coords(cube: &Cube, first_piece: u8) -> (u32, u32) {
    let first = usize::from(first_piece);
    let mut slots = [0u8; EDGE_GROUP_PIECES];
    let mut orientation = 0u32;
    for (slot, &piece) in cube.ep.iter().enumerate() {
        let piece = usize::from(piece);
        if piece >= first && piece < first + EDGE_GROUP_PIECES {
            slots[piece - first] = slot as u8;
            orientation |= u32::from(cube.eo[slot] & 1) << (piece - first);
        }
    }
    (place_rank(&slots), orientation)
}

/// Search coordinates, carried incrementally.
#[derive(Clone, Copy, Debug)]
struct Coord {
    cp: u16,
    twist: u16,
    edge_a_place: u32,
    edge_a_ori: u32,
    edge_b_place: u32,
    edge_b_ori: u32,
}

impl Coord {
    fn from_cube(cube: &Cube) -> Self {
        let (edge_a_place, edge_a_ori) = edge_coords(cube, 0);
        let (edge_b_place, edge_b_ori) = edge_coords(cube, 6);
        Self {
            cp: coords::cp_rank(cube),
            twist: coords::twist(cube),
            edge_a_place,
            edge_a_ori,
            edge_b_place,
            edge_b_ori,
        }
    }
}

/// Statistics for one oracle call, returned even when no solution is found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptimalStats {
    /// Total nodes expanded, across all iterations.
    pub nodes: u64,
    /// Nodes expanded per depth bound; index is the bound (`0..=20`).
    pub nodes_by_bound: Vec<u64>,
    /// The root heuristic, `max(corner, edge_a, edge_b)`.
    pub initial_h: u8,
    /// Deepest depth fully exhausted without a solution.
    pub deepest_exhausted: Option<u8>,
    /// The last depth bound attempted.
    pub bound: u8,
    /// Whether the returned solution carries an optimality proof.
    pub optimal: bool,
}

/// An oracle call's solution (or error) together with auditable counters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptimalResult {
    /// The solution, or why the search stopped.
    pub solution: Result<Solution, SolveError>,
    /// Counters for auditing the optimality claim.
    pub stats: OptimalStats,
}

/// Loaded pattern databases plus the transient placement tables.
#[derive(Debug)]
pub struct OptimalOracle {
    distances: OptimalDistanceBundle,
    moves: TableBundle,
    edge_a: Vec<u32>,
    edge_b: Vec<u32>,
}

impl OptimalOracle {
    /// Loads the move bundle and the `CCOP` bundle from `directory`, then
    /// rebuilds the two transient placement tables.
    ///
    /// # Errors
    ///
    /// [`TablesError::Missing`] or [`TablesError::TableCorrupt`] from either
    /// bundle.
    pub fn load(directory: &Path) -> Result<Self, TablesError> {
        let distances = OptimalDistanceBundle::load(directory)?;
        let moves = tables::load(directory)?;
        let edge_a = build_placement_table(&moves, 0);
        let edge_b = build_placement_table(&moves, 6);
        Ok(Self {
            distances,
            moves,
            edge_a,
            edge_b,
        })
    }

    /// Diameters implied by the loaded tables.
    #[must_use]
    pub fn diameters(&self) -> (u8, u8, u8) {
        (
            finite_max(&self.distances.corner),
            finite_max(&self.distances.edge_a),
            finite_max(&self.distances.edge_b),
        )
    }

    /// Solves with IDA* and returns auditable counters.
    #[must_use]
    pub fn solve_with_stats(&self, cube: &Cube, limits: Limits) -> OptimalResult {
        let max_depth = limits
            .max_length
            .map_or(MAX_OPTIMAL_DEPTH, |n| n.min(MAX_OPTIMAL_DEPTH));
        if !cube.is_valid() {
            return OptimalResult {
                solution: Err(SolveError::InvalidState),
                stats: empty_stats(0, max_depth),
            };
        }
        if cube.is_solved() {
            return OptimalResult {
                solution: Err(SolveError::AlreadySolved),
                stats: empty_stats(0, max_depth),
            };
        }

        let root = Coord::from_cube(cube);
        let initial_h = self.heuristic(&root);
        let mut search = Search {
            oracle: self,
            path: Vec::with_capacity(usize::from(MAX_OPTIMAL_DEPTH)),
            nodes: 0,
            nodes_by_bound: vec![0u64; usize::from(MAX_OPTIMAL_DEPTH) + 1],
            total_nodes: 0,
            aborted: false,
            ticks: 0,
            started: clock::now(),
            time_ms: limits.time_ms,
        };
        let mut deepest: Option<u8> = None;

        for bound in initial_h..=max_depth {
            search.nodes = 0;
            search.path.clear();
            if search.dfs(root, 0, bound, u8::MAX) {
                search.nodes_by_bound[usize::from(bound)] = search.nodes;
                let moves: Vec<Move> = search.path.iter().copied().map(Move).collect();
                let mut replay = *cube;
                replay.apply_all(&moves);
                if !replay.is_solved() {
                    return OptimalResult {
                        solution: Err(SolveError::InvalidState),
                        stats: search.stats(initial_h, deepest, bound, false),
                    };
                }
                let proven = bound == 0 || deepest == Some(bound - 1) || bound == initial_h;
                let stats = search.stats(initial_h, Some(bound.saturating_sub(1)), bound, proven);
                return OptimalResult {
                    solution: Ok(Solution {
                        moves,
                        optimal: proven,
                        phases: None,
                    }),
                    stats,
                };
            }
            search.total_nodes += search.nodes;
            search.nodes_by_bound[usize::from(bound)] = search.nodes;
            search.nodes = 0;
            if search.aborted {
                return OptimalResult {
                    solution: Err(SolveError::Timeout),
                    stats: search.stats(initial_h, deepest, bound, false),
                };
            }
            deepest = Some(bound);
        }

        OptimalResult {
            solution: Err(SolveError::NoSolutionWithinLimits),
            stats: search.stats(initial_h, deepest, max_depth, false),
        }
    }

    fn heuristic(&self, coord: &Coord) -> u8 {
        let corner = self.distances.corner
            [usize::from(coord.cp) * coords::N_TWIST + usize::from(coord.twist)];
        let edge_a = self.distances.edge_a
            [coord.edge_a_place as usize * EDGE_ORIENTATIONS + coord.edge_a_ori as usize];
        let edge_b = self.distances.edge_b
            [coord.edge_b_place as usize * EDGE_ORIENTATIONS + coord.edge_b_ori as usize];
        corner.max(edge_a).max(edge_b)
    }
}

impl Solver for OptimalOracle {
    fn solve(&self, cube: &Cube, limits: Limits) -> Result<Solution, SolveError> {
        self.solve_with_stats(cube, limits).solution
    }
}

fn empty_stats(initial_h: u8, bound: u8) -> OptimalStats {
    OptimalStats {
        nodes: 0,
        nodes_by_bound: vec![0u64; usize::from(MAX_OPTIMAL_DEPTH) + 1],
        initial_h,
        deepest_exhausted: None,
        bound,
        optimal: false,
    }
}

fn finite_max(values: &[u8]) -> u8 {
    values
        .iter()
        .copied()
        .filter(|&d| d != u8::MAX)
        .max()
        .unwrap_or(0)
}

struct Search<'a> {
    oracle: &'a OptimalOracle,
    path: Vec<u8>,
    nodes: u64,
    nodes_by_bound: Vec<u64>,
    total_nodes: u64,
    aborted: bool,
    ticks: u64,
    started: clock::Instant,
    time_ms: Option<u32>,
}

impl Search<'_> {
    fn stats(&self, initial_h: u8, deepest: Option<u8>, bound: u8, optimal: bool) -> OptimalStats {
        OptimalStats {
            nodes: self.total_nodes + self.nodes,
            nodes_by_bound: self.nodes_by_bound.clone(),
            initial_h,
            deepest_exhausted: deepest,
            bound,
            optimal,
        }
    }

    fn dfs(&mut self, coord: Coord, depth: u8, bound: u8, last_face: u8) -> bool {
        self.nodes += 1;
        self.tick();
        if self.aborted {
            return false;
        }
        let h = self.oracle.heuristic(&coord);
        if h == u8::MAX || depth.saturating_add(h) > bound {
            return false;
        }
        if h == 0 {
            return true;
        }
        if depth == bound {
            return false;
        }

        for move_index in 0..N_MOVES {
            let face = (move_index / 3) as u8;
            if !move_allowed(face, last_face) {
                continue;
            }
            let next = self.apply(coord, move_index);
            self.path.push(move_index as u8);
            if self.dfs(next, depth + 1, bound, face) {
                return true;
            }
            self.path.pop();
            if self.aborted {
                return false;
            }
        }
        false
    }

    fn apply(&self, coord: Coord, move_index: usize) -> Coord {
        let moves = &self.oracle.moves;
        let packed_a = self.oracle.edge_a[coord.edge_a_place as usize * N_MOVES + move_index];
        let packed_b = self.oracle.edge_b[coord.edge_b_place as usize * N_MOVES + move_index];
        Coord {
            cp: moves.move_cp[usize::from(coord.cp) * N_MOVES + move_index],
            twist: moves.move_twist[usize::from(coord.twist) * N_MOVES + move_index],
            edge_a_place: packed_a & PLACEMENT_MASK,
            edge_a_ori: (coord.edge_a_ori ^ ((packed_a >> FLIP_SHIFT) & FLIP_MASK)) & FLIP_MASK,
            edge_b_place: packed_b & PLACEMENT_MASK,
            edge_b_ori: (coord.edge_b_ori ^ ((packed_b >> FLIP_SHIFT) & FLIP_MASK)) & FLIP_MASK,
        }
    }

    fn tick(&mut self) {
        self.ticks += 1;
        if self.ticks.is_multiple_of(4096) {
            if let Some(time_ms) = self.time_ms {
                if clock::elapsed_ms(self.started) >= u128::from(time_ms) {
                    self.aborted = true;
                }
            }
        }
    }
}

fn move_allowed(face: u8, last_face: u8) -> bool {
    if last_face == u8::MAX {
        return true;
    }
    if face == last_face {
        return false;
    }
    // D014: opposite faces commute; keep the higher face index last.
    if face.abs_diff(last_face) == 3 && face < last_face {
        return false;
    }
    true
}

fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn read_u64(data: &[u8], offset: usize) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&data[offset..offset + 8]);
    u64::from_le_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_rank_round_trips_every_state() {
        let mut slots = [0u8; EDGE_GROUP_PIECES];
        for rank in 0..N_PLACEMENTS {
            place_unrank(rank as u32, &mut slots);
            assert_eq!(place_rank(&slots), rank as u32);
        }
    }

    #[test]
    fn placement_rank_goal_values() {
        assert_eq!(place_rank(&[0, 1, 2, 3, 4, 5]), 0);
        assert_eq!(place_rank(&[11, 10, 9, 8, 7, 6]), N_PLACEMENTS as u32 - 1);
        let mut slots = [0u8; EDGE_GROUP_PIECES];
        place_unrank(0, &mut slots);
        assert_eq!(slots, [0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn solved_coordinates_are_group_goals() {
        let coord = Coord::from_cube(&Cube::SOLVED);
        assert_eq!(coord.cp, 0);
        assert_eq!(coord.twist, 0);
        assert_eq!(coord.edge_a_place, 0);
        assert_eq!(coord.edge_a_ori, 0);
        assert_eq!(coord.edge_b_place, place_rank(&[6, 7, 8, 9, 10, 11]));
        assert_eq!(coord.edge_b_ori, 0);
    }

    #[test]
    fn move_pruning_keeps_opposite_canonical_order() {
        // D014 keeps the higher face index last: D may follow U.
        assert!(move_allowed(3, 0));
        assert!(!move_allowed(0, 3));
        assert!(!move_allowed(0, 0));
    }

    /// End-to-end check of the packed edge transitions against real cubes.
    ///
    /// Ignored because it builds both 48 MB placement tables. Run with
    /// `cargo test --release -p color-cube-solver --features optimal -- --ignored`.
    #[test]
    #[ignore = "builds ~96 MB of placement transition tables"]
    fn edge_transitions_match_cube() {
        let moves = tables::generate();
        let edge_a = build_placement_table(&moves, 0);
        let edge_b = build_placement_table(&moves, 6);
        for seed in 0..200u64 {
            let cube = Cube::from_seed(seed);
            for first in [0u8, 6] {
                let table = if first == 0 { &edge_a } else { &edge_b };
                let (place, orientation) = edge_coords(&cube, first);
                for move_index in 0..N_MOVES {
                    let packed = table[place as usize * N_MOVES + move_index];
                    let mut moved = cube;
                    moved.apply(Move(move_index as u8));
                    let (next_place, next_orientation) = edge_coords(&moved, first);
                    assert_eq!(packed & PLACEMENT_MASK, next_place);
                    assert_eq!(
                        (orientation ^ ((packed >> FLIP_SHIFT) & FLIP_MASK)) & FLIP_MASK,
                        next_orientation
                    );
                    // Corner coordinates come straight from the move bundle.
                    if first == 0 {
                        assert_eq!(
                            moves.move_cp
                                [usize::from(coords::cp_rank(&cube)) * N_MOVES + move_index],
                            coords::cp_rank(&moved)
                        );
                        assert_eq!(
                            moves.move_twist
                                [usize::from(coords::twist(&cube)) * N_MOVES + move_index],
                            coords::twist(&moved)
                        );
                    }
                }
            }
        }
    }
}
