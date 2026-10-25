// Stateless browser front end (D026/D028): the only cube-shaped value in JS is
// the last 54-facelet snapshot returned by the wasm module. Every operation
// sends that snapshot back with a move string and stores the new result. The
// canvas renderer adds camera state, never move logic.
import init, * as cube from "./pkg/color_cube_wasm.js";
import {
  deriveHints,
  FACE_INDICATOR_FACES,
  PERIMETER_FACES,
} from "./hints.js";
import { createRenderer } from "./renderer.js";

const FACES = ["U", "R", "F", "D", "L", "B"];
const COLOR_LETTERS = ["U", "D", "F", "B", "R", "L"];
const PAIR_DELAY_MS = 110;
const REDUCED_MOTION = window.matchMedia("(prefers-reduced-motion: reduce)");
const DEFAULT_REPO = "https://github.com/jpeckenpaugh/cc-livive-net";
const ARRAY_LENGTHS = { cp: 8, co: 8, ep: 12, eo: 12 };

// Source links default to `main` and are pinned to the build commit only when
// provenance reports a clean tree (D031).
const sourceBase = { repo: DEFAULT_REPO, ref: "main" };
const liveCells = {};

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
let netVisible = true;
let hintMap = {};
const hintCells = new Map();

// Perimeter hint cells live in the four 3×3 bands around the cross. Each cell
// spans the band's full depth so the 12 cells tile the bands exactly.
const PERIMETER_LAYOUT = {
  U: [
    [1, 5, 2, 6],
    [1, 6, 2, 7],
    [1, 7, 2, 8],
  ],
  L: [
    [5, 1, 6, 2],
    [6, 1, 7, 2],
    [7, 1, 8, 2],
  ],
  B: [
    [5, 14, 6, 15],
    [6, 14, 7, 15],
    [7, 14, 8, 15],
  ],
  D: [
    [11, 5, 12, 6],
    [11, 6, 12, 7],
    [11, 7, 12, 8],
  ],
};

function setStatus(message, tone = "") {
  statusLine.textContent = message;
  statusLine.dataset.tone = tone;
}

function fail(error) {
  lastError = error instanceof Error ? error.message : String(error);
  setStatus(lastError, "error");
}

let sourceLoaded = false;

/// Provenance is optional and only matters for commit-pinned source links, so
/// it is fetched when the Rust-types tab is first opened, never at page load.
async function ensureSource() {
  if (sourceLoaded) return;
  sourceLoaded = true;
  await loadSource();
  applySourceLinks();
}

async function loadSource() {
  try {
    const response = await fetch("./provenance.json", { cache: "no-cache" });
    if (!response.ok) return;
    const data = await response.json();
    const repo = data?.source?.repo;
    const commit = data?.source?.commit;
    if (typeof repo === "string" && repo.length > 0) {
      sourceBase.repo = repo.replace(/\/+$/u, "");
    }
    if (typeof commit === "string" && commit.length > 0 && data?.source?.dirty === false) {
      sourceBase.ref = commit;
    }
  } catch {
    // Provenance is optional: keep the main-branch defaults.
  }
}

function applySourceLinks() {
  document.querySelector("#source-link").href = sourceBase.repo;
  document.querySelector("#footer-repo").href = sourceBase.repo;
  for (const anchor of document.querySelectorAll("a[data-source-file]")) {
    anchor.href = `${sourceBase.repo}/blob/${sourceBase.ref}/${anchor.dataset.sourceFile}`;
  }
}

function sourceHref(file) {
  return `${sourceBase.repo}/blob/${sourceBase.ref}/${file}`;
}

function buildLiveArrays() {
  for (const [key, length] of Object.entries(ARRAY_LENGTHS)) {
    const list = document.querySelector(`#live-${key}`);
    const fragment = document.createDocumentFragment();
    const cells = [];
    for (let index = 0; index < length; index += 1) {
      const item = document.createElement("li");
      const label = document.createElement("span");
      label.className = "idx";
      label.textContent = String(index);
      const value = document.createElement("span");
      value.className = "val";
      value.textContent = "—";
      item.append(label, value);
      cells.push(value);
      fragment.append(item);
    }
    list.replaceChildren(fragment);
    liveCells[key] = cells;
  }
}

function setText(selector, text) {
  const element = document.querySelector(selector);
  if (element) element.textContent = text;
}

