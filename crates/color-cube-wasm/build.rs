//! Build-time check that the embedded table bundles exist (D027).
//!
//! `include_bytes!` would fail with a bare "No such file" error. This script
//! turns that into an actionable message and makes Cargo rebuild the wrapper
//! whenever a bundle is regenerated.

use std::env;
use std::path::PathBuf;

const BUNDLES: [&str; 2] = ["cc-tables-v1.bin", "cc-thistlethwaite-v1.bin"];

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let tables = manifest.join("../../tables");
    for bundle in BUNDLES {
        let path = tables.join(bundle);
        println!("cargo:rerun-if-changed={}", path.display());
        if !path.is_file() {
            panic!(
                "missing table bundle: {}\n\
                 run ./scripts/gen-tables.sh to generate it, then rebuild.",
                path.display()
            );
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
}
