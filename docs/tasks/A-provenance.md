# A — Deterministic build, provenance, and published evidence

**Read first:** `concept.md` §10, §13; `docs/DECISIONS.md`; `docker/Dockerfile`;
`docker/Dockerfile.browser`; `scripts/`; `.github/workflows/`.

**Mode:** question-first. Return questions before writing files. No git. Use
`./scripts/docker.sh`.

## Context (measured facts — do not re-derive)

The current wasm is already reproducible across architecture and path:

| build | platform | path | SHA-256 |
|---|---|---|---|
| local | linux/arm64 | `/work` | `dd15946538949fccc61f1e51452696e0e303618ff34f90d4e508f70f4a691efd` |
| local alt | linux/arm64 | `/alt` | same |
| Pages CI | linux/amd64 | `/work` | same |

Tool versions today: rustc 1.99.1, cargo 1.99.1, wasm-pack 0.13.1,
wasm-bindgen 0.2.129, wasm-opt (binaryen) 117, node 24.22.1.

Container **images** are not bit-reproducible (Docker layer timestamps); the
**artifact** is. Provenance must say exactly that.

## Goals

1. Pin every input that can change the wasm, and remove inputs that cannot.
2. Emit a machine-readable `provenance.json` and show it in the demo.
3. Prove reproducibility with a script and a CI job that fail on a hash
   mismatch.
4. Never claim more than was measured.

## Decisions already made

- **D030:** CI on linux/amd64 is the canonical release platform. Local native
  builds are supported and must match; a cross-arch repro job enforces it.
- **D031:** provenance distinguishes pinned inputs, build identity (image ID,
  recorded not claimed reproducible), and artifact identity (SHA-256).
- **D032:** no `linux/amd64` forcing for local dev (emulation is slow); the
  reproducibility check is what guarantees equivalence.

## Work

### 1. Dockerfile pinning (`docker/Dockerfile`)

- `FROM rust:1.99.1-bookworm@sha256:48b7f435b8fc6378ce43b44f15e3a47fd8919b2825f835c39bd4bd3da50af5b8`
- `COPY --from=node:24.22.1-bookworm-slim@sha256:55fb5f5a527ea2c5c4a95f963e48ef90aa38c7b346054fdc8ee9957a2b55d205`
- Delete the `apt-get` block entirely (`jq` and `libssl-dev` are unused; this
  removes the only unbounded package fetch). Verify nothing breaks.
- Keep wasm-pack 0.13.1 pinned. Do **not** upgrade (it changes output).
- Bake wasm-pack's downloaded tools: during image build, build a throwaway crate
  with `wasm-bindgen = "=0.2.129"` via wasm-pack, delete the crate, and keep the
  cache at `/opt/wasm-pack-cache` (`chmod -R 777` for the runtime uid). Set
  `XDG_CACHE_HOME=/opt/wasm-pack-cache` so builds do not touch the network.
- Record in the Dockerfile comments *which* pins affect the wasm and which are
  test-only (Node is test-only).

### 2. Build flags

- `RUSTFLAGS="--remap-path-prefix=/work=/src --remap-path-prefix=/cargo=/cargo"`
- `CARGO_INCREMENTAL=0`
- `SOURCE_DATE_EPOCH` from the commit timestamp, passed by the scripts (not
  hard-coded in the image).
- Add `[profile.release] strip = "debuginfo"` to the workspace `Cargo.toml` and
  confirm native release and tests still pass.

### 3. `scripts/provenance.mjs`

Runs after the wasm build and writes `web/provenance.json` (+ a plain-text
summary at `web/provenance.txt`). Schema:

```json
{
  "schema": 1,
  "generated_at": "RFC3339",
  "source": { "repo", "commit", "tree", "dirty", "cargo_lock_sha256" },
  "toolchain": { "rustc", "cargo", "target", "wasm_pack", "wasm_bindgen",
                 "wasm_opt", "node", "docker" },
  "images": { "rust_base", "node_base", "dev_image_id", "platform" },
  "build": { "rustflags", "profile", "source_date_epoch" },
  "artifacts": { "wasm": {"sha256","bytes"}, "js": {"sha256","bytes"},
                 "wasm_gzip": {"sha256","bytes"},
                 "tables": { "moves", "thistlethwaite": {"sha256","bytes"} } },
  "evidence": { "check": "pass", "node_assertions": 35,
                "browser_assertions": 10, "table_diameters": {...} },
  "ci": { "run_url": "..." }
}
```

- `generated_at` and `ci` are metadata, not part of artifact identity; document
  that.
- Missing values are `null`, never invented.
- Include the exact verify command string.

### 4. `scripts/repro-check.sh`

- Build the wasm twice in clean containers, at different mount paths (`/work`
  and `/alt`) and different target dirs, with all pins applied.
- Compare SHA-256; also compare after gzip.
- `--platform linux/amd64` flag (best effort) for cross-arch checking.
- Print a single PASS/FAIL line and exit non-zero on mismatch.
- Do not depend on the local `tables/` contents: regenerate or verify their
  hashes too.

### 5. Site: Provenance panel

- A collapsible panel on `web/index.html` that fetches `./provenance.json` and
  renders: commit link, wasm SHA-256 (truncated with copy button), rustc,
  wasm-pack, wasm-bindgen, wasm-opt, base image digests, platform, CI run link,
  and the verify command.
- Degrade gracefully when the JSON is absent (local dev without the step).
- Link to `/provenance.json` and `/provenance.txt`.
- No layout regressions at 375 px; keep the browser test green.

### 6. CI

- Pages workflow: generate provenance after the wasm build, before upload; set
  `GITHUB_RUN_ID`/repository/commit env so the JSON can include the run URL.
- CI: add a `repro` job that runs `scripts/repro-check.sh` twice (native, and
  `--platform linux/amd64` if available) and fails on mismatch. It must not
  block if emulation is unavailable — detect and skip with a printed reason.
- Optional: GitHub artifact attestation for the Pages artifact; if it is cheap,
  add it, otherwise leave a TODO in the workflow.

### 7. Docs

- `docs/PROVENANCE.md`: the exact claim we make, the evidence, what is and is
  not reproducible, how to verify, and how to update pins (checklist).
- Update `README.md` and `docs/STATUS.md`.
- Record D030–D032.

## Acceptance

- `./scripts/check.sh` and `./scripts/browser-smoke.sh` pass after a clean image
  rebuild.
- `./scripts/repro-check.sh` passes on this machine (native) and reports the
  same hash as the live site.
- `web/provenance.json` is valid JSON, matches the schema, and every hash in it
  matches the file it names.
- Removing `tables/` and `web/pkg/` still works from a clean state.
- No claim in docs that the container itself is reproducible.

## Reporting

Pins used, image size delta, cache-bake result, new wasm hash (it will change
because of RUSTFLAGS), repro-check output, JSON sample, CI job summary, and
anything unverified. Under 900 words.
