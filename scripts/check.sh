#!/usr/bin/env bash
# Full local gate: fmt, tables, clippy, no_std build, tests, both wasm builds,
# and the Node smoke test. Everything runs in the pinned Docker image (D002).
#
# Order matters (D027): the wasm crate embeds `tables/*.bin` at compile time, so
# the tables must exist before clippy/tests compile it.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"

section() {
  echo
  echo "== $* =="
}

TABLES=(tables/cc-tables-v1.bin tables/cc-thistlethwaite-v1.bin)
missing=0
for table in "${TABLES[@]}"; do
  [ -f "$table" ] || missing=1
done

section "fmt"
./scripts/docker.sh cargo fmt --all -- --check

if [ "$missing" -eq 1 ]; then
  section "generate tables"
  ./scripts/gen-tables.sh
else
  section "tables present"
  echo "using existing tables/ (regenerate with ./scripts/gen-tables.sh)"
fi

section "clippy"
./scripts/docker.sh cargo clippy --all-targets --all-features -- -D warnings

section "no_std build"
./scripts/docker.sh cargo build -p color-cube --no-default-features

section "tests"
./scripts/docker.sh cargo test --all

section "wasm (web)"
./scripts/docker.sh wasm-pack build crates/color-cube-wasm \
  --target web --release --out-dir ../../web/pkg

section "wasm (nodejs)"
./scripts/docker.sh wasm-pack build crates/color-cube-wasm \
  --target nodejs --release --out-dir ../../web/pkg-node

section "node smoke"
./scripts/docker.sh node scripts/smoke.mjs

echo
echo "OK"
