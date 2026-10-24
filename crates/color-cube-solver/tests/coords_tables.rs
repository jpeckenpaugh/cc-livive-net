//! Coordinate round-trips, move-table agreement, bundle determinism and
//! corruption handling.
//!
//! This is the only integration binary that owns a generated table bundle. It
//! generates once into `std::env::temp_dir()/cc-tables-<pid>` behind a
//! `OnceLock`; other test binaries never touch tables.

use std::sync::OnceLock;
use std::time::Instant;

use color_cube::{Cube, Move, N_MOVES};
use color_cube_solver::coords;
use color_cube_solver::tables::{self, TableBundle, TablesError, PHASE2_MOVES};

fn bundle() -> &'static TableBundle {
    static BUNDLE: OnceLock<TableBundle> = OnceLock::new();
    BUNDLE.get_or_init(|| {
        let started = Instant::now();
        let bundle = tables::generate();
        eprintln!(
            "generated all move tables in {:.3} s (debug build)",
            started.elapsed().as_secs_f64()
        );
        let directory = std::env::temp_dir().join(format!("cc-tables-{}", std::process::id()));
        tables::write(&directory, &bundle).expect("write generated bundle");
        let reloaded = tables::load(&directory).expect("reload generated bundle");
        assert_eq!(tables::encode(&bundle), tables::encode(&reloaded));
        bundle
    })
}

fn cube_for_twist(value: u16) -> Cube {
    let mut cube = Cube::SOLVED;
    cube.set_corner_twist(value);
    cube
}

fn cube_for_flip(value: u16) -> Cube {
    let mut cube = Cube::SOLVED;
    cube.set_edge_flip(value);
    cube
}

fn cube_for_slice(value: u16) -> Cube {
    let mut cube = Cube::SOLVED;
    cube.set_slice_combination(value);
    cube
}

#[test]
fn core_twist_and_flip_round_trip_and_stay_valid() {
    for value in 0..coords::N_TWIST as u16 {
        let mut cube = Cube::SOLVED;
        cube.set_corner_twist(value);
        assert_eq!(cube.corner_twist(), value);
        assert!(cube.is_valid());
    }
    for value in 0..coords::N_FLIP as u16 {
        let mut cube = Cube::SOLVED;
        cube.set_edge_flip(value);
        assert_eq!(cube.edge_flip(), value);
        assert!(cube.is_valid());
    }
}

#[test]
fn core_slice_round_trips_but_is_raw() {
    let mut invalid = 0;
    for value in 0..coords::N_SLICE as u16 {
        let mut cube = Cube::SOLVED;
        cube.set_slice_combination(value);
        assert_eq!(cube.slice_combination(), value);
        assert_eq!(
            cube.ep[..8].iter().filter(|&&piece| piece >= 8).count()
                + cube.ep[8..].iter().filter(|&&piece| piece >= 8).count(),
            4
        );
        if !cube.is_valid() {
            invalid += 1;
        }
    }
    // The raw setter is documented as possibly flipping ep parity; this pins
    // the fact that it does for some ranks rather than "repairing" silently.
    assert!(invalid > 0, "expected some raw slice ranks to be invalid");
}

#[test]
fn solver_setters_round_trip_and_are_always_valid() {
    let mut cube = Cube::SOLVED;
    for value in 0..coords::N_TWIST as u16 {
        coords::set_twist(&mut cube, value);
        assert_eq!(coords::twist(&cube), value);
        assert!(cube.is_valid());
    }
    for value in 0..coords::N_FLIP as u16 {
        coords::set_flip(&mut cube, value);
        assert_eq!(coords::flip(&cube), value);
        assert!(cube.is_valid());
    }
    for value in 0..coords::N_SLICE as u16 {
        coords::set_slice(&mut cube, value);
        assert_eq!(coords::slice(&cube), value);
        assert!(cube.is_valid());
    }
    for value in 0..coords::N_SP as u16 {
        coords::set_sp(&mut cube, value);
        assert_eq!(coords::sp_rank(&cube), value);
        assert!(cube.is_valid());
    }
    for value in 0..coords::N_CP as u16 {
        coords::set_cp(&mut cube, value);
        assert_eq!(coords::cp_rank(&cube), value);
        assert!(cube.is_valid());
    }
    for value in 0..coords::N_EP8 as u16 {
        coords::set_ep8(&mut cube, value);
        assert_eq!(coords::ep8_rank(&cube), value);
        assert!(cube.is_valid());
        assert_eq!(coords::slice(&cube), coords::SLICE_SOLVED);
    }
    for cp in (0..coords::N_CP as u16).step_by(97) {
        for ep8 in (0..coords::N_EP8 as u16).step_by(97) {
            coords::set_cp_ep8(&mut cube, cp, ep8);
            assert_eq!(coords::cp_rank(&cube), cp);
            assert_eq!(coords::ep8_rank(&cube), ep8);
            assert!(cube.is_valid());
        }
    }
}

