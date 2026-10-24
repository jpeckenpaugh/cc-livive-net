#!/usr/bin/env bash
# Docker wrapper for all build/test tooling. The host never runs cargo directly.
#
#   ./scripts/docker.sh build              # build/refresh the dev image
#   ./scripts/docker.sh check              # fmt + clippy + tests + no_std build
#   ./scripts/docker.sh cargo test --all   # arbitrary cargo invocation
#   ./scripts/docker.sh cargo run -p color-cube-cli -- solve ...
#   ./scripts/docker.sh wasm-pack build crates/color-cube-wasm --target web
#   ./scripts/docker.sh npm <args>         # node/npm in the same image
#   ./scripts/docker.sh sh                 # interactive shell in the container
#
# Volumes: cargo registry/git cache and the target dir persist between runs.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
IMAGE=${IMAGE:-cc-livive-dev:latest}
CARGO_VOLUME=${CARGO_VOLUME:-cc-livive-cargo}
TARGET_VOLUME=${TARGET_VOLUME:-cc-livive-target}
DOCKER=${DOCKER:-docker}

uid=$(id -u)
gid=$(id -g)

ensure_volume() {
  if ! "$DOCKER" volume inspect "$1" >/dev/null 2>&1; then
    "$DOCKER" volume create "$1" >/dev/null
  fi
}

build() {
  "$DOCKER" build -t "$IMAGE" -f "$ROOT/docker/Dockerfile" "$ROOT"
  ensure_volume "$CARGO_VOLUME"
  ensure_volume "$TARGET_VOLUME"
  # Make the cache volumes writable by the host uid so bind-mounted outputs are
  # never root-owned.
  "$DOCKER" run --rm -u 0 \
    -v "$CARGO_VOLUME:/cargo" \
    -v "$TARGET_VOLUME:/work/target" \
    "$IMAGE" chown -R "$uid:$gid" /cargo /work/target
}

run_container() {
  local tty_flags=()
  if [ -t 0 ] && [ -t 1 ]; then tty_flags=(-it); fi
  ensure_volume "$CARGO_VOLUME"
  ensure_volume "$TARGET_VOLUME"
  "$DOCKER" run --rm "${tty_flags[@]}" \
    --user "$uid:$gid" \
    -e HOME=/tmp \
    -e CARGO_HOME=/cargo \
    -e CARGO_TARGET_DIR=/work/target \
    -v "$ROOT:/work" \
    -v "$CARGO_VOLUME:/cargo" \
    -v "$TARGET_VOLUME:/work/target" \
    -w /work \
    "$IMAGE" "$@"
}

check() {
  run_container bash -lc '
    set -euo pipefail
    echo "== fmt =="
    cargo fmt --all -- --check
    echo "== clippy =="
    cargo clippy --all-targets --all-features -- -D warnings
    echo "== build (no_std core) =="
    cargo build -p color-cube --no-default-features
    echo "== test =="
    cargo test --all
    echo "OK"
  '
}

cmd=${1:-help}
shift || true

case "$cmd" in
  build)      build ;;
  check)      check ;;
  sh|shell)   run_container bash ;;
  cargo)      run_container cargo "$@" ;;
  wasm-pack)  run_container wasm-pack "$@" ;;
  npm|node)   run_container "$cmd" "$@" ;;
  rustc)      run_container rustc "$@" ;;
  help|-h|--help)
    sed -n '2,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    ;;
  *)
    echo "unknown command: $cmd" >&2
    echo "try: ./scripts/docker.sh help" >&2
    exit 2
    ;;
esac
