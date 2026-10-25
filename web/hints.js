// Net turn hints (workstream C). Pure presentation logic: no cube state and no
// wasm import, so the browser spec can reuse it to derive the expected hints
// from the real facelet effect of a move.
//
// The net layout is the unfolded cross from concept.md §3.2:
//
//   . . . . U U U . . . . . . .
//   . . . . U U U . . . . . . .
//   . . . . U U U . . . . . . .
//   . L L L F F F R R R B B B .
//   . L L L F F F R R R B B B .
//   . L L L F F F R R R B B B .
//   . . . . D D D . . . . . . .
//   . . . . D D D . . . . . . .
//   . . . . D D D . . . . . . .
//
// Row/column numbers below are 1-based grid coordinates. A facelet is
// face * 9 + row * 3 + col in U R F D L B face order (D009).

export const FACE_ORDER = ["U", "R", "F", "D", "L", "B"];

// Colour index order is U D F B R L (COLOR_LETTERS in app.js). On the solved
// cube every sticker carries its source face's colour, so after one move the
// colour at a destination facelet identifies the face the sticker came from.
const FACE_BY_COLOUR = ["U", "D", "F", "B", "R", "L"];

const NET_ORIGIN = {
  U: { row: 2, col: 5 },
  R: { row: 5, col: 8 },
  F: { row: 5, col: 5 },
  D: { row: 8, col: 5 },
  L: { row: 5, col: 2 },
  B: { row: 5, col: 11 },
};

const NET_CENTRE = {
  U: { row: 3, col: 6 },
  R: { row: 6, col: 9 },
  F: { row: 6, col: 6 },
  D: { row: 9, col: 6 },
  L: { row: 6, col: 3 },
  B: { row: 6, col: 12 },
};

// Each hint probes the three destination facelets that the turn carries into
// the band next to the hint. The resulting source face tells which way the
// material travels across the net. `axis` is the chevron axis, not the axis of
// the face's own printed rotation.
//
// Probes, with the clockwise direction they derive:
//   U: F top row       <- R   (source to the right)  => left
//   D: F bottom row    <- L   (source to the left)   => right
//   L: F left column   <- U   (source above)         => down
//   B: R right column  <- D   (source below)         => up
//   F: U bottom row    <- L   (source to the left)   => right
//   R: U right column  <- F   (source below)         => up
export const HINT_GROUPS = {
  U: {
    face: "U",
    axis: "horizontal",
    probe: { face: "F", rows: [0, 0, 0], cols: [0, 1, 2] },
  },
  D: {
    face: "D",
    axis: "horizontal",
    probe: { face: "F", rows: [2, 2, 2], cols: [0, 1, 2] },
  },
  L: {
    face: "L",
    axis: "vertical",
    probe: { face: "F", rows: [0, 1, 2], cols: [0, 0, 0] },
  },
  B: {
    face: "B",
    axis: "vertical",
    probe: { face: "R", rows: [0, 1, 2], cols: [2, 2, 2] },
  },
  F: {
    face: "F",
    axis: "horizontal",
    probe: { face: "U", rows: [2, 2, 2], cols: [0, 1, 2] },
  },
  R: {
    face: "R",
    axis: "vertical",
    probe: { face: "U", rows: [0, 1, 2], cols: [2, 2, 2] },
  },
};

export const PERIMETER_FACES = ["U", "D", "L", "B"];
export const FACE_INDICATOR_FACES = ["F", "R"];

const OPPOSITE = { left: "right", right: "left", up: "down", down: "up" };

export function faceOfToken(token) {
  const face = token.replace(/['2]/gu, "");
  if (!(face in HINT_GROUPS)) {
    throw new Error(`unknown move token: ${token}`);
  }
  return face;
}

function netCell(face, row, col) {
  const origin = NET_ORIGIN[face];
  return { row: origin.row + row, col: origin.col + col };
}

/// Derives the hint for `token` from the facelets after the plain clockwise
/// quarter turn of that face. The quarter turn is the only move that encodes
/// the rotation direction; `2` and `'` are derived from it by count/inversion.
export function deriveHint(token, quarterAfter) {
  const face = faceOfToken(token);
  const group = HINT_GROUPS[face];
  const probeFace = group.probe.face;
  const probeIndex = FACE_ORDER.indexOf(probeFace);
  const directions = new Set();

  for (let i = 0; i < 3; i += 1) {
    const row = group.probe.rows[i];
    const col = group.probe.cols[i];
    const destination = probeIndex * 9 + row * 3 + col;
    const sourceFace = FACE_BY_COLOUR[quarterAfter[destination]];
    if (sourceFace === undefined) {
      throw new Error(`facelet ${destination} has no source face`);
    }
    const source = NET_CENTRE[sourceFace];
    const destinationCell = netCell(probeFace, row, col);
    if (group.axis === "horizontal") {
      directions.add(source.col > destinationCell.col ? "left" : "right");
    } else {
      directions.add(source.row > destinationCell.row ? "up" : "down");
    }
  }

  if (directions.size !== 1) {
    throw new Error(`inconsistent hint derivation for ${token}: ${[...directions].join(", ")}`);
  }

  const clockwise = [...directions][0];
  const half = token.endsWith("2");
  const prime = token.endsWith("'");
  return {
    face,
    axis: group.axis,
    state: half ? "half" : "quarter",
    direction: half ? "both" : prime ? OPPOSITE[clockwise] : clockwise,
  };
}

/// Derives hints for every move name. `apply(token)` must return the facelets
/// after applying `token` to a solved cube; only the quarter turn of each face
/// is used, so `2`/`'` inherit its direction.
export function deriveHints(moveNames, apply) {
  const hints = {};
  for (const token of moveNames) {
    const face = faceOfToken(token);
    hints[token] = deriveHint(token, apply(face));
  }
  return hints;
}