#[test]
fn move_tables_agree_with_realisation() {
    let tables = bundle();
    for value in 0..coords::N_TWIST {
        let base = cube_for_twist(value as u16);
        for index in 0..N_MOVES {
            let mut cube = base;
            cube.apply(Move(index as u8));
            assert_eq!(
                tables.move_twist[value * N_MOVES + index],
                cube.corner_twist(),
                "twist {value} move {index}"
            );
        }
    }
    for value in 0..coords::N_FLIP {
        let base = cube_for_flip(value as u16);
        for index in 0..N_MOVES {
            let mut cube = base;
            cube.apply(Move(index as u8));
            assert_eq!(
                tables.move_flip[value * N_MOVES + index],
                cube.edge_flip(),
                "flip {value} move {index}"
            );
        }
    }
    for value in 0..coords::N_SLICE {
        let base = cube_for_slice(value as u16);
        for index in 0..N_MOVES {
            let mut cube = base;
            cube.apply(Move(index as u8));
            assert_eq!(
                tables.move_slice[value * N_MOVES + index],
                cube.slice_combination(),
                "slice {value} move {index}"
            );
        }
    }
    for value in 0..coords::N_SP {
        let mut base = Cube::SOLVED;
        coords::set_sp(&mut base, value as u16);
        for &index in &PHASE2_MOVES {
            let mut cube = base;
            cube.apply(Move(index as u8));
            assert_eq!(
                tables.move_sp[value * N_MOVES + index],
                coords::sp_rank(&cube) as u8,
                "sp {value} move {index}"
            );
        }
        for index in 0..N_MOVES {
            if !PHASE2_MOVES.contains(&index) {
                assert_eq!(tables.move_sp[value * N_MOVES + index], u8::MAX);
            }
        }
    }
    for value in (0..coords::N_CP).step_by(997) {
        let mut base = Cube::SOLVED;
        coords::set_cp(&mut base, value as u16);
        for index in 0..N_MOVES {
            let mut cube = base;
            cube.apply(Move(index as u8));
            assert_eq!(
                tables.move_cp[value * N_MOVES + index],
                coords::cp_rank(&cube),
                "cp {value} move {index}"
            );
        }
    }
    for value in (0..coords::N_EP8).step_by(997) {
        let mut base = Cube::SOLVED;
        coords::set_ep8(&mut base, value as u16);
        for &index in &PHASE2_MOVES {
            let mut cube = base;
            cube.apply(Move(index as u8));
            assert_eq!(
                tables.move_ep8[value * N_MOVES + index],
                coords::ep8_rank(&cube),
                "ep8 {value} move {index}"
            );
        }
        for index in 0..N_MOVES {
            if !PHASE2_MOVES.contains(&index) {
                assert_eq!(tables.move_ep8[value * N_MOVES + index], u16::MAX);
            }
        }
    }
}

