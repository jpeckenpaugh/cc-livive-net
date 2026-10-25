// Net turn hints (workstream C). Pure presentation logic: no cube state and no
// wasm import.
//
// One cell per move. The net is 15 x 12 and the eighteen cells are laid out
// exactly as agreed:
//
//   left of L, one column (single):      U, F, D
//   right of B, inner column (single):   U', F', D'
//   right of B, outer column (double):   U2, F2, D2
//   above U, top row (double):           L2, R2, B2
//   above U, second row (single):        L, R, B
//   below D, one row (single):           L', R', B'
//
// A half turn cell shows two chevrons; everything else shows one. Arrows point
// outward from the net. Every cell is a button that applies its token.

// The arrow shows how the turned layer moves in the net, not where the cell
// sits. U/D are horizontal bands (left/right), L/R are vertical bands
// (down/up), and F/B are face rotations (curved). A half turn shows the
// opposed pair.
const GLYPHS = {
  U: ["\u25C0"], U2: ["\u25C0", "\u25B6"], "U'": ["\u25B6"],
  D: ["\u25B6"], D2: ["\u25C0", "\u25B6"], "D'": ["\u25C0"],
  L: ["\u25BC"], L2: ["\u25BC", "\u25B2"], "L'": ["\u25B2"],
  R: ["\u25B2"], R2: ["\u25B2", "\u25BC"], "R'": ["\u25BC"],
  F: ["\u21BB"], F2: ["\u21BB", "\u21BA"], "F'": ["\u21BA"],
  B: ["\u21BA"], B2: ["\u21BA", "\u21BB"], "B'": ["\u21BB"],
};

function cell(token, area) {
  const glyphs = GLYPHS[token];
  return { token, area, glyphs, power: glyphs.length };
}

export const HINT_LAYOUT = [
  // left of L
  cell("U", [6, 1, 7, 2]),
  cell("F", [7, 1, 8, 2]),
  cell("D", [8, 1, 9, 2]),
  // right of B, inner column
  cell("U'", [6, 14, 7, 15]),
  cell("F'", [7, 14, 8, 15]),
  cell("D'", [8, 14, 9, 15]),
  // right of B, outer column
  cell("U2", [6, 15, 7, 16]),
  cell("F2", [7, 15, 8, 16]),
  cell("D2", [8, 15, 9, 16]),
  // above U, top row
  cell("L2", [1, 5, 2, 6]),
  cell("R2", [1, 6, 2, 7]),
  cell("B2", [1, 7, 2, 8]),
  // above U, second row
  cell("L", [2, 5, 3, 6]),
  cell("R", [2, 6, 3, 7]),
  cell("B", [2, 7, 3, 8]),
  // below D
  cell("L'", [12, 5, 13, 6]),
  cell("R'", [12, 6, 13, 7]),
  cell("B'", [12, 7, 13, 8]),
];

export const HINT_BY_TOKEN = new Map(HINT_LAYOUT.map((hint) => [hint.token, hint]));

export function hintFor(token) {
  return HINT_BY_TOKEN.get(token) ?? null;
}
