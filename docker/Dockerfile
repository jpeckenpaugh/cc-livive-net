# syntax=docker/dockerfile:1
#
# Dev/build image for cc-livive-net. Everything (cargo, wasm-pack, node) runs in
# here; the host only needs git and an editor.
#
# Build:  ./scripts/docker.sh build
# Use:    ./scripts/docker.sh cargo test -p color-cube
#
# Pins that can change the wasm: the rust image, wasm-pack, the wasm-bindgen CLI
# (0.2.129) and wasm-opt/binaryen (117) baked below, Cargo.lock, the table
# generator, and the RUSTFLAGS/profile supplied by scripts/docker.sh. The Node
# image is test-only; it cannot change the wasm bytes. Container images are not
# bit-reproducible (Docker layer timestamps); the artifact is (D030/D031).

# Multi-arch manifest lists (amd64 + arm64), verified with
# `docker buildx imagetools inspect`. Do not replace these with per-platform
# child digests: the index is what lets the same Dockerfile build natively on
# both release platforms.
FROM rust:1.99.1-bookworm@sha256:48b7f435b8fc6378ce43b44f15e3a47fd8919b2825f835c39bd4bd3da50af5b8

# Node is needed for the WASM smoke test and the web demo. Copy the official
# image's install so we do not hand-roll a Node source list. Test-only pin.
COPY --from=node:24.22.1-bookworm-slim@sha256:55fb5f5a527ea2c5c4a95f963e48ef90aa38c7b346054fdc8ee9957a2b55d205 /usr/local /usr/local

# No apt-get: the rust bookworm image already carries ca-certificates, curl and
# git, and the only packages the old block added (jq, libssl-dev) were unused.
# Removing it removes the only unbounded package fetch in the image build.

RUN rustup target add wasm32-unknown-unknown \
 && rustup component add rustfmt clippy

# wasm-pack has no arm64 prebuilt binary, so compile it once in the image.
# Pinned for reproducibility; bump deliberately (0.13.1 is known to change
# wasm-bindgen/wasm-opt output).
ARG WASM_PACK_VERSION=0.13.1
RUN cargo install wasm-pack --version "${WASM_PACK_VERSION}" --locked \
 && rm -rf "$CARGO_HOME/registry/src" "$CARGO_HOME/registry/cache"

# Bake wasm-pack's downloaded tools (wasm-bindgen 0.2.129 + wasm-opt 117) so a
# build never has to hit the network. wasm-pack resolves its cache through
# dirs::cache_dir(), i.e. $XDG_CACHE_HOME/.wasm-pack; HOME is /tmp at runtime,
# so the XDG variable is the one that matters. The throwaway crate is deleted
# and the registry source cache is removed again to keep the layer small.
ENV XDG_CACHE_HOME=/opt/wasm-pack-cache
RUN mkdir -p /tmp/wasm-pack-warm/src \
 && printf '%s\n' \
      '[package]' \
      'name = "wasm-pack-warm"' \
      'version = "0.0.0"' \
      'edition = "2021"' \
      '' \
      '[lib]' \
      'crate-type = ["cdylib"]' \
      '' \
      '[dependencies]' \
      'wasm-bindgen = "=0.2.129"' \
      > /tmp/wasm-pack-warm/Cargo.toml \
 && printf '%s\n' 'pub fn warm() {}' > /tmp/wasm-pack-warm/src/lib.rs \
 && cd /tmp/wasm-pack-warm \
 && wasm-pack build --target web --release --out-dir /tmp/wasm-pack-out \
 && cd / \
 && rm -rf /tmp/wasm-pack-warm /tmp/wasm-pack-out \
 && rm -rf "$CARGO_HOME/registry/src" "$CARGO_HOME/registry/cache" \
 && chmod -R 777 /opt/wasm-pack-cache \
 && test -x /opt/wasm-pack-cache/.wasm-pack/wasm-bindgen-*/wasm-bindgen \
 && test -x /opt/wasm-pack-cache/.wasm-pack/wasm-opt-*/bin/wasm-opt

ENV CARGO_TERM_COLOR=always \
    RUST_BACKTRACE=1

WORKDIR /work