// Updates text nodes in place: re-rendering the panel during playback would
// destroy focus, so the live tab keeps its DOM for the life of the page.
function updateLive(view) {
  for (const [key, length] of Object.entries(ARRAY_LENGTHS)) {
    const values = view[key];
    const cells = liveCells[key];
    if (!cells || !values) continue;
    for (let index = 0; index < length; index += 1) {
      cells[index].textContent = String(values[index]);
    }
  }
  setText("#coord-twist", String(view.twist));
  setText("#coord-flip", String(view.flip));
  setText("#coord-slice", String(view.slice));
  setText("#coord-cp", String(view.cp_rank));
  // `undefined`, not null, is the wasm-bindgen mapping for None (D018).
  setText("#coord-ep8", view.ep8_rank === undefined ? "—" : String(view.ep8_rank));
  setText("#coord-sp", view.sp_rank === undefined ? "—" : String(view.sp_rank));
  document.querySelector("#coord-note").hidden = view.ep8_rank !== undefined;
}

function buildStructureCards() {
  const container = document.querySelector("#structure-cards");
  let entries;
  try {
    entries = JSON.parse(cube.data_structure_meta());
  } catch (error) {
    container.textContent = "structure metadata unavailable";
    fail(error);
    return;
  }
  const fragment = document.createDocumentFragment();
  for (const entry of entries) {
    const card = document.createElement("article");
    card.className = "structure-card";
    card.dataset.structure = entry.id;

    const head = document.createElement("div");
    head.className = "card-head";
    const title = document.createElement("h3");
    title.textContent = entry.name;
    const kind = document.createElement("span");
    kind.className = "kind";
    kind.textContent = entry.kind;
    head.append(title, kind);
    card.append(head);

    const declaration = document.createElement("pre");
    declaration.className = "declaration";
    declaration.textContent = entry.rust;
    card.append(declaration);

    if (Array.isArray(entry.fields) && entry.fields.length > 0) {
      const scroller = document.createElement("div");
      scroller.className = "table-scroll";
      const table = document.createElement("table");
      table.className = "field-table";
      const header = document.createElement("tr");
      for (const label of ["field", "type", "meaning"]) {
        const cell = document.createElement("th");
        cell.scope = "col";
        cell.textContent = label;
        header.append(cell);
      }
      const thead = document.createElement("thead");
      thead.append(header);
      const tbody = document.createElement("tbody");
      for (const field of entry.fields) {
        const row = document.createElement("tr");
        for (const value of [field.name, field.type, field.meaning]) {
          const cell = document.createElement("td");
          cell.textContent = value;
          row.append(cell);
        }
        tbody.append(row);
      }
      table.append(thead, tbody);
      scroller.append(table);
      card.append(scroller);
    }

    if (Array.isArray(entry.invariants) && entry.invariants.length > 0) {
      const list = document.createElement("ul");
      list.className = "invariants";
      for (const invariant of entry.invariants) {
        const item = document.createElement("li");
        item.textContent = invariant;
        list.append(item);
      }
      card.append(list);
    }

    const meta = document.createElement("p");
    meta.className = "card-meta";
    const sizeText =
      entry.bytes === null
        ? "size: layout-dependent"
        : `size: ${entry.bytes} byte${entry.bytes === 1 ? "" : "s"}`;
    meta.textContent = `${sizeText} · `;
    const link = document.createElement("a");
    link.href = sourceHref(entry.source.file);
    link.target = "_blank";
    link.rel = "noopener";
    link.textContent = `${entry.source.file} → ${entry.source.symbol}`;
    meta.append(link);
    card.append(meta);

    fragment.append(card);
  }
  container.replaceChildren(fragment);
}

function setupTabs() {
  const tablist = document.querySelector('[role="tablist"]');
  const tabs = Array.from(tablist.querySelectorAll('[role="tab"]'));
  const panels = tabs.map((tab) => document.getElementById(tab.getAttribute("aria-controls")));

  function select(target, focus) {
    tabs.forEach((tab, index) => {
      const active = tab === target;
      tab.setAttribute("aria-selected", String(active));
      tab.tabIndex = active ? 0 : -1;
      panels[index].hidden = !active;
    });
    if (panels[tabs.indexOf(target)]?.id === "panel-structures") void ensureSource();
    if (focus) target.focus();
  }

  tabs.forEach((tab) => {
    tab.addEventListener("click", () => select(tab, false));
  });

  tablist.addEventListener("keydown", (event) => {
    const index = tabs.indexOf(document.activeElement);
    if (index === -1) return;
    let next = null;
    if (event.key === "ArrowRight" || event.key === "ArrowDown") {
      next = (index + 1) % tabs.length;
    } else if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
      next = (index - 1 + tabs.length) % tabs.length;
    } else if (event.key === "Home") {
      next = 0;
    } else if (event.key === "End") {
      next = tabs.length - 1;
    }
    if (next === null) return;
    event.preventDefault();
    select(tabs[next], true);
  });
}

