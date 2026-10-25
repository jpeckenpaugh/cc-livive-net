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

const GLYPH = { left: "\u25C0", right: "\u25B6", up: "\u25B2", down: "\u25BC" };

function cell(token, dir, area) {
  return {
    token,
    area,
    glyph: GLYPH[dir],
    power: token.endsWith("2") ? 2 : 1,
  };
}

export const HINT_LAYOUT = [
  // left of L
  cell("U", "left", [6, 1, 7, 2]),
  cell("F", "left", [7, 1, 8, 2]),
  cell("D", "left", [8, 1, 9, 2]),
  // right of B, inner column
  cell("U'", "right", [6, 14, 7, 15]),
  cell("F'", "right", [7, 14, 8, 15]),
  cell("D'", "right", [8, 14, 9, 15]),
  // right of B, outer column
  cell("U2", "right", [6, 15, 7, 16]),
  cell("F2", "right", [7, 15, 8, 16]),
  cell("D2", "right", [8, 15, 9, 16]),
  // above U, top row
  cell("L2", "up", [1, 5, 2, 6]),
  cell("R2", "up", [1, 6, 2, 7]),
  cell("B2", "up", [1, 7, 2, 8]),
  // above U, second row
  cell("L", "up", [2, 5, 3, 6]),
  cell("R", "up", [2, 6, 3, 7]),
  cell("B", "up", [2, 7, 3, 8]),
  // below D
  cell("L'", "down", [12, 5, 13, 6]),
  cell("R'", "down", [12, 6, 13, 7]),
  cell("B'", "down", [12, 7, 13, 8]),
];

export const HINT_BY_TOKEN = new Map(HINT_LAYOUT.map((hint) => [hint.token, hint]));

export function hintFor(token) {
  return HINT_BY_TOKEN.get(token) ?? null;
}
