#!/usr/bin/env bash
# Proves the web wasm is reproducible: two clean builds in one image, at two
# mount paths (/work and /alt), with fresh target dirs and regenerated tables.
#
#   ./scripts/repro-check.sh                    # host platform, clean HEAD
#   ./scripts/repro-check.sh --allow-dirty      # local working tree (not evidence)
#   ./scripts/repro-check.sh --platform linux/amd64   # cross-arch, best effort
#
# Prints one PASS/FAIL line; exits non-zero on a mismatch. Missing emulation is
# a printed skip (exit 0), per the brief. Container images are not compared:
# only artifact identity (wasm+js) and table bytes are.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"

DOCKER=${DOCKER:-docker}
IMAGE=${IMAGE:-cc-livive-dev:latest}
CARGO_VOLUME=${CARGO_VOLUME:-cc-livive-cargo}
MIN_FREE_GB=${MIN_FREE_GB:-6}
PLATFORM=""
PLATFORM_EXPLICIT=0
ALLOW_DIRTY=0

usage() {
  sed -n '2,12p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --platform) PLATFORM=${2:?--platform needs a value}; PLATFORM_EXPLICIT=1; shift 2 ;;
    --allow-dirty) ALLOW_DIRTY=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

uid=$(id -u)
gid=$(id -g)
host_platform=$("$DOCKER" info --format '{{.OSType}}/{{.Architecture}}' 2>/dev/null || true)
host_platform=$(printf '%s' "$host_platform" | sed 's#/aarch64$#/arm64#; s#/x86_64$#/amd64#')
if [ -z "$PLATFORM" ]; then
  PLATFORM=$host_platform
fi
if [ -z "$PLATFORM" ]; then
  echo "FAIL: cannot determine docker platform" >&2
  exit 2
fi
if [ "$PLATFORM_EXPLICIT" -eq 1 ] && [ "$PLATFORM" = "$host_platform" ]; then
  echo "SKIP: $PLATFORM is the native platform; the native repro-check run covers it"
  exit 0
fi

platform_tag=${IMAGE}-$(printf '%s' "$PLATFORM" | tr '/:' '--')
build_image=$IMAGE

if ! "$DOCKER" image inspect "$IMAGE" >/dev/null 2>&1; then
  echo "== build native image =="
  ./scripts/docker.sh build
fi

if [ "$PLATFORM" != "$host_platform" ]; then
  echo "== cross-arch check: $PLATFORM (host $host_platform) =="
  if ! "$DOCKER" image inspect "$platform_tag" >/dev/null 2>&1; then
    if ! "$DOCKER" build --platform "$PLATFORM" -t "$platform_tag" \
         -f docker/Dockerfile .; then
      echo "SKIP: cannot build $PLATFORM image (emulation unavailable?)"
      exit 0
    fi
  fi
  build_image=$platform_tag
  if ! "$DOCKER" run --rm --platform "$PLATFORM" "$build_image" true >/dev/null 2>&1; then
    echo "SKIP: cannot run $PLATFORM containers (emulation unavailable)"
    exit 0
  fi
fi

platform_flag=()
if [ "$PLATFORM" != "$host_platform" ]; then
  platform_flag=(--platform "$PLATFORM")
fi

if [ "$ALLOW_DIRTY" -eq 1 ]; then
  echo "WARNING: --allow-dirty builds the working tree; this is not release evidence"
else
  if [ -n "$(git -C "$ROOT" status --porcelain --untracked-files=no)" ]; then
    echo "FAIL: working tree has tracked changes; commit first or pass --allow-dirty" >&2
    exit 2
  fi
fi

epoch=$(git -C "$ROOT" log -1 --format=%ct 2>/dev/null || true)
if [ -z "$epoch" ]; then
  echo "FAIL: cannot read commit timestamp for SOURCE_DATE_EPOCH" >&2
  exit 2
fi

ensure_cargo() {
  if ! "$DOCKER" volume inspect "$CARGO_VOLUME" >/dev/null 2>&1; then
    "$DOCKER" volume create "$CARGO_VOLUME" >/dev/null
    "$DOCKER" run --rm -u 0 -v "$CARGO_VOLUME:/cargo" "$build_image" \
      chown -R "$uid:$gid" /cargo
  fi
}