#[test]
fn move_tables_compose() {
    let tables = bundle();
    for value in (0..coords::N_TWIST).step_by(37) {
        for a in 0..N_MOVES {
            for b in 0..N_MOVES {
                let next = tables.move_twist[value * N_MOVES + a] as usize;
                let composed = tables.move_twist[next * N_MOVES + b];
                let mut cube = cube_for_twist(value as u16);
                cube.apply_all(&[Move(a as u8), Move(b as u8)]);
                assert_eq!(composed, cube.corner_twist());
            }
        }
    }
    for value in (0..coords::N_CP).step_by(2_113) {
        for a in 0..N_MOVES {
            for b in 0..N_MOVES {
                let next = tables.move_cp[value * N_MOVES + a] as usize;
                let composed = tables.move_cp[next * N_MOVES + b];
                let mut cube = Cube::SOLVED;
                coords::set_cp(&mut cube, value as u16);
                cube.apply_all(&[Move(a as u8), Move(b as u8)]);
                assert_eq!(composed, coords::cp_rank(&cube));
            }
        }
    }
    for value in 0..coords::N_SP {
        for &a in &PHASE2_MOVES {
            for &b in &PHASE2_MOVES {
                let next = tables.move_sp[value * N_MOVES + a] as usize;
                let composed = tables.move_sp[next * N_MOVES + b];
                let mut cube = Cube::SOLVED;
                coords::set_sp(&mut cube, value as u16);
                cube.apply_all(&[Move(a as u8), Move(b as u8)]);
                assert_eq!(composed, coords::sp_rank(&cube) as u8);
            }
        }
    }
}

#[test]
fn bundle_is_deterministic_and_detects_corruption() {
    let tables = bundle();
    let first = tables::encode(tables);
    let second = tables::encode(tables);
    assert_eq!(first, second);

    let directory = std::env::temp_dir().join(format!("cc-tables-enc-{}", std::process::id()));
    let path = tables::write(&directory, tables).expect("write bundle");
    let bytes = std::fs::read(&path).expect("read bundle");
    assert_eq!(bytes, first);

    let mut corrupt = bytes;
    let last = corrupt.len() - 1;
    corrupt[last] ^= 0x01;
    assert_eq!(tables::decode(&corrupt), Err(TablesError::TableCorrupt));

    let mut bad_magic = first.clone();
    bad_magic[0] ^= 0xFF;
    assert_eq!(tables::decode(&bad_magic), Err(TablesError::TableCorrupt));

    let missing = std::env::temp_dir().join(format!("cc-tables-none-{}", std::process::id()));
    let _ = std::fs::remove_file(missing.join(tables::BUNDLE_FILE));
    assert_eq!(tables::load(&missing), Err(TablesError::Missing));
}

#[test]
fn coordinates_cross_check_against_cubie_arrays() {
    for seed in 0..1_000u64 {
        let cube = Cube::from_seed(seed);
        let twist: u16 = cube
            .co
            .iter()
            .take(7)
            .enumerate()
            .map(|(i, &orientation)| u16::from(orientation) * 3u16.pow(i as u32))
            .sum();
        assert_eq!(cube.corner_twist(), twist, "seed {seed}");

        let flip: u16 = cube
            .eo
            .iter()
            .take(11)
            .enumerate()
            .map(|(i, &orientation)| u16::from(orientation) * 2u16.pow(i as u32))
            .sum();
        assert_eq!(cube.edge_flip(), flip, "seed {seed}");

        let mut positions = [0usize; 4];
        let mut len = 0;
        for (position, &piece) in cube.ep.iter().enumerate() {
            if piece >= 8 {
                positions[len] = position;
                len += 1;
            }
        }
        assert_eq!(len, 4);
        let slice = positions
            .iter()
            .enumerate()
            .map(|(k, &position)| {
                let n = position as u32;
                let k = k as u32 + 1;
                if k > n {
                    0
                } else {
                    (1..=k).fold(1, |acc, i| acc * (n + 1 - i) / i)
                }
            })
            .sum::<u32>();
        assert_eq!(u32::from(cube.slice_combination()), slice, "seed {seed}");
        assert_eq!(coords::cp_rank(&cube), {
            let mut rank = 0u32;
            for (i, &value) in cube.cp.iter().enumerate() {
                let smaller = cube.cp[i + 1..].iter().filter(|&&v| v < value).count() as u32;
                rank = rank * (8 - i) as u32 + smaller;
            }
            rank as u16
        });
    }
}
