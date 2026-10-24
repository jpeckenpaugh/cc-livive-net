// Node smoke test for the wasm-bindgen wrapper (D026).
//
// Loads the CommonJS `--target nodejs` build through createRequire, so no root
// package.json is needed and the ESM/CJS boundary stays explicit.

import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { gzipSync } from "node:zlib";

const require = createRequire(import.meta.url);
const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
const pkgNode = join(root, "web", "pkg-node");
const wasm = require(join(pkgNode, "color_cube_wasm.js"));

let assertions = 0;

function ok(condition, message) {
  assertions += 1;
  if (!condition) {
    throw new Error(`assertion failed: ${message}`);
  }
}

function equal(actual, expected, message) {
  ok(actual === expected, `${message} (got ${actual}, expected ${expected})`);
}

function throws(fn, message) {
  assertions += 1;
  try {
    fn();
  } catch {
    return;
  }
  throw new Error(`assertion failed: ${message} did not throw`);
}

function sameBytes(a, b) {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i += 1) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

function sameValues(actual, expected) {
  return sameBytes(Array.from(actual), expected);
}

// --- static surface -------------------------------------------------------

const solved = wasm.solved_facelets();
equal(solved.length, 54, "solved_facelets length");
ok(solved.every((value) => value >= 0 && value < 6), "colour indices in 0..6");

const names = wasm.move_names();
equal(names.length, 18, "move_names length");
equal(names[0], "U", "first move name");
equal(names[17], "B'", "last move name");

const action = wasm.action_mask();
equal(action.length, 18, "action_mask length");
ok(action.every((value) => value === 1), "action_mask is all ones");

const unrestricted = wasm.canonical_mask(undefined);
equal(unrestricted.filter((value) => value === 1).length, 18, "canonical_mask(None)");
const afterU = wasm.canonical_mask(0);
equal(afterU.filter((value) => value === 1).length, 15, "canonical_mask(U) prunes U moves");
throws(() => wasm.canonical_mask(200), "canonical_mask out-of-range index");

// --- scramble -> solve -> replay -----------------------------------------

const scramble = wasm.scramble(42);
const scrambleMoves = scramble.split(" ").filter(Boolean);
equal(scrambleMoves.length, 25, "scramble move count");
ok(scrambleMoves.every((token) => names.includes(token)), "scramble uses valid labels");

const facelets = wasm.facelets_after(scramble);
equal(facelets.length, 54, "scrambled facelets length");
ok(!sameBytes(facelets, solved), "scramble is not the solved cube");

const solution = wasm.solve(facelets);
ok(solution.length > 0, "solve returned moves");
ok(solution.split(" ").filter(Boolean).length <= 45, "solution within 45 moves");
const replayed = wasm.apply_facelets(facelets, solution);
ok(sameBytes(replayed, solved), "solution replays to solved from the scrambled facelets");
const replayFromSolved = wasm.facelets_after(`${scramble} ${solution}`);
ok(sameBytes(replayFromSolved, solved), "scramble + solution replays from solved via facelets_after");

const report = wasm.solve_report(facelets);
equal(report.moves, solution, "solve_report moves match solve");
ok(report.phase1 <= 7 && report.phase2 <= 10, "phase bounds respected");
ok(report.elapsed_ms >= 0, "elapsed_ms is non-negative");
equal(report.solver, "thistlethwaite", "solver name");

equal(wasm.solve(solved), "", "solve(solved) is empty");
const solvedReport = wasm.solve_report(solved);
equal(solvedReport.moves, "", "solve_report(solved) is an empty report");
equal(solvedReport.phase1, 0, "solved report phase1");

// --- observation ----------------------------------------------------------

