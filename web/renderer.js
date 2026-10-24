// Isometric canvas renderer.
//
// This module knows nothing about moves or cube state: it receives the 54
// facelets produced by the wasm module and paints them. Camera state (yaw,
// pitch, zoom) lives in this closure; it is view state, not cube state.
//
// Facelet convention (concept.md §3.2, D009): face order U R F D L B, facelet
// index = face * 9 + row * 3 + col, value is the colour index U D F B R L.
//
// Painter's order: the cube is convex, so a face is visible exactly when its
// rotated normal points towards the camera. Hidden faces are culled and the
// remaining faces project to disjoint screen regions, so no per-sticker depth
// sort is needed. The dark face plate is painted before its nine stickers so
// antialiased gaps show the plate, never a hidden sticker.

const FACES = ["U", "R", "F", "D", "L", "B"];

// normal, net-right tangent, net-down tangent (right-handed cube coordinates)
const BASIS = {
  U: { n: [0, 1, 0], r: [1, 0, 0], d: [0, 0, 1] },
  R: { n: [1, 0, 0], r: [0, 0, -1], d: [0, -1, 0] },
  F: { n: [0, 0, 1], r: [1, 0, 0], d: [0, -1, 0] },
  D: { n: [0, -1, 0], r: [1, 0, 0], d: [0, 0, -1] },
  L: { n: [-1, 0, 0], r: [0, 0, 1], d: [0, -1, 0] },
  B: { n: [0, 0, -1], r: [-1, 0, 0], d: [0, -1, 0] },
};

// Flat colours match the CSS palette (--u .. --l and body.cb overrides).
const PALETTE = [
  "#f4f4f5", // U
  "#facc15", // D
  "#22c55e", // F
  "#3b82f6", // B
  "#ef4444", // R
  "#f97316", // L
];
const PALETTE_CB = ["#ffffff", "#ffd166", "#06d6a0", "#118ab2", "#ef476f", "#8f5cf7"];

const PLATE = "#070a0e";
const DEFAULT_VIEW = { yaw: -0.62, pitch: 0.42, zoom: 1 };
const STICKER = 1 / 3;
const GAP = 0.045;

function rotate(point, yaw, pitch) {
  const [x, y, z] = point;
  const cy = Math.cos(yaw);
  const sy = Math.sin(yaw);
  const cp = Math.cos(pitch);
  const sp = Math.sin(pitch);
  // yaw about Y, then pitch about X; +z points towards the camera
  const x1 = x * cy + z * sy;
  const z1 = -x * sy + z * cy;
  const y1 = y * cp - z1 * sp;
  const z2 = y * sp + z1 * cp;
  return [x1, y1, z2];
}

