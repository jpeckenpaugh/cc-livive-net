# C — Net turn hints (12 perimeter cells + in-face F/R)

**Read first:** `concept.md` §10; `docs/DECISIONS.md`; `web/index.html`,
`web/app.js`, `web/style.css`, `web/tests/browser.spec.mjs`.

**Mode:** implement directly. The PM has resolved the open questions below;
do not re-litigate them. Ask only if something is actually blocked.

## Resolved design

- Net grows from **12 × 9** to **18 × 15**; the four corner blocks stay empty.
- New cells:
  - 3 left of `L` (cols 1–3, rows 7–9)
  - 3 above `U` (cols 7–9, rows 1–3)
  - 3 below `D` (cols 7–9, rows 13–15)
  - 3 right of `B` (cols 16–18, rows 7–9)
- These 12 cells indicate `U`, `D`, `L`, `B` turns.
- `F` and `R` are interior; add small in-face rotation indicators on the `F`
  and `R` tiles (same glyph language), not extra cells.
- Each perimeter cell holds **two opposed chevrons** along the tangent
  (`◀ ▶` for top/bottom, `▲ ▼` for left/right), dim grey at rest.
  - quarter turn: exactly **one** chevron white, pointing the way the band moves
  - half turn: **both** chevrons white
- Direction must be **derived** from the move's facelet effect on the net
  layout, not guessed. Add a test that checks all 18 moves against the
  derived mapping.
- Hints persist until the next move; cleared on reset. They update on single
  moves, keyboard turns, sequence apply, scramble, and every playback step.
- Transition ~120 ms; instant under `prefers-reduced-motion`.
- Accessibility: each cell has an `aria-label` (e.g. "U turn, quarter, left");
  arrow direction + count carry the meaning, never colour alone.

## Implementation notes

- Keep the face blocks at their current proportions inside the larger grid.
  Update `.face-U/L/F/R/B/D` grid areas and the net `grid-template`/aspect.
- Do not obscure face colours. Hint cells are outside faces; F/R indicators are
  small overlays in a face corner.
- `app.js` already tracks the last move for the canonical mask; reuse that
  state path rather than adding a second source of truth.
- No wasm changes. No new dependencies. All hint logic is presentation.
- Scale with the grid: cells roughly square, readable at 375 px without
  horizontal page scroll.

## Direction expectations (verify against the real move tables)

The following are what the net layout implies; confirm or correct them by
applying each move to `Cube::SOLVED` and inspecting facelet movement:

- `U` moves the middle band right-to-left in the net → `◀`; `U'` → `▶`; `U2` both.
- `D` moves it left-to-right → `▶`; `D'` → `◀`.
- `L` moves the left column downward → `▼`; `L'` → `▲`.
- `B` direction follows the same derivation; state it in a comment.

If a derivation disagrees with this list, trust the derivation and say so in
the report — do not force the guess.

## Tests

- Browser spec:
  - 12 hint cells exist with the expected edge classes;
  - at rest all chevrons are dim, none active;
  - `U` lights exactly one chevron in the top group; `U2` lights two;
  - `L'` lights one chevron in the left group pointing the other way;
  - `F` and `R` light their in-face indicators; `B` lights the right group;
  - reset clears all hints;
  - the net still fits at 375 px with no page overflow.
- Keep the existing 15 browser assertions green (update the net-toggle one only
  if its selectors change).
- Node smoke unchanged unless the DOM probe is easy to extend.

## Reporting

Files, the final move→group/direction table, browser-test output, screenshot
path, and anything you had to correct in the direction expectations. Under 700
words.
