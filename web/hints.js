// Net turn hints (workstream C). Pure presentation logic: no cube state and no
// wasm import.
//
// Twelve cells sit outside the net, three per edge. Both chevrons in a cell
// point the same way, away from the centre of the net:
//
//   above U  ▲ ▲      below D  ▼ ▼
//   left  L  ◀ ◀      right B  ▶ ▶
//
// A quarter turn lights one chevron; a half turn lights both. The group tells
// you which row/column is turning and the count tells you the amplitude.
// F and R are interior faces and deliberately have no hint cells.

export const PERIMETER_FACES = ["U", "D", "L", "B"];

const GROUPS = {
  U: { glyph: "▲", axis: "row" },
  D: { glyph: "▼", axis: "row" },
  L: { glyph: "◀", axis: "column" },
  B: { glyph: "▶", axis: "column" },
};

const POWER_SUFFIX = /['2]/gu;

export function faceOfToken(token) {
  return token.replace(POWER_SUFFIX, "");
}

/// Returns `null` for faces that have no perimeter hint (F and R).
export function deriveHint(token) {
  const face = faceOfToken(token);
  const group = GROUPS[face];
  if (group === undefined) return null;
  return {
    face,
    glyph: group.glyph,
    axis: group.axis,
    state: token.endsWith("2") ? "half" : "quarter",
  };
}

export function deriveHints(moveNames) {
  const hints = {};
  for (const token of moveNames) hints[token] = deriveHint(token);
  return hints;
}