const observation = wasm.observe(facelets);
equal(observation.length, 324, "observation length");
const ones = Array.from(observation).filter((value) => value === 1).length;
equal(ones, 54, "observation has exactly 54 ones");
let sum = 0;
for (const value of observation) sum += value;
equal(sum, 54, "observation sums to 54");
let oneHot = true;
for (let facelet = 0; facelet < 54; facelet += 1) {
  let block = 0;
  for (let colour = 0; colour < 6; colour += 1) block += observation[facelet * 6 + colour];
  if (block !== 1) oneHot = false;
}
ok(oneHot, "each facelet block is one-hot");

// --- malformed input ------------------------------------------------------

throws(() => wasm.solve(new Uint8Array(53)), "short facelet array");
throws(() => wasm.solve(new Uint8Array(54).fill(6)), "out-of-range colour");
throws(() => wasm.facelets_after("U2 Q"), "unknown move token");
throws(() => wasm.observe(new Uint8Array(54).fill(9)), "observe rejects bad colours");

// --- inspector view -------------------------------------------------------

const view = wasm.cube_view(solved);
ok(sameValues(view.cp, [0, 1, 2, 3, 4, 5, 6, 7]), "cube_view cp of solved");
ok(sameValues(view.co, [0, 0, 0, 0, 0, 0, 0, 0]), "cube_view co of solved");
ok(
  sameValues(view.ep, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
  "cube_view ep of solved",
);
ok(sameValues(view.eo, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]), "cube_view eo of solved");
equal(view.twist, 0, "solved twist");
equal(view.flip, 0, "solved flip");
equal(view.slice, 494, "solved slice");
equal(view.cp_rank, 0, "solved cp_rank");
equal(view.ep8_rank, 0, "solved ep8_rank");
equal(view.sp_rank, 0, "solved sp_rank");
equal(view.is_valid, true, "solved is_valid");
equal(view.is_solved, true, "solved is_solved");
equal(view.cube_bytes, 40, "Cube size at the wasm boundary");

const afterR = wasm.cube_view(wasm.facelets_after("R"));
equal(afterR.ep8_rank, undefined, "ep8_rank is undefined outside G1");
equal(afterR.sp_rank, undefined, "sp_rank is undefined outside G1");
ok(afterR.slice !== 494, "R dirties the slice");
ok(afterR.cp_rank >= 0, "cp_rank stays total outside G1");

throws(() => wasm.cube_view(new Uint8Array(53)), "cube_view rejects short facelets");
throws(() => wasm.cube_view(new Uint8Array(54).fill(9)), "cube_view rejects bad colours");

const meta = JSON.parse(wasm.data_structure_meta());
ok(Array.isArray(meta) && meta.length >= 13, "structure metadata has every entry");
const metaIds = new Set(meta.map((entry) => entry.id));
for (const id of [
  "cube",
  "move",
  "color",
  "scramble",
  "solution",
  "limits",
  "cube-error",
  "twist",
  "flip",
  "slice",
  "cp-rank",
  "ep8-rank",
  "sp-rank",
]) {
  ok(metaIds.has(id), `structure metadata includes ${id}`);
}
for (const entry of meta) {
  ok(typeof entry.rust === "string" && entry.rust.length > 0, `${entry.id} has a declaration`);
  ok(typeof entry.source?.file === "string", `${entry.id} names a source file`);
}

// --- conformance suite ----------------------------------------------------

const start = performance.now();
const conformance = wasm.check();
const elapsed = performance.now() - start;
equal(conformance.failed, 0, "check() has no failures");
ok(conformance.passed >= 8, "check() ran the core suite");
console.log(conformance.details.trimEnd());

// --- sizes ----------------------------------------------------------------

const wasmBytes = readFileSync(join(pkgNode, "color_cube_wasm_bg.wasm"));
const gzipped = gzipSync(wasmBytes, { level: 9 });

console.log(
  `PASS: ${assertions} assertions; solution ${solution.split(" ").filter(Boolean).length} moves; ` +
    `check ${conformance.passed} passed / ${conformance.failed} failed in ${elapsed.toFixed(1)} ms; ` +
    `wasm ${wasmBytes.length} bytes raw / ${gzipped.length} bytes gzip`,
);
