// Writes web/provenance.json and web/provenance.txt (workstream A).
//
//   ./scripts/docker.sh node scripts/provenance.mjs
//
// Run after the web wasm build. Values that are not observable are null; the
// script never guesses. Artifact identity is the SHA-256 of the wasm and the
// generated JS. `generated_at`, CI metadata, tool/image versions, platform,
// evidence counts and the advisory gzip hash are recorded metadata, not
// identity (D031; docs/PROVENANCE.md).

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const WEB = join(ROOT, "web");
const PKG = join(WEB, "pkg");
const WASM_FILE = join(PKG, "color_cube_wasm_bg.wasm");
const JS_FILE = join(PKG, "color_cube_wasm.js");
const MOVES_TABLE = join(ROOT, "tables", "cc-tables-v1.bin");
const THISTLE_TABLE = join(ROOT, "tables", "cc-thistlethwaite-v1.bin");

const RUST_BASE = "rust:1.99.1-bookworm@sha256:48b7f435b8fc6378ce43b44f15e3a47fd8919b2825f835c39bd4bd3da50af5b8";
const NODE_BASE = "node:24.22.1-bookworm-slim@sha256:55fb5f5a527ea2c5c4a95f963e48ef90aa38c7b346054fdc8ee9957a2b55d205";

function run(command, args) {
  try {
    return execFileSync(command, args, {
      encoding: "utf8",
      cwd: ROOT,
      stdio: ["ignore", "pipe", "ignore"],
    }).trim() || null;
  } catch {
    return null;
  }
}

function git(args) {
  return run("git", ["-C", ROOT, ...args]);
}

function sha256(data) {
  return createHash("sha256").update(data).digest("hex");
}

function fileArtifact(path) {
  if (!existsSync(path)) return null;
  const bytes = readFileSync(path);
  return { sha256: sha256(bytes), bytes: bytes.length };
}

function integerEnv(name) {
  const raw = process.env[name];
  if (raw === undefined || raw === "") return null;
  const value = Number(raw);
  return Number.isFinite(value) ? Math.trunc(value) : null;
}

function toolVersion(command) {
  return run(command, ["--version"]);
}

function findCachedTool(directoryPrefix, relativeExecutable) {
  const roots = [
    process.env.XDG_CACHE_HOME ? join(process.env.XDG_CACHE_HOME, ".wasm-pack") : null,
    process.env.HOME ? join(process.env.HOME, ".cache", ".wasm-pack") : null,
    "/opt/wasm-pack-cache/.wasm-pack",
  ].filter(Boolean);
  for (const root of roots) {
    if (!existsSync(root)) continue;
    let entries;
    try {
      entries = readdirSync(root);
    } catch {
      continue;
    }
    for (const entry of entries) {
      if (!entry.startsWith(directoryPrefix)) continue;
      const executable = join(root, entry, relativeExecutable);
      if (existsSync(executable)) return run(executable, ["--version"]);
    }
  }
  return null;
}

function normalizePlatform(raw) {
  if (!raw) return null;
  return raw
    .replace(/\/aarch64$/, "/arm64")
    .replace(/\/x86_64$/, "/amd64");
}

// The CCTH bundle header is frozen in crates/color-cube-solver/src/tables.rs:
// magic(4) version(2) count(4) payload_len(8) param_hash(8) checksum(8), then
// count × (id u32, rows u32, cols u32, width u8). Distance payloads are one
// byte per entry; phase 4 uses 255 as "unreachable" and is excluded from max.
function tableDiameters(path) {
  if (!existsSync(path)) return null;
  const data = readFileSync(path);
  if (data.length < 34 || data.toString("ascii", 0, 4) !== "CCTH") return null;
  const count = data.readUInt32LE(6);
  const payloadStart = 34 + count * 13;
  let offset = payloadStart;
  const names = {
    1: "phase1",
    2: "phase2",
    3: "phase3",
    4: "phase4_cp",
    5: "phase4_ep",
  };
  const out = {};
  for (let index = 0; index < count; index += 1) {
    const entry = 34 + index * 13;
    const id = data.readUInt32LE(entry);
    const rows = data.readUInt32LE(entry + 4);
    const cols = data.readUInt32LE(entry + 8);
    const width = data[entry + 12];
    const size = rows * cols * width;
    if (offset + size > data.length) return null;
    if (names[id] && width === 1) {
      let max = 0;
      for (let cursor = offset; cursor < offset + size; cursor += 1) {
        const value = data[cursor];
        if (value !== 255 && value > max) max = value;
      }
      out[names[id]] = max;
    }
    offset += size;
  }
  return Object.keys(out).length === 5 ? out : null;
}

function repoUrl() {
  if (process.env.GITHUB_REPOSITORY) {
    const server = process.env.GITHUB_SERVER_URL || "https://github.com";
    return `${server}/${process.env.GITHUB_REPOSITORY}`;
  }
  const remote = git(["config", "--get", "remote.origin.url"]);
  if (!remote) return null;
  if (/^git@[^:]+:/.test(remote)) {
    return `https://${remote.replace(/^git@([^:]+):/, "$1/").replace(/\.git$/, "")}`;
  }
  return remote.replace(/\.git$/, "");
}

