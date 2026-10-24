# Provenance and reproducibility

## The claim

The **web wasm artifact** is reproducible: two clean builds of the same commit,
in the same pinned image, at different mount paths (`/work`, `/alt`), with
fresh target directories and regenerated tables, produce the same SHA-256 for
the wasm and the generated JS. The canonical release platform is
`linux/amd64` (D030); native `linux/arm64` builds use the same manifest-list
image pins and are checked with `scripts/repro-check.sh`.

The **container image is not reproducible** and is not claimed to be: Docker
layer timestamps and the local image ID are recorded as build metadata only
(D031). Node is test-only and cannot change the wasm; its pin is recorded for
completeness.

Artifact identity is exactly:

- `artifacts.wasm.sha256`
- `artifacts.js.sha256`

Everything else in `web/provenance.json` — `generated_at`, source commit/tree,
tool versions, image IDs, platform, evidence counts, table diameters and the
advisory gzip hash — is recorded metadata. `ci` metadata is not identity.
`wasm_gzip` is produced with mtime 0 for a stable human-facing size, but gzip
bytes differ between zlib and GNU gzip; `repro-check.sh` compares GNU gzip
output only within one image. It is never part of artifact identity.

## Pinned inputs

| Input | Pin |
|---|---|
| Rust base | `rust:1.99.1-bookworm@sha256:48b7f435b8fc6378ce43b44f15e3a47fd8919b2825f835c39bd4bd3da50af5b8` |
| Node (test-only) | `node:24.22.1-bookworm-slim@sha256:55fb5f5a527ea2c5c4a95f963e48ef90aa38c7b346054fdc8ee9957a2b55d205` |
| wasm-pack | `0.13.1` (`--locked`) |
| wasm-bindgen CLI | `0.2.129` (baked into the image cache) |
| wasm-opt / binaryen | `117` (baked into the image cache) |
| Dependencies | `Cargo.lock` (committed, D012) |
| Table generator + conventions | `crates/color-cube-solver/src/**` and the `CONVENTIONS` literal |
| Rust flags | `--remap-path-prefix=/work=/src --remap-path-prefix=/alt=/src`, `CARGO_INCREMENTAL=0` |
| Profile | `[profile.release] strip = "debuginfo"` |

Both image references are multi-arch manifest lists (amd64 + arm64), verified
with `docker buildx imagetools inspect`. Do not replace them with per-platform
child digests: that would break native builds on one of the two platforms.

`scripts/docker.sh` exports the path remaps and `CARGO_INCREMENTAL=0` for every
cargo/wasm-pack invocation, so the shared target volume does not thrash when
`check.sh` and the wasm build alternate. `SOURCE_DATE_EPOCH` is set from the
HEAD commit timestamp by the wrapper; a dirty tree is marked `dirty: true` and
is not release evidence. `rustc 1.99.1` and `wasm-opt 117` did not change their
output when this variable changed in a controlled same-output-path test, so it
is currently recorded metadata rather than a proven input.

The image bakes wasm-pack's tool cache under `XDG_CACHE_HOME=/opt/wasm-pack-cache`
(`$XDG_CACHE_HOME/.wasm-pack`), mode `777`, and deletes the throwaway crate. A
build with `--network none`, `HOME` pointing at an empty directory, and no
`~/.wasm-pack` completes using only that cache. If wasm-pack changes its cache
resolution, the offline test in the image-rebuild checklist must be repeated
before trusting a release.

## Verify a published build

```sh
shasum -a 256 web/pkg/color_cube_wasm_bg.wasm
```

Compare with `artifacts.wasm.sha256` in `https://cc.livive.net/provenance.json`
(or `web/provenance.json` locally). The demo's Provenance panel is lazy-loaded
and shows the same values. It fetches `./provenance.json`, so it works under a
GitHub Pages project subpath; a missing or malformed file renders an
"unavailable" state instead of an error.

## Reproduce locally

```sh
./scripts/repro-check.sh                 # clean HEAD, native platform
./scripts/repro-check.sh --platform linux/amd64   # cross-arch, best effort
```

The script refuses tracked modifications (use `--allow-dirty` only for local
debugging; it is not evidence), builds twice at `/work` and `/alt` in throwaway
volumes, regenerates tables in each build, checks free disk, and compares wasm,
JS, GNU-gzip wasm, and both table bundle hashes. It prints a single `PASS` or
`FAIL` line and exits non-zero on mismatch. Missing emulation is a printed
`SKIP`, not a failure. The current run's one-line result is in the gate log.

`scripts/check.sh` also generates `web/provenance.json` and
`web/provenance.txt` after the Node smoke, so local `check.sh` keeps the panel
fresh. CI's `wasm` job does the same and records the smoke assertion count; the
Pages workflow generates provenance before uploading `web/`.

The table diameters are read back from `cc-thistlethwaite-v1.bin`: 7/10/13 for
phases 1–3 and 4/8 for the phase-4 corner and full-edge heuristic tables. The
phase-4 **search bound** is 15 (D021); the heuristic tables cover only the
reachable `G3` states, so their maxima are smaller. Do not report them as the
phase-4 diameter.

## Updating pins

1. Resolve the new tag and inspect it:
   `docker buildx imagetools inspect <ref>`; confirm `linux/amd64` and
   `linux/arm64` are present, then copy the **index** digest.
2. Update the `FROM`/`COPY --from` lines in `docker/Dockerfile` and the two
   constants in `scripts/provenance.mjs`; update the table above.
3. Rebuild the image and prove the baked cache offline (`--network none`, empty
   `HOME`, no `~/.wasm-pack`).
4. Run `./scripts/check.sh`, then `./scripts/repro-check.sh` on a clean commit.
5. Run `./scripts/repro-check.sh --platform linux/amd64` (or arm64) and keep
   the output in the gate log.
6. Do not bump wasm-pack, wasm-bindgen or wasm-opt casually: they change the
   wasm. Record the new artifact hash in the commit/report.

Container images are never called reproducible in docs, commit messages, or
the demo.
