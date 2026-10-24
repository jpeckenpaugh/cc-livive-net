# M4b — Isometric canvas, browser test, UI polish

**Read first:** `concept.md` §10, §13 (definition of done for M4);
`docs/DECISIONS.md` (D005, D026–D028); `docs/tasks/M4a-wasm.md`; the current
`web/` tree and `scripts/`.

**Mode:** question-first. Return questions before writing files. No git. Use
`./scripts/docker.sh` / `./scripts/check.sh`.

## Goal

Close M4: the demo shows a real 3D-looking cube on a canvas, and a headless
browser test proves Scramble → Solve → playback → solved through the actual DOM
and wasm, not just the module API.

## 1. Isometric canvas renderer

- Replace the primary visual with an **isometric canvas** (`<canvas>`) showing
  the cube, keeping the unfolded net as a secondary toggle (the net is the
  accessible/text fallback).
- Render all 54 stickers from the current facelet array; no cube state in JS
  beyond `facelets` and a move stack.
- **Drag to rotate** (yaw + pitch, clamped), wheel/`+`/`-` to zoom if cheap.
  Double-click resets the view. A "spin" idle animation is optional; skip it if
  it complicates the test hook.
- Correct painter's order and visible-face culling: hidden stickers must not
  bleed through. Use flat colours from the colour-blind-safe palette when the
  toggle is on; redraw on toggle.
- Move application stays stateless: `apply_facelets(facelets, token)` or the
  existing `facelets_after`. No JS move engine.
- Keep the 18 move buttons, keyboard turns, sequence input, solve/playback,
  move counter, solver timing, observation/action-mask inspector, and
  conformance panel working.

## 2. Browser test (Playwright, Docker-only)

- Add `docker/Dockerfile.browser` (`FROM cc-livive-dev:latest`; install
  `playwright` and Chromium with `--with-deps`). Keep it separate so the base
  dev image and `scripts/check.sh` stay browser-free (D026 stands for local
  checks).
- `scripts/browser-smoke.sh`: build the browser image if missing, build the web
  wasm if missing, serve `web/` from a tiny Node static server (no npm deps
  beyond Playwright), and run `web/tests/browser.spec.mjs`.
- The spec must:
  1. load the page, wait for the wasm-ready status;
  2. assert the canvas is present and non-blank (sample a few pixels);
  3. click **Scramble**, assert the status/move counter shows 25 moves and the
     cube is not solved;
  4. click **Solve**, assert a solution length > 0;
  5. click **Play solution** and wait for the solved status;
  6. toggle the colour-blind palette and assert it does not throw;
  7. exercise the keyboard turn path once.
- Expose a minimal read-only test hook, e.g. `window.__cc = { facelets:
  () => [...], solved: () => boolean, lastError: () => string|null }`, guarded
  so it is harmless in production. Prefer this over scraping text.
- Script exits non-zero with a readable message on any failure; prints a
  one-line PASS summary.

## 3. CI

- Add a `browser` job (or a step in `wasm`) that builds the browser image, uses
  the same table cache, runs `scripts/browser-smoke.sh`, and uploads a
  screenshot artifact on failure.
- Keep `rust` and `wasm` independent.

## 4. UI polish

- Empty/loading/error states are visible and readable; no unhandled promise
  rejections in the console (the browser test should fail on `console.error`).
- Canvas has an `aria-label` and the net toggle is keyboard reachable.
- Respect `prefers-reduced-motion` for playback speed.
- Mobile: controls usable at 375px width; canvas scales with its container.

## Tests / acceptance

- `scripts/browser-smoke.sh` passes locally in Docker and in CI.
- `scripts/check.sh` still passes and remains browser-free.
- No second cube implementation in JS (grep for facelet rotation logic in
  `web/` should find none; all moves go through wasm).
- Node smoke still passes.

## Reporting

Files, commands, browser-test output, screenshot path, canvas render notes
(painter order), and anything unresolved. Under 800 words.