prepare_volume() {
  local volume=$1
  "$DOCKER" volume create "$volume" >/dev/null
  "$DOCKER" run --rm -u 0 -v "$volume:/src" "$build_image" \
    chown -R "$uid:$gid" /src
  if [ "$ALLOW_DIRTY" -eq 1 ]; then
    tar -C "$ROOT" -cf - \
      --exclude .git --exclude target --exclude '**/target' \
      --exclude tables --exclude web/pkg --exclude web/pkg-node \
      --exclude node_modules --exclude artifacts --exclude .DS_Store . \
      | "$DOCKER" run --rm -i --user "$uid:$gid" -v "$volume:/src" \
          "$build_image" tar -xf - -C /src
  else
    git -C "$ROOT" archive --format=tar HEAD \
      | "$DOCKER" run --rm -i --user "$uid:$gid" -v "$volume:/src" \
          "$build_image" tar -xf - -C /src
  fi
}

check_disk() {
  local volume=$1
  local available_kb
  available_kb=$("$DOCKER" run --rm ${platform_flag[@]+"${platform_flag[@]}"} \
    -v "$volume:/w" "$build_image" df -Pk /w | awk 'NR==2 {print $4}')
  if [ -z "$available_kb" ] || [ "$available_kb" -lt $((MIN_FREE_GB * 1024 * 1024)) ]; then
    echo "FAIL: less than ${MIN_FREE_GB} GB free in the build volume (${available_kb:-?} KB)" >&2
    exit 2
  fi
}

run_build() {
  local volume=$1 mount=$2
  "$DOCKER" run --rm ${platform_flag[@]+"${platform_flag[@]}"} --user "$uid:$gid" \
    -e HOME=/tmp \
    -e CARGO_HOME=/cargo \
    -e CARGO_TARGET_DIR="$mount/target" \
    -e CARGO_INCREMENTAL=0 \
    -e SOURCE_DATE_EPOCH="$epoch" \
    -e "RUSTFLAGS=--remap-path-prefix=/work=/src --remap-path-prefix=/alt=/src" \
    -v "$volume:$mount" \
    -v "$CARGO_VOLUME:/cargo" \
    -w "$mount" \
    "$build_image" bash -c '
      set -euo pipefail
      cargo run --release -p color-cube-solver --bin gen-tables >/dev/null
      wasm-pack build crates/color-cube-wasm --target web --release --out-dir "$PWD/out" >/dev/null
      printf "HASH wasm %s\n" "$(sha256sum out/color_cube_wasm_bg.wasm | cut -d" " -f1)"
      printf "HASH js %s\n" "$(sha256sum out/color_cube_wasm.js | cut -d" " -f1)"
      printf "HASH gzip %s\n" "$(gzip -9 -n -c out/color_cube_wasm_bg.wasm | sha256sum | cut -d" " -f1)"
      printf "HASH table_moves %s\n" "$(sha256sum tables/cc-tables-v1.bin | cut -d" " -f1)"
      printf "HASH table_thistlethwaite %s\n" "$(sha256sum tables/cc-thistlethwaite-v1.bin | cut -d" " -f1)"
    '
}

hash_of() {
  printf '%s\n' "$2" | awk -v key="$1" '$1 == "HASH" && $2 == key {print $3; exit}'
}

volume_a="cc-livive-repro-a-$$"
volume_b="cc-livive-repro-b-$$"
cleanup() {
  "$DOCKER" volume rm -f "$volume_a" "$volume_b" >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "== reproducible wasm build: $PLATFORM (image $build_image) =="
"$DOCKER" system df 2>/dev/null || true
ensure_cargo
prepare_volume "$volume_a"
prepare_volume "$volume_b"
check_disk "$volume_a"
check_disk "$volume_b"

echo "== build A at /work =="
out_a=$(run_build "$volume_a" /work)
echo "== build B at /alt =="
out_b=$(run_build "$volume_b" /alt)

keys="wasm js gzip table_moves table_thistlethwaite"
mismatch=0
for key in $keys; do
  a=$(hash_of "$key" "$out_a")
  b=$(hash_of "$key" "$out_b")
  if [ -z "$a" ] || [ -z "$b" ] || [ "$a" != "$b" ]; then
    echo "MISMATCH $key: A=${a:-missing} B=${b:-missing}"
    mismatch=1
  else
    echo "MATCH $key $a"
  fi
done

if [ "$mismatch" -ne 0 ]; then
  echo "FAIL: reproducible build mismatch on $PLATFORM"
  exit 1
fi
echo "PASS: wasm $(hash_of wasm "$out_a") js $(hash_of js "$out_a") gzip $(hash_of gzip "$out_a") on $PLATFORM"