export function createRenderer(canvas) {
  if (!(canvas instanceof HTMLCanvasElement)) {
    throw new Error("renderer needs a <canvas>");
  }
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("2d canvas context unavailable");

  const view = { ...DEFAULT_VIEW };
  let facelets = new Uint8Array(54);
  let palette = PALETTE;
  let dragging = false;
  let lastX = 0;
  let lastY = 0;
  let dpr = 1;

  function resize() {
    const rect = canvas.getBoundingClientRect();
    const width = Math.max(1, Math.round(rect.width));
    const height = Math.max(1, Math.round(rect.height));
    dpr = Math.min(window.devicePixelRatio || 1, 2);
    if (canvas.width !== width * dpr || canvas.height !== height * dpr) {
      canvas.width = width * dpr;
      canvas.height = height * dpr;
    }
    draw();
  }

  function project(point, centre, scale) {
    // Mild perspective keeps the isometric read while adding depth cues.
    const perspective = 7 / (7 - point[2]);
    return {
      x: centre.x + point[0] * scale * perspective,
      y: centre.y - point[1] * scale * perspective,
    };
  }

  function draw() {
    const width = canvas.width;
    const height = canvas.height;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, width, height);
    ctx.scale(dpr, dpr);

    const cssWidth = width / dpr;
    const cssHeight = height / dpr;
    const centre = { x: cssWidth / 2, y: cssHeight / 2 };
    // sqrt(3) ~= half-diagonal of a unit-half cube, plus perspective margin.
    const scale = Math.min(cssWidth, cssHeight) * 0.24 * view.zoom;

    const visible = [];
    for (const face of FACES) {
      const basis = BASIS[face];
      const n = rotate(basis.n, view.yaw, view.pitch);
      if (n[2] <= 0.015) continue;
      visible.push({ face, basis, n });
    }
    visible.sort((a, b) => a.n[2] - b.n[2]);

    for (const { face, basis } of visible) {
      const faceIndex = FACES.indexOf(face);
      const n = basis.n;
      const r = basis.r;
      const d = basis.d;

      // Face plate: the whole 2x2 face, slightly inset.
      const plate = [];
      for (const [u, v] of [
        [-1, -1],
        [1, -1],
        [1, 1],
        [-1, 1],
      ]) {
        const p = rotate(
          [n[0] + r[0] * u + d[0] * v, n[1] + r[1] * u + d[1] * v, n[2] + r[2] * u + d[2] * v],
          view.yaw,
          view.pitch,
        );
        plate.push(project(p, centre, scale));
      }
      ctx.beginPath();
      ctx.moveTo(plate[0].x, plate[0].y);
      for (let i = 1; i < plate.length; i += 1) ctx.lineTo(plate[i].x, plate[i].y);
      ctx.closePath();
      ctx.fillStyle = PLATE;
      ctx.fill();

      for (let row = 0; row < 3; row += 1) {
        for (let col = 0; col < 3; col += 1) {
          const colour = facelets[faceIndex * 9 + row * 3 + col];
          if (colour === undefined || colour > 5) continue;
          const u0 = (col - 1) * (2 / 3);
          const v0 = (row - 1) * (2 / 3);
          const half = STICKER - GAP;
          const quad = [];
          for (const [su, sv] of [
            [-1, -1],
            [1, -1],
            [1, 1],
            [-1, 1],
          ]) {
            const p = rotate(
              [
                n[0] + r[0] * (u0 + su * half) + d[0] * (v0 + sv * half),
                n[1] + r[1] * (u0 + su * half) + d[1] * (v0 + sv * half),
                n[2] + r[2] * (u0 + su * half) + d[2] * (v0 + sv * half),
              ],
              view.yaw,
              view.pitch,
            );
            quad.push(project(p, centre, scale));
          }
          ctx.beginPath();
          ctx.moveTo(quad[0].x, quad[0].y);
          for (let i = 1; i < quad.length; i += 1) ctx.lineTo(quad[i].x, quad[i].y);
          ctx.closePath();
          ctx.fillStyle = palette[colour];
          ctx.fill();
        }
      }
    }
  }

  function resetView() {
    Object.assign(view, DEFAULT_VIEW);
    draw();
  }

  function setFacelets(next) {
    facelets = next;
    draw();
  }

  function setPalette(colourBlind) {
    palette = colourBlind ? PALETTE_CB : PALETTE;
    draw();
  }

  function onPointerDown(event) {
    if (event.button !== 0) return;
    dragging = true;
    lastX = event.clientX;
    lastY = event.clientY;
    canvas.setPointerCapture(event.pointerId);
    canvas.classList.add("dragging");
  }

  function onPointerMove(event) {
    if (!dragging) return;
    const dx = event.clientX - lastX;
    const dy = event.clientY - lastY;
    lastX = event.clientX;
    lastY = event.clientY;
    view.yaw += dx * 0.01;
    view.pitch = Math.max(-1.3, Math.min(1.3, view.pitch + dy * 0.008));
    draw();
  }

  function onPointerUp(event) {
    if (!dragging) return;
    dragging = false;
    canvas.classList.remove("dragging");
    if (canvas.hasPointerCapture(event.pointerId)) {
      canvas.releasePointerCapture(event.pointerId);
    }
  }

  function onWheel(event) {
    event.preventDefault();
    const factor = Math.exp(-event.deltaY * 0.001);
    view.zoom = Math.max(0.6, Math.min(1.8, view.zoom * factor));
    draw();
  }

  const observer = new ResizeObserver(resize);
  observer.observe(canvas);
  canvas.addEventListener("pointerdown", onPointerDown);
  canvas.addEventListener("pointermove", onPointerMove);
  canvas.addEventListener("pointerup", onPointerUp);
  canvas.addEventListener("pointercancel", onPointerUp);
  canvas.addEventListener("wheel", onWheel, { passive: false });
  canvas.addEventListener("dblclick", resetView);

  resize();

  return {
    setFacelets,
    setPalette,
    resetView,
    draw,
    view: () => ({ ...view }),
    destroy() {
      observer.disconnect();
      canvas.removeEventListener("pointerdown", onPointerDown);
      canvas.removeEventListener("pointermove", onPointerMove);
      canvas.removeEventListener("pointerup", onPointerUp);
      canvas.removeEventListener("pointercancel", onPointerUp);
      canvas.removeEventListener("wheel", onWheel);
      canvas.removeEventListener("dblclick", resetView);
    },
  };
}
