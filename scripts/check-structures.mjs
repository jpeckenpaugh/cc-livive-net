// Drift guard for web/structures.json (workstream B).
//
// Every `rust` snippet must be a verbatim, contiguous block of its named source
// file: lines are compared after trimming trailing whitespace, but tokens and
// indentation are never normalised. Byte sizes are asserted against a
// hard-coded table so a layout change cannot silently reach the page.
//
//   node scripts/check-structures.mjs
//
// Runs on the host or in the dev image; no dependencies.

import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

// Sizes are contract, not UI data. Types whose layout is not fixed (Vec,
// Option, non_exhaustive enums) are `null` in structures.json and absent here.
const EXPECTED_BYTES = new Map([
  ["cube", 40],
  ["move", 1],
  ["color", 1],
  ["scramble", 33],
  ["twist", 2],
  ["flip", 2],
  ["slice", 2],
  ["cp-rank", 2],
  ["ep8-rank", 2],
  ["sp-rank", 2],
]);

const REQUIRED_KEYS = ["id", "name", "kind", "rust", "fields", "invariants", "bytes", "source"];
const KINDS = new Set(["core", "solver", "coordinate"]);

function trimTrailing(text) {
  return text
    .split("\n")
    .map((line) => line.replace(/[ \t]+$/u, ""))
    .join("\n");
}

const failures = [];
let checked = 0;

function fail(message) {
  failures.push(message);
}

let entries;
try {
  entries = JSON.parse(readFileSync(join(root, "web", "structures.json"), "utf8"));
} catch (error) {
  console.error(`FAIL: cannot read web/structures.json: ${error.message}`);
  process.exit(1);
}

if (!Array.isArray(entries) || entries.length === 0) {
  console.error("FAIL: web/structures.json must be a non-empty array");
  process.exit(1);
}

const seen = new Set();
for (const entry of entries) {
  const label = entry?.id ?? "(missing id)";
  for (const key of REQUIRED_KEYS) {
    if (!(key in (entry ?? {}))) fail(`${label}: missing key \`${key}\``);
  }
  if (typeof entry?.id !== "string" || entry.id.length === 0) {
    fail(`${label}: id must be a non-empty string`);
    continue;
  }
  if (seen.has(entry.id)) fail(`${label}: duplicate id`);
  seen.add(entry.id);
  if (!KINDS.has(entry.kind)) fail(`${label}: unknown kind \`${entry.kind}\``);
  if (!Array.isArray(entry.fields) || entry.fields.length === 0) {
    fail(`${label}: fields must be a non-empty array`);
  }
  if (!Array.isArray(entry.invariants) || entry.invariants.length === 0) {
    fail(`${label}: invariants must be a non-empty array`);
  }
  if (typeof entry.source?.file !== "string" || typeof entry.source?.symbol !== "string") {
    fail(`${label}: source.file and source.symbol must be strings`);
    continue;
  }
  if (typeof entry.rust !== "string" || entry.rust.trim().length === 0) {
    fail(`${label}: rust must be a non-empty string`);
    continue;
  }

  const expected = EXPECTED_BYTES.get(entry.id);
  if (expected === undefined) {
    if (entry.bytes !== null) {
      fail(`${label}: bytes must be null for a type without a fixed layout`);
    }
  } else if (entry.bytes !== expected) {
    fail(`${label}: bytes is ${entry.bytes}, expected ${expected}`);
  }

  let source;
  try {
    source = readFileSync(join(root, entry.source.file), "utf8");
  } catch (error) {
    fail(`${label}: cannot read ${entry.source.file}: ${error.message}`);
    continue;
  }

  const haystack = trimTrailing(source);
  const needle = trimTrailing(entry.rust);
  if (!haystack.includes(needle)) {
    const firstMissing = needle
      .split("\n")
      .map((line) => line.trimEnd())
      .find((line) => line.trim().length > 0 && !haystack.includes(line));
    fail(
      `${label}: rust snippet is not a verbatim block of ${entry.source.file}` +
        (firstMissing ? ` (first unmatched line: \`${firstMissing.trim()}\`)` : ""),
    );
    continue;
  }
  checked += 1;
}

if (failures.length > 0) {
  console.error(`FAIL: ${failures.length} structures.json problem(s)`);
  for (const failure of failures) console.error(` - ${failure}`);
  process.exit(1);
}

console.log(
  `PASS: ${checked} structure snippet(s) verified against source and ${EXPECTED_BYTES.size} byte size(s) match`,
);
