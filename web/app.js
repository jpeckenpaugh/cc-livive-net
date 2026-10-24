// Stateless browser front end (D026/D028): the only cube-shaped value in JS is
// the last 54-facelet snapshot returned by the wasm module. Every operation
// sends that snapshot back with a move string and stores the new result. The
// canvas renderer adds camera state, never move logic.
import init, * as cube from "./pkg/color_cube_wasm.js";
import { createRenderer } from "./renderer.js";

const FACES = ["U", "R", "F", "D", "L", "B"];
const COLOR_LETTERS = ["U", "D", "F", "B", "R", "L"];
const PAIR_DELAY_MS = 110;
const REDUCED_MOTION = window.matchMedia("(prefers-reduced-motion: reduce)");

const canvas = document.querySelector("#cube");
const net = document.querySelector("#net");
const viewToggle = document.querySelector("#view-toggle");
const statusLine = document.querySelector("#status");
const moveCount = document.querySelector("#move-count");
const solutionLength = document.querySelector("#solution-length");
const solverTime = document.querySelector("#solver-time");
const sequenceInput = document.querySelector("#sequence");
const conformanceOutput = document.querySelector("#conformance");
const paletteToggle = document.querySelector("#cb-toggle");
const playButton = document.querySelector("#play");

const state = {
  facelets: new Uint8Array(0),
  history: [],
  solution: [],
  playing: false,
};

let moveNames = [];
let stickers = [];
let renderer = null;
let lastError = null;
let netVisible = false;

function setStatus(message, tone = "") {
  statusLine.textContent = message;
  statusLine.dataset.tone = tone;
}

function fail(error) {
  lastError = error instanceof Error ? error.message : String(error);
  setStatus(lastError, "error");
}

function buildNet() {
  const fragment = document.createDocumentFragment();
  for (let face = 0; face < 6; face += 1) {
    const faceElement = document.createElement("div");
    faceElement.className = `face face-${FACES[face]}`;
    for (let offset = 0; offset < 9; offset += 1) {
      const sticker = document.createElement("div");
      sticker.className = "sticker";
      const facelet = face * 9 + offset;
      sticker.dataset.facelet = String(facelet);
      sticker.title = `facelet ${facelet}`;
      stickers.push(sticker);
      faceElement.append(sticker);
    }
    fragment.append(faceElement);
  }
  net.replaceChildren(fragment);
}

function render() {
  for (const sticker of stickers) {
    const colour = state.facelets[Number(sticker.dataset.facelet)];
    sticker.dataset.colour = String(colour);
    sticker.textContent = COLOR_LETTERS[colour] ?? "?";
  }
  if (renderer) renderer.setFacelets(state.facelets);
}

function updateInspector() {
  try {
    const observation = cube.observe(state.facelets);
    const ones = Array.from(observation).filter((value) => value === 1).length;
    document.querySelector("#facelet-count").textContent = `${state.facelets.length}`;
    document.querySelector("#observation-shape").textContent = `${observation.length} floats`;
    document.querySelector("#observation-ones").textContent = `${ones}`;

    const last = state.history.at(-1);
    const lastIndex = last === undefined ? undefined : moveNames.indexOf(last);
    const mask = cube.canonical_mask(lastIndex);
    const legal = mask.filter((value) => value === 1).length;
    document.querySelector("#legal-moves").textContent = `${legal} / ${mask.length}`;
  } catch (error) {
    fail(error);
  }
}

function refresh() {
  render();
  moveCount.textContent = String(state.history.length);
  solutionLength.textContent = state.solution.length > 0 ? String(state.solution.length) : "—";
  playButton.disabled = state.playing || state.solution.length === 0;
  updateInspector();
}

function reset() {
  if (state.playing) return;
  state.facelets = Uint8Array.from(cube.solved_facelets());
  state.history = [];
  state.solution = [];
  sequenceInput.value = "";
  solverTime.textContent = "—";
  lastError = null;
  refresh();
  setStatus("solved state");
}

function applyToken(token) {
  state.facelets = Uint8Array.from(cube.apply_facelets(state.facelets, token));
  state.history.push(token);
}

function applySequence(text) {
  if (state.playing) return;
  const tokens = text.trim().split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return;
  try {
    state.facelets = Uint8Array.from(cube.apply_facelets(state.facelets, tokens.join(" ")));
    state.history.push(...tokens);
    state.solution = [];
    solverTime.textContent = "—";
    refresh();
    setStatus(`applied ${tokens.length} move${tokens.length === 1 ? "" : "s"}`);
  } catch (error) {
    fail(error);
  }
}

function scramble() {
  if (state.playing) return;
  try {
    const seed = (Date.now() ^ Math.floor(Math.random() * 0xffff_ffff)) >>> 0;
    const text = cube.scramble(seed);
    state.facelets = Uint8Array.from(cube.facelets_after(text));
    state.history = text.split(/\s+/).filter(Boolean);
    state.solution = [];
    solverTime.textContent = "—";
    lastError = null;
    refresh();
    setStatus(`scrambled with seed ${seed} · 25 moves`);
  } catch (error) {
    fail(error);
  }
}

