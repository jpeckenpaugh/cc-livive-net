#!/usr/bin/env bash
# M4b browser gate: Playwright + Chromium in Docker, serving web/ over HTTP.
#
# Builds the browser image if missing and the web wasm if missing, then runs
# web/tests/browser.spec.mjs. Screenshots on failure land in artifacts/.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"
DOCKER=${DOCKER:-docker}

if ! "$DOCKER" image inspect cc-livive-browser:latest >/dev/null 2>&1; then
  echo "== build browser image (cc-livive-browser:latest) =="
  ./scripts/docker.sh browser-build
fi

if [ ! -f web/pkg/color_cube_wasm.js ] || [ ! -f web/pkg/color_cube_wasm_bg.wasm ]; then
  echo "== build wasm (web) =="
  ./scripts/docker.sh wasm-pack build crates/color-cube-wasm \
    --target web --release --out-dir ../../web/pkg
fi

mkdir -p artifacts

# The page fetches provenance only when the Rust-types tab is opened, but the
# browser spec does open it. Generate it when a developer has not run check.sh
# yet, so a missing file can never look like a UI failure.
if [ ! -f web/provenance.json ]; then
  echo "== provenance =="
  CC_CHECK=pass ./scripts/docker.sh node scripts/provenance.mjs
fi

echo "== browser smoke =="
./scripts/docker.sh browser bash -c '
  set -euo pipefail
  rm -f /tmp/cc-serve.log
  node scripts/serve.mjs web > /tmp/cc-serve.log 2>&1 &
  server_pid=$!
  trap "kill $server_pid 2>/dev/null || true" EXIT
  for _ in $(seq 1 100); do
    if grep -q "^http://" /tmp/cc-serve.log 2>/dev/null; then break; fi
    sleep 0.1
  done
  base_url=$(head -n 1 /tmp/cc-serve.log)
  if [ -z "${base_url:-}" ]; then
    cat /tmp/cc-serve.log >&2
    echo "static server failed to start" >&2
    exit 1
  fi
  node web/tests/browser.spec.mjs "$base_url"
'
