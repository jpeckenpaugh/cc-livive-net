#!/usr/bin/env bash
# Docker wrapper for all build/test tooling. The host never runs cargo directly.
#
#   ./scripts/docker.sh build              # build/refresh the dev image
#   ./scripts/docker.sh check              # fmt + clippy + tests + no_std build
#   ./scripts/docker.sh cargo test --all   # arbitrary cargo invocation
#   ./scripts/docker.sh cargo run -p color-cube-cli -- solve ...
#   ./scripts/docker.sh wasm-pack build crates/color-cube-wasm --target web
#   ./scripts/docker.sh npm <args>         # node/npm in the same image
#   ./scripts/docker.sh browser-build      # build the Playwright/Chromium image
#   ./scripts/docker.sh browser <cmd>      # run in the browser image
#   ./scripts/docker.sh sh                 # interactive shell in the container
#
# Volumes: cargo registry/git cache and the target dir persist between runs.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
IMAGE=${IMAGE:-cc-livive-dev:latest}
BROWSER_IMAGE=${BROWSER_IMAGE:-cc-livive-browser:latest}
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
  # macOS ships bash 3.2; expanding an empty array under `set -u` is an error
  # there, so use the guarded form.
  local tty_flags=()
  if [ -t 0 ]; then tty_flags+=(-i); fi
  if [ -t 1 ]; then tty_flags+=(-t); fi
  ensure_volume "$CARGO_VOLUME"
  ensure_volume "$TARGET_VOLUME"
  "$DOCKER" run --rm ${tty_flags[@]+"${tty_flags[@]}"} \
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

browser_build() {
  if ! "$DOCKER" image inspect "$IMAGE" >/dev/null 2>&1; then
    build
  fi
  "$DOCKER" build -t "$BROWSER_IMAGE" -f "$ROOT/docker/Dockerfile.browser" "$ROOT"
}

run_browser_container() {
  local tty_flags=()
  if [ -t 0 ]; then tty_flags+=(-i); fi
  if [ -t 1 ]; then tty_flags+=(-t); fi
  ensure_volume "$CARGO_VOLUME"
  ensure_volume "$TARGET_VOLUME"
  "$DOCKER" run --rm ${tty_flags[@]+"${tty_flags[@]}"} \
    --user "$uid:$gid" \
    --shm-size=1g \
    -e HOME=/tmp \
    -e CARGO_HOME=/cargo \
    -e CARGO_TARGET_DIR=/work/target \
    -e PLAYWRIGHT_BROWSERS_PATH=/ms-playwright \
    -e NODE_PATH=/opt/browser/node_modules \
    -v "$ROOT:/work" \
    -v "$CARGO_VOLUME:/cargo" \
    -v "$TARGET_VOLUME:/work/target" \
    -w /work \
    "$BROWSER_IMAGE" "$@"
}

check() {
  # `bash -c`, not `bash -lc`: a login shell re-sources /etc/profile and drops
  # the image's rustup PATH.
  run_container bash -c '
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
  browser-build|browser_build) browser_build ;;
  browser)    run_browser_container "${@:-bash}" ;;
  check)      check ;;
  sh|shell)   run_container bash ;;
  cargo)      run_container cargo "$@" ;;
  wasm-pack)  run_container wasm-pack "$@" ;;
  npm|node)   run_container "$cmd" "$@" ;;
  rustc)      run_container rustc "$@" ;;
  help|-h|--help)
    sed -n '2,24p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    ;;
  *)
    echo "unknown command: $cmd" >&2
    echo "try: ./scripts/docker.sh help" >&2
    exit 2
    ;;
esac