function ciMetadata() {
  const runId = process.env.GITHUB_RUN_ID || null;
  const repo = process.env.GITHUB_REPOSITORY || null;
  const server = process.env.GITHUB_SERVER_URL || "https://github.com";
  const runUrl = runId && repo ? `${server}/${repo}/actions/runs/${runId}` : null;
  return {
    run_url: runUrl,
    run_id: runId,
    workflow: process.env.GITHUB_WORKFLOW || null,
    attempt: integerEnv("GITHUB_RUN_ATTEMPT"),
  };
}

if (!existsSync(WASM_FILE) || !existsSync(JS_FILE)) {
  console.error("provenance: missing web/pkg wasm or js; run the wasm build first");
  process.exit(1);
}

const wasmBytes = readFileSync(WASM_FILE);
const wasmGzip = gzipSync(wasmBytes, { level: 9 });
const commit = process.env.GITHUB_SHA || git(["rev-parse", "HEAD"]);
const tree = git(["rev-parse", "HEAD^{tree}"]);
const gitStatus = git(["status", "--porcelain", "--untracked-files=no"]);
const dirty = gitStatus === null ? null : gitStatus !== "";
const remote = repoUrl();
const epochFromEnv = integerEnv("SOURCE_DATE_EPOCH");
const epochFromGit = Number.parseInt(git(["log", "-1", "--format=%ct"]) ?? "", 10);

const provenance = {
  schema: 1,
  generated_at: new Date().toISOString(),
  source: {
    repo: remote,
    commit,
    tree,
    dirty,
    cargo_lock_sha256: fileArtifact(join(ROOT, "Cargo.lock"))?.sha256 ?? null,
  },
  toolchain: {
    rustc: toolVersion("rustc"),
    cargo: toolVersion("cargo"),
    target: "wasm32-unknown-unknown",
    wasm_pack: toolVersion("wasm-pack"),
    wasm_bindgen: findCachedTool("wasm-bindgen-", "wasm-bindgen"),
    wasm_opt: findCachedTool("wasm-opt-", join("bin", "wasm-opt")),
    node: process.version,
    docker: process.env.DOCKER_VERSION || null,
  },
  images: {
    rust_base: RUST_BASE,
    node_base: NODE_BASE,
    dev_image_id: process.env.DEV_IMAGE_ID || null,
    platform: normalizePlatform(
      process.env.DOCKER_PLATFORM
        || `linux/${process.arch === "x64" ? "amd64" : process.arch}`,
    ),
  },
  build: {
    rustflags: process.env.RUSTFLAGS || null,
    profile: "release",
    source_date_epoch: Number.isFinite(epochFromEnv) ? epochFromEnv
      : Number.isFinite(epochFromGit) ? epochFromGit : null,
  },
  artifacts: {
    wasm: { sha256: sha256(wasmBytes), bytes: wasmBytes.length },
    js: fileArtifact(JS_FILE),
    wasm_gzip: { sha256: sha256(wasmGzip), bytes: wasmGzip.length },
    tables: {
      moves: fileArtifact(MOVES_TABLE),
      thistlethwaite: fileArtifact(THISTLE_TABLE),
    },
  },
  evidence: {
    check: process.env.CC_CHECK || null,
    node_assertions: integerEnv("CC_NODE_ASSERTIONS"),
    browser_assertions: integerEnv("CC_BROWSER_ASSERTIONS"),
    table_diameters: tableDiameters(THISTLE_TABLE),
  },
  ci: ciMetadata(),
  verify: {
    command: "shasum -a 256 web/pkg/color_cube_wasm_bg.wasm",
    expected_wasm_sha256: sha256(wasmBytes),
  },
};

const json = `${JSON.stringify(provenance, null, 2)}\n`;
writeFileSync(join(WEB, "provenance.json"), json);

const value = (v) => (v === null || v === undefined ? "—" : String(v));
const lines = [
  "cc.livive.net build provenance",
  `generated: ${provenance.generated_at}`,
  `commit: ${value(provenance.source.commit)}${provenance.source.dirty ? " (dirty)" : ""}`,
  `tree: ${value(provenance.source.tree)}`,
  `wasm sha256: ${provenance.artifacts.wasm.sha256}`,
  `wasm bytes: ${provenance.artifacts.wasm.bytes}`,
  `js sha256: ${value(provenance.artifacts.js?.sha256)}`,
  `wasm gzip sha256 (advisory, not identity): ${provenance.artifacts.wasm_gzip.sha256}`,
  `rustc: ${value(provenance.toolchain.rustc)}`,
  `wasm-pack: ${value(provenance.toolchain.wasm_pack)}`,
  `wasm-bindgen: ${value(provenance.toolchain.wasm_bindgen)}`,
  `wasm-opt: ${value(provenance.toolchain.wasm_opt)}`,
  `platform: ${value(provenance.images.platform)}`,
  `verify: ${provenance.verify.command}`,
  "",
  "Artifact identity is the wasm+js SHA-256. Container images are recorded, not",
  "claimed reproducible. See docs/PROVENANCE.md.",
  "",
];
writeFileSync(join(WEB, "provenance.txt"), lines.join("\n"));
console.log(`wrote web/provenance.json (wasm ${provenance.artifacts.wasm.sha256})`);
