#!/usr/bin/env bash
# Generates the solver table bundles into ./tables (gitignored).
#
#   ./scripts/gen-tables.sh            # generate into $COLOR_CUBE_TABLES or ./tables
#   ./scripts/gen-tables.sh --verify   # also exhaustively verify the G3 space
#
# All cargo work runs in the pinned Docker image (D002).
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
exec "$ROOT/scripts/docker.sh" cargo run --release -p color-cube-solver --bin gen-tables -- "$@"
