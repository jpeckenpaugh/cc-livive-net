# syntax=docker/dockerfile:1
#
# Dev/build image for cc-livive-net. Everything (cargo, wasm-pack, node) runs in
# here; the host only needs git and an editor.
#
# Build:  ./scripts/docker.sh build
# Use:    ./scripts/docker.sh cargo test -p color-cube
#
ARG RUST_VERSION=1.99

FROM rust:${RUST_VERSION}-bookworm

# Node is needed for the WASM smoke test and the web demo. Copy the official
# image's install so we do not hand-roll a Node source list.
COPY --from=node:24-bookworm-slim /usr/local /usr/local

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      ca-certificates curl git jq pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/*

RUN rustup target add wasm32-unknown-unknown \
 && rustup component add rustfmt clippy

# wasm-pack has no arm64 prebuilt binary, so compile it once in the image.
# Pinned for reproducibility; bump deliberately.
ARG WASM_PACK_VERSION=0.13.1
RUN cargo install wasm-pack --version "${WASM_PACK_VERSION}" --locked \
 && rm -rf "$CARGO_HOME/registry/src" "$CARGO_HOME/registry/cache"

ENV CARGO_TERM_COLOR=always \
    RUST_BACKTRACE=1

WORKDIR /work