function buildHintCell(face, axis) {
  const cell = document.createElement("div");
  cell.className = `turn-hint hint-${face}`;
  cell.dataset.hintCell = "perimeter";
  cell.dataset.hintFace = face;
  cell.dataset.axis = axis;
  cell.dataset.state = "rest";
  cell.dataset.direction = "none";
  cell.setAttribute("role", "img");
  cell.setAttribute("aria-label", `${face} turn hint, at rest`);

  const [first, second] = axis === "horizontal" ? ["◀", "▶"] : ["▲", "▼"];
  for (const [glyph, direction] of [
    [first, axis === "horizontal" ? "left" : "up"],
    [second, axis === "horizontal" ? "right" : "down"],
  ]) {
    const chevron = document.createElement("span");
    chevron.className = "chev";
    chevron.dataset.dir = direction;
    chevron.setAttribute("aria-hidden", "true");
    chevron.textContent = glyph;
    cell.append(chevron);
  }
  return cell;
}

function buildNetHints(fragment) {
  for (const face of PERIMETER_FACES) {
    const cells = [];
    PERIMETER_LAYOUT[face].forEach(([rowStart, colStart, rowEnd, colEnd]) => {
      const cell = buildHintCell(face, face === "U" || face === "D" ? "horizontal" : "vertical");
      cell.style.gridArea = `${rowStart} / ${colStart} / ${rowEnd} / ${colEnd}`;
      cells.push(cell);
      fragment.append(cell);
    });
    hintCells.set(face, cells);
  }
  return fragment;
}

function buildFaceIndicator(face, faceElement) {
  const indicator = buildHintCell(face, face === "F" ? "horizontal" : "vertical");
  indicator.dataset.hintCell = "face";
  indicator.classList.add("face-hint");
  faceElement.append(indicator);
  hintCells.set(face, [indicator]);
}

function buildNet() {
  const fragment = document.createDocumentFragment();
  for (let face = 0; face < 6; face += 1) {
    const faceElement = document.createElement("div");
    faceElement.className = `face face-${FACES[face]}`;
    faceElement.setAttribute("role", "group");
    faceElement.setAttribute("aria-label", `${FACES[face]} face`);
    for (let offset = 0; offset < 9; offset += 1) {
      const sticker = document.createElement("div");
      sticker.className = "sticker";
      const facelet = face * 9 + offset;
      sticker.dataset.facelet = String(facelet);
      sticker.title = `facelet ${facelet}`;
      stickers.push(sticker);
      faceElement.append(sticker);
    }
    if (FACE_INDICATOR_FACES.includes(FACES[face])) {
      buildFaceIndicator(FACES[face], faceElement);
    }
    fragment.append(faceElement);
  }
  buildNetHints(fragment);
  net.replaceChildren(fragment);
}

function applyHint(cell, hint) {
  const chevrons = cell.querySelectorAll(".chev");
  if (!hint) {
    cell.dataset.state = "rest";
    cell.dataset.direction = "none";
    cell.setAttribute("aria-label", `${cell.dataset.hintFace} turn hint, at rest`);
    for (const chevron of chevrons) chevron.classList.remove("lit");
    return;
  }
  cell.dataset.state = hint.state;
  cell.dataset.direction = hint.direction;
  const label =
    hint.state === "half"
      ? `${hint.face} turn, half, both directions`
      : `${hint.face} turn, quarter, ${hint.direction}`;
  cell.setAttribute("aria-label", label);
  for (const chevron of chevrons) {
    const lit = hint.direction === "both" || chevron.dataset.dir === hint.direction;
    chevron.classList.toggle("lit", lit);
  }
}

// The last token in history is the single source of truth for the hint; reset
// empties history, so the hints clear without a separate code path.
function updateHints() {
  const token = state.history.at(-1);
  const hint = token === undefined ? undefined : hintMap[token];
  for (const [face, cells] of hintCells.entries()) {
    const cellHint = hint && hint.face === face ? hint : undefined;
    for (const cell of cells) applyHint(cell, cellHint);
  }
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
    const view = cube.cube_view(state.facelets);
    updateLive(view);

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
  updateHints();
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
  // The canvas is always shown; the net is an additional view, not a mode.
  net.hidden = !show;
  viewToggle.setAttribute("aria-pressed", String(show));
  viewToggle.textContent = show ? "Hide net" : "Show net";
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
    applySourceLinks();
    buildLiveArrays();
    buildStructureCards();
    setupTabs();
    moveNames = Array.from(cube.move_names());
    // Derive each hint once from the real wasm move effect on SOLVED, so the
    // arrows follow the facelet permutation rather than a hand-written table.
    const solved = Uint8Array.from(cube.solved_facelets());
    hintMap = deriveHints(moveNames, (token) =>
      Uint8Array.from(cube.apply_facelets(solved, token)),
    );
    buildMoveButtons();
    wireEvents();
    setNetVisible(true);
    reset();
    installTestHook();
  } catch (error) {
    fail(error);
    setStatus(`failed to load wasm: ${error instanceof Error ? error.message : error}`, "error");
  }
}

main();
