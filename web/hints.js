// Net turn hints (workstream C). Pure presentation logic: no cube state and no
// wasm import.
//
// The net is 15 x 12. Eighteen cells sit outside it:
//
//   above U   row 1: three ▲▲ (U2)     row 2: three ▲ (U)
//   below D   row 12: three ▼ (D)
//   left  L   column 1: three ◀ (L)
//   right B   column 14: three ▶ (B)   column 15: three ▶▶ (B2)
//
// A single arrow is a quarter turn, two chevrons are a half turn. Every cell
// is a button that applies its move. L2 and D2 deliberately have no cells.

const GROUPS = {
  U: [1, 2],
  D: [1],
  L: [1],
  B: [1, 2],
};

const POWER_SUFFIX = /['2]/gu;

export function faceOfToken(token) {
  return token.replace(POWER_SUFFIX, "");
}

/// Returns `null` for faces/amplitudes that have no perimeter cells
/// (F, R, L2, D2).
export function deriveHint(token) {
  const face = faceOfToken(token);
  const power = token.endsWith("2") ? 2 : 1;
  if (!GROUPS[face]?.includes(power)) return null;
  return { face, power, state: power === 2 ? "half" : "quarter" };
}

export function deriveHints(moveNames) {
  const hints = {};
  for (const token of moveNames) hints[token] = deriveHint(token);
  return hints;
}