function solve() {
  if (state.playing) return;
  try {
    const started = performance.now();
    const report = cube.solve_report(state.facelets);
    const wall = performance.now() - started;
    const moves = report.moves.split(/\s+/).filter(Boolean);
    state.solution = moves;
    lastError = null;
    if (moves.length === 0) {
      solverTime.textContent = "0 ms";
      setStatus("already solved");
    } else {
      solverTime.textContent = `${report.elapsed_ms.toFixed(1)} ms wasm / ${wall.toFixed(1)} ms wall`;
      setStatus(
        `${moves.length} moves · phase 1: ${report.phase1}, phase 2: ${report.phase2} · ${report.solver}`,
      );
    }
    refresh();
  } catch (error) {
    fail(error);
  }
}

function playbackDelay() {
  return REDUCED_MOTION.matches ? 0 : PAIR_DELAY_MS;
}

async function play() {
  if (state.playing || state.solution.length === 0) return;
  state.playing = true;
  setStatus("playing solution…");
  refresh();
  try {
    for (const token of state.solution) {
      applyToken(token);
      refresh();
      const delay = playbackDelay();
      if (delay > 0) await new Promise((resolve) => setTimeout(resolve, delay));
    }
    state.solution = [];
    refresh();
    setStatus("solved");
  } catch (error) {
    fail(error);
  } finally {
    state.playing = false;
    refresh();
  }
}

function buildMoveButtons() {
  const fragment = document.createDocumentFragment();
  for (const name of moveNames) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = name;
    button.addEventListener("click", () => {
      if (state.playing) return;
      try {
        applyToken(name);
        state.solution = [];
        solverTime.textContent = "—";
        refresh();
        setStatus(`applied ${name}`);
      } catch (error) {
        fail(error);
      }
    });
    fragment.append(button);
  }
  document.querySelector("#moves").replaceChildren(fragment);
}

async function runConformance() {
  const button = document.querySelector("#run-conformance");
  button.disabled = true;
  conformanceOutput.textContent = "running…";
  try {
    const started = performance.now();
    const report = cube.check();
    const elapsed = performance.now() - started;
    conformanceOutput.textContent = `${report.details.trimEnd()}\n\n${report.passed} passed, ${report.failed} failed in ${elapsed.toFixed(1)} ms`;
  } catch (error) {
    conformanceOutput.textContent = `error: ${error instanceof Error ? error.message : error}`;
  } finally {
    button.disabled = false;
  }
}

function setNetVisible(show) {
  netVisible = show;
  net.hidden = !show;
  if (renderer) {
    canvas.hidden = show;
  }
  viewToggle.setAttribute("aria-pressed", String(show));
  viewToggle.textContent = show ? "Show cube" : "Show net";
}

function wireEvents() {
  document.querySelector("#scramble").addEventListener("click", scramble);
  document.querySelector("#solve").addEventListener("click", solve);
  document.querySelector("#play").addEventListener("click", play);
  document.querySelector("#reset").addEventListener("click", reset);
  document.querySelector("#apply-sequence").addEventListener("click", () => {
    applySequence(sequenceInput.value);
  });
  sequenceInput.addEventListener("keydown", (event) => {
    if (event.key === "Enter") applySequence(sequenceInput.value);
  });
  document.querySelector("#copy-solution").addEventListener("click", async () => {
    const text = state.solution.join(" ");
    if (!text) {
      setStatus("no solution to copy");
      return;
    }
    try {
      await navigator.clipboard.writeText(text);
      setStatus("solution copied");
    } catch {
      sequenceInput.value = text;
      setStatus("clipboard unavailable; solution placed in the sequence box");
    }
  });
  document.querySelector("#run-conformance").addEventListener("click", runConformance);
  viewToggle.addEventListener("click", () => setNetVisible(!netVisible));
  paletteToggle.addEventListener("change", (event) => {
    document.body.classList.toggle("cb", event.target.checked);
    if (renderer) renderer.setPalette(event.target.checked);
  });

  window.addEventListener("keydown", (event) => {
    if (event.metaKey || event.ctrlKey || event.altKey) return;
    if (event.target instanceof HTMLInputElement) return;
    const face = event.key.toUpperCase();
    if (!"URFDLB".includes(face) || face.length !== 1) return;
    event.preventDefault();
    const token = event.shiftKey ? `${face}'` : face;
    try {
      applyToken(token);
      state.solution = [];
      solverTime.textContent = "—";
      refresh();
      setStatus(`applied ${token}`);
    } catch (error) {
      fail(error);
    }
  });
}

// Minimal read-only hook for the Playwright spec. Frozen, closure-backed, and
// never exposes the live Uint8Array or any mutation path.
function installTestHook() {
  const solvedFacelets = Array.from(cube.solved_facelets());
  const hook = {
    facelets: () => Array.from(state.facelets),
    solved: () =>
      state.facelets.length === solvedFacelets.length &&
      solvedFacelets.every((value, index) => state.facelets[index] === value),
    lastError: () => lastError,
    moveCount: () => state.history.length,
    playing: () => state.playing,
    view: () => (renderer ? renderer.view() : null),
  };
  Object.defineProperty(window, "__cc", {
    value: Object.freeze(hook),
    writable: false,
    configurable: false,
    enumerable: false,
  });
}

async function main() {
  buildNet();
  try {
    await init();
    renderer = createRenderer(canvas);
    moveNames = Array.from(cube.move_names());
    buildMoveButtons();
    wireEvents();
    setNetVisible(false);
    reset();
    installTestHook();
  } catch (error) {
    fail(error);
    setStatus(`failed to load wasm: ${error instanceof Error ? error.message : error}`, "error");
  }
}

main();
