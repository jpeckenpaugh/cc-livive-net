// Playwright browser gate for M4b (Docker only).
//
//   node web/tests/browser.spec.mjs http://127.0.0.1:PORT
//
// Exercises the real DOM + wasm path: load, canvas paint, scramble, solve,
// playback, palette toggle, keyboard turn. Fails on console errors and writes
// artifacts/browser-failure.png when anything goes wrong.

import { createRequire } from "node:module";
import { mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import { deriveHint, faceOfToken } from "../hints.js";

const require = createRequire(process.env.PLAYWRIGHT_PACKAGE ?? "/opt/browser/package.json");
const { chromium } = require("playwright");

const baseUrl = process.argv[2];
if (!baseUrl) {
  console.error("usage: browser.spec.mjs <base-url>");
  process.exit(2);
}

const failures = [];
const consoleErrors = [];
let browser;

function check(condition, message) {
  if (!condition) failures.push(message);
}

async function attempt(name, fn) {
  const before = failures.length;
  try {
    await fn();
  } catch (error) {
    failures.push(`${name}: ${error instanceof Error ? error.message : error}`);
  }
  if (failures.length === before) {
    console.log(`ok   ${name}`);
  } else {
    console.log(`FAIL ${name}`);
  }
}

async function sampleCanvas(page) {
  return page.evaluate(() => {
    const canvas = document.querySelector("#cube");
    if (!(canvas instanceof HTMLCanvasElement)) return null;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    if (!ctx) return null;
    const { data, width, height } = ctx.getImageData(0, 0, canvas.width, canvas.height);
    let opaque = 0;
    let total = 0;
    const colours = new Set();
    const step = Math.max(2, Math.floor(width / 96));
    for (let y = 0; y < height; y += step) {
      for (let x = 0; x < width; x += step) {
        const index = (y * width + x) * 4;
        total += 1;
        if (data[index + 3] > 200) {
          opaque += 1;
          colours.add(`${data[index] >> 4},${data[index + 1] >> 4},${data[index + 2] >> 4}`);
        }
      }
    }
    return {
      width,
      height,
      opaque,
      total,
      colours: [...colours],
      label: canvas.getAttribute("aria-label"),
    };
  });
}

async function readHints(page) {
  return page.evaluate(() =>
    [...document.querySelectorAll("[data-hint-cell]")].map((cell) => ({
      cell: cell.dataset.hintCell,
      face: cell.dataset.hintFace,
      power: Number(cell.dataset.power),
      state: cell.dataset.state,
      lit: cell.querySelectorAll(".chev.lit").length,
      glyphs: [...cell.querySelectorAll(".chev")].map((chevron) => chevron.textContent),
      label: cell.getAttribute("aria-label"),
      classes: [...cell.classList],
    })),
  );
}

async function applyHintsMove(page, token) {
  await page.locator("#reset").click();
  await page.locator("#sequence").fill(token);
  await page.locator("#sequence").press("Enter");
  await page.waitForFunction(() => window.__cc.moveCount() === 1);
}

async function run() {
  browser = await chromium.launch({ args: ["--no-sandbox"] });
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  const page = await context.newPage();
  page.setDefaultTimeout(60_000);

  page.on("console", (message) => {
    if (message.type() === "error") consoleErrors.push(message.text());
  });
  page.on("pageerror", (error) => consoleErrors.push(`pageerror: ${error.message}`));

  await attempt("page loads and wasm initialises", async () => {
    await page.goto(baseUrl, { waitUntil: "domcontentloaded" });
    await page.waitForFunction(() => typeof window.__cc === "object", null, { timeout: 60_000 });
    check((await page.title()).includes("cc.livive.net"), "unexpected page title");
  });

  await attempt("Source link points at the repository", async () => {
    const href = await page.locator("#source-link").getAttribute("href");
    check(
      href === "https://github.com/jpeckenpaugh/cc-livive-net",
      `unexpected source href: ${href}`,
    );
    check(
      (await page.locator("#source-link").getAttribute("rel"))?.includes("noopener"),
      "source link missing rel=noopener",
    );
    check(
      (await page.locator("#source-link").getAttribute("target")) === "_blank",
      "source link does not open a new tab",
    );
  });

  await attempt("Rust types tab shows Cube and a pinned source link", async () => {
    await page.locator("#tab-types").click();
    check(await page.locator("#panel-types").isVisible(), "types panel not visible");
    check(await page.locator("#panel-live").isHidden(), "live panel still visible");
    const card = page.locator('[data-structure="cube"]');
    check(await card.isVisible(), "Cube card missing");
    check((await card.textContent()).includes("pub struct Cube"), "Cube declaration missing");
    const href = await card.locator("a").getAttribute("href");
    check(
      /^https:\/\/github\.com\/.+\/blob\/(main|[0-9a-f]{40})\/crates\/color-cube\/src\/lib\.rs$/.test(
        href,
      ),
      `unexpected card link: ${href}`,
    );
  });

  await attempt("tab keyboard navigation follows the WAI-ARIA pattern", async () => {
    await page.locator("#tab-live").click();
    await page.locator("#tab-live").focus();
    await page.keyboard.press("ArrowRight");
    check(
      (await page.locator("#tab-types").getAttribute("aria-selected")) === "true",
      "ArrowRight did not select Rust types",
    );
    check(await page.locator("#panel-types").isVisible(), "ArrowRight did not show types panel");
    await page.keyboard.press("Home");
    check(
      (await page.locator("#tab-live").getAttribute("aria-selected")) === "true",
      "Home did not select Live state",
    );
    check(await page.locator("#panel-live").isVisible(), "Home did not show the live panel");
  });

  await attempt("applying a move updates the live cp row", async () => {
    await page.locator("#reset").click();
    const before = await page.locator("#live-cp").textContent();
    await page.locator("#moves button", { hasText: /^R$/ }).click();
    const after = await page.locator("#live-cp").textContent();
    check(before !== after, "cp row did not change after R");
    check(after.includes("—") === false, "cp row still has placeholder text");
  });

  await attempt("canvas is present, 3D, and non-blank", async () => {
    check((await page.locator("#cube").count()) === 1, "canvas missing");
    // Reset the camera so sampling is deterministic.
    await page.locator("#cube").dblclick();
    await page.waitForTimeout(100);
    const stats = await sampleCanvas(page);
    check(stats !== null, "canvas context missing");
    if (!stats) return;
    check(stats.width > 0 && stats.height > 0, "canvas has no backing size");
    check(stats.opaque / stats.total > 0.1, `canvas looks blank (${stats.opaque}/${stats.total})`);
    check(stats.colours.length >= 3, `only ${stats.colours.length} distinct colours`);
    check(Boolean(stats.label && stats.label.length > 10), "canvas lacks an aria-label");
  });

  await attempt("drag rotates the view", async () => {
    const before = await page.evaluate(() => window.__cc.view());
    const box = await page.locator("#cube").boundingBox();
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.move(box.x + box.width / 2 + 60, box.y + box.height / 2 + 20, { steps: 5 });
    await page.mouse.up();
    const after = await page.evaluate(() => window.__cc.view());
    check(Math.abs(after.yaw - before.yaw) > 0.1, "yaw did not change on drag");
    await page.locator("#cube").dblclick();
  });

  await attempt("scramble produces 25 moves and a solved=false cube", async () => {
    await page.locator("#scramble").click();
    await page.waitForFunction(() => window.__cc.moveCount() === 25);
    check((await page.evaluate(() => window.__cc.solved())) === false, "cube still reports solved");
  });

  await attempt("solve returns a non-empty solution", async () => {
    await page.locator("#solve").click();
    await page.waitForFunction(
      () => {
        const value = document.querySelector("#solution-length")?.textContent ?? "";
        return value !== "—" && Number(value) > 0;
      },
      null,
      { timeout: 60_000 },
    );
    const length = Number(await page.locator("#solution-length").textContent());
    check(length > 0 && length <= 45, `solution length out of range: ${length}`);
  });

  await attempt("playback drives the cube to solved", async () => {
    await page.locator("#play-speed").selectOption("8");
    await page.waitForFunction(() => window.__cc.solved() && !window.__cc.playing(), null, {
      timeout: 60_000,
    });
    check((await page.locator("#status").textContent())?.includes("solved"), "status not solved");
  });

  await attempt("colour-blind palette toggles and repaints", async () => {
    const before = await sampleCanvas(page);
    await page.locator("#cb-toggle").check();
    await page.waitForTimeout(150);
    const after = await sampleCanvas(page);
    check(after && after.opaque / after.total > 0.1, "canvas blank after palette toggle");
    check(
      JSON.stringify(before?.colours) !== JSON.stringify(after?.colours),
      "palette colours did not change",
    );
    check((await page.evaluate(() => window.__cc.lastError())) === null, "app recorded an error");
  });

  await attempt("cube and net are shown together; the toggle only hides the net", async () => {
    check(await page.locator("#cube").isVisible(), "canvas is not visible by default");
    check(await page.locator("#net").isVisible(), "net is not visible by default");
    await page.locator("#view-toggle").click();
    check(await page.locator("#net").isHidden(), "net still visible after hiding");
    check(await page.locator("#cube").isVisible(), "canvas disappeared with the net");
    await page.locator("#view-toggle").click();
    check(await page.locator("#net").isVisible(), "net did not come back");
    check(await page.locator("#cube").isVisible(), "canvas disappeared with the net");
  });

  await attempt("hint cells match the 15x12 layout and amplitudes", async () => {
    await page.locator("#reset").click();
    const hints = await readHints(page);
    check(hints.length === 18, `expected 18 hint cells, got ${hints.length}`);
    const count = (face, power) =>
      hints.filter((hint) => hint.face === face && hint.power === power).length;
    check(count("U", 2) === 3 && count("U", 1) === 3, "U needs 3 double and 3 single cells");
    check(count("D", 1) === 3 && count("D", 2) === 0, "D needs 3 single cells only");
    check(count("L", 1) === 3 && count("L", 2) === 0, "L needs 3 single cells only");
    check(count("B", 1) === 3 && count("B", 2) === 3, "B needs 3 single and 3 double cells");
    check(
      hints.every((hint) => hint.state === "rest" && hint.lit === 0),
      "hints are not dim at rest",
    );
    check(
      hints.every((hint) => hint.glyphs.length === hint.power),
      "a cell must hold one chevron per power",
    );
    check(
      hints.every((hint) => new Set(hint.glyphs).size === 1),
      "chevrons in a cell must share one outward direction",
    );
    const expectedGlyph = { U: "\u25B2", D: "\u25BC", L: "\u25C0", B: "\u25B6" };
    for (const [face, glyph] of Object.entries(expectedGlyph)) {
      const cells = hints.filter((hint) => hint.face === face);
      check(cells.every((hint) => hint.glyphs.every((g) => g === glyph)), `${face} arrows do not point outward`);
    }
  });

  await attempt("hint cells apply quarter and double turns", async () => {
    await page.locator("#reset").click();
    const before = await page.evaluate(() => ({
      count: window.__cc.moveCount(),
      facelets: window.__cc.facelets(),
    }));

    await page.locator('.hint-U[data-power="1"]').first().click();
    await page.waitForFunction((count) => window.__cc.moveCount() === count + 1, before.count);
    const afterQuarter = await page.evaluate(() => window.__cc.facelets());
    check(
      JSON.stringify(afterQuarter) !== JSON.stringify(before.facelets),
      "clicking a single-arrow cell did not turn the cube",
    );
    let hints = await readHints(page);
    check(
      hints.filter((hint) => hint.face === "U" && hint.power === 1).every((hint) => hint.state === "active"),
      "U did not light the three single cells",
    );
    check(
      hints.filter((hint) => !(hint.face === "U" && hint.power === 1)).every((hint) => hint.state === "rest"),
      "U lit cells outside the single row",
    );

    await page.locator('.hint-U[data-power="2"]').first().click();
    await page.waitForFunction((count) => window.__cc.moveCount() === count + 2, before.count);
    hints = await readHints(page);
    check(
      hints.filter((hint) => hint.face === "U" && hint.power === 2).every((hint) => hint.state === "active"),
      "U2 did not light the three double cells",
    );
    check(
      hints.filter((hint) => hint.face === "U" && hint.power === 1).every((hint) => hint.state === "rest"),
      "U2 left the single row lit",
    );

    await page.locator('.hint-B[data-power="2"]').first().click();
    hints = await readHints(page);
    check(
      hints.filter((hint) => hint.face === "B" && hint.power === 2).every((hint) => hint.state === "active"),
      "B2 did not light the outer column",
    );
    check(
      hints.filter((hint) => hint.face === "B" && hint.power === 1).every((hint) => hint.state === "rest"),
      "B2 left the inner column lit",
    );

    await page.locator('.hint-L[data-power="1"]').first().click();
    hints = await readHints(page);
    check(
      hints.filter((hint) => hint.face === "L").every((hint) => hint.state === "active"),
      "L did not light its three cells",
    );

    await page.locator("#reset").click();
    hints = await readHints(page);
    check(
      hints.every((hint) => hint.state === "rest" && hint.lit === 0),
      "reset did not clear the hints",
    );
  });

  await attempt("all 18 moves match the derived hint mapping", async () => {
    const moveNames = [
      "U", "U2", "U'", "R", "R2", "R'", "F", "F2", "F'",
      "D", "D2", "D'", "L", "L2", "L'", "B", "B2", "B'",
    ];
    for (const token of moveNames) {
      await applyHintsMove(page, token);
      const expected = deriveHint(token);
      const hints = await readHints(page);
      if (expected === null) {
        check(hints.every((hint) => hint.state === "rest" && hint.lit === 0), `${token} must not light a hint`);
        continue;
      }
      const active = hints.filter(
        (hint) => hint.face === expected.face && hint.power === expected.power,
      );
      check(active.length === 3, `${token}: expected three active cells`);
      check(
        active.every((hint) => hint.state === "active" && hint.lit === expected.power),
        `${token}: cells are not active with ${expected.power} lit chevron(s)`,
      );
      check(
        hints
          .filter((hint) => !(hint.face === expected.face && hint.power === expected.power))
          .every((hint) => hint.state === "rest"),
        `${token}: lit cells outside the expected row/column`,
      );
    }
  });

  const dir = resolve("artifacts");
  await mkdir(dir, { recursive: true });
  await page.locator("#reset").click();
  await applyHintsMove(page, "L'");
  await page.screenshot({ path: resolve(dir, "net-hints.png"), fullPage: true });
  await page.screenshot({ path: resolve(dir, "browser-pass.png"), fullPage: true });

  await attempt("keyboard turn applies exactly one move", async () => {
    const before = await page.evaluate(() => ({
      count: window.__cc.moveCount(),
      facelets: window.__cc.facelets(),
    }));
    await page.locator("#cube").focus();
    await page.keyboard.press("r");
    await page.waitForFunction((count) => window.__cc.moveCount() === count + 1, before.count);
    const after = await page.evaluate(() => window.__cc.facelets());
    check(JSON.stringify(before.facelets) !== JSON.stringify(after), "facelets did not change");
    check((await page.evaluate(() => window.__cc.lastError())) === null, "app recorded an error");
  });

  await attempt("375px layout does not overflow horizontally", async () => {
    await page.setViewportSize({ width: 375, height: 800 });
    await page.reload({ waitUntil: "domcontentloaded" });
    await page.waitForFunction(() => typeof window.__cc === "object", null, { timeout: 60_000 });
    const width = await page.evaluate(() => window.innerWidth);
    const scroll = await page.evaluate(() => document.documentElement.scrollWidth);
    check(scroll <= width + 1, `live tab overflows: ${scroll} > ${width}`);
    await page.locator("#tab-types").click();
    const typesScroll = await page.evaluate(() => document.documentElement.scrollWidth);
    check(typesScroll <= width + 1, `types tab overflows: ${typesScroll} > ${width}`);
    await page.screenshot({ path: resolve("artifacts", "browser-mobile.png"), fullPage: true });
  });

  await attempt("no console errors or unhandled rejections", async () => {
    check(consoleErrors.length === 0, `console errors: ${consoleErrors.join(" | ")}`);
  });
}

try {
  await run();
} catch (error) {
  failures.push(`fatal: ${error instanceof Error ? error.stack ?? error.message : error}`);
} finally {
  if (failures.length > 0) {
    const dir = resolve("artifacts");
    await mkdir(dir, { recursive: true });
    try {
      if (browser) {
        const pages = browser.contexts().flatMap((context) => context.pages());
        if (pages.length > 0) {
          await pages[0].screenshot({ path: resolve(dir, "browser-failure.png"), fullPage: true });
        }
      }
    } catch (error) {
      console.error(`screenshot failed: ${error instanceof Error ? error.message : error}`);
    }
    if (browser) await browser.close();
    console.error(`\nFAIL: ${failures.length} failure(s)`);
    for (const failure of failures) console.error(` - ${failure}`);
    process.exit(1);
  }
  if (browser) await browser.close();
  console.log("PASS: load, canvas, drag, scramble, solve, playback, palette, keyboard");
}
