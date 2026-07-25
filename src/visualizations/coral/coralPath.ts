/**
 * Coral / Branch turtle-path engine (§5.5).
 *
 * Pure geometry: given the **already-computed** Parity Sequence (read from the
 * Trajectory Object — never recomputed from `state_sequence`, Principle #3), it
 * produces the turtle path as a list of points. The only mathematics here is the
 * render-boundary trigonometry §4.5 permits for visualization; no trajectory field
 * is computed or corrected.
 *
 * Lives under `src/visualizations/`, so the ESLint boundary rule forbids importing
 * any engine-computation path.
 */

/**
 * Direction rules. The first five are the FROZEN set from §5.5 (`relative` is the
 * default); **`aesthetic` is an addition requested after the frozen spec** and
 * changes nothing about the other five.
 */
export type DirectionRule =
  | 'relative'
  | 'absolute'
  | 'rotate-before'
  | 'rotate-after'
  | 'alternating'
  | 'aesthetic';

/** Human-readable labels for the direction-rule selector. */
export const DIRECTION_RULES: { value: DirectionRule; label: string }[] = [
  { value: 'relative', label: 'Relative (default)' },
  { value: 'absolute', label: 'Absolute' },
  { value: 'rotate-before', label: 'Rotate Before Drawing' },
  { value: 'rotate-after', label: 'Rotate After Drawing' },
  { value: 'alternating', label: 'Alternating' },
  { value: 'aesthetic', label: 'Aesthetic (tree — overlay many)' },
];

/** The geometric Coral parameters (§5.5). Opacity and scale are draw-time. */
export interface CoralParams {
  /** Turn angle (degrees) after an odd-parity transition. */
  oddAngle: number;
  /** Turn angle (degrees) after an even-parity transition. */
  evenAngle: number;
  /** Length of each drawn segment. */
  lineLength: number;
  /** Global rotation offset (degrees) — the turtle's starting heading. */
  rotation: number;
}

export interface Point {
  x: number;
  y: number;
}

const DEG = Math.PI / 180;

/**
 * Computes the turtle path for a Coral drawing.
 *
 * `parity[i]` selects the turn magnitude (odd → `oddAngle`, even → `evenAngle`).
 * The `rule` decides how that turn maps to headings:
 *
 * - **relative** / **rotate-before**: turn is applied to the previous heading
 *   (cumulative), then the segment is drawn — the default coral behaviour.
 * - **absolute**: each segment's heading is `rotation + turn`, measured from the
 *   fixed starting axis, ignoring the prior heading (not cumulative).
 * - **rotate-after**: the segment is drawn along the current heading first, then
 *   the turn is accumulated for the next segment.
 * - **alternating**: the turn's sign alternates each step (+, −, +, …), producing
 *   symmetric branching; magnitude still comes from the parity bit.
 * - **aesthetic**: `relative`, but the parity sequence is walked **in reverse** —
 *   the path is traced from the trajectory's end (the fixed point 1) back to its
 *   start. Give the odd and even angles opposite signs (e.g. +16° / −8°) so the
 *   two parities bend opposite ways; roughly `even ≈ −odd/2` keeps the trunk
 *   straight, since even steps outnumber odd about 2:1.
 *   Because every Collatz trajectory ends `… → 4 → 2 → 1`, reversing means every
 *   trajectory *begins* with that shared tail — so when many are drawn from a
 *   common origin they overlap into a trunk and fan outward, producing the
 *   classic Collatz-tree form. It only looks like a tree with many overlaid
 *   trajectories; a single one is just one branch.
 *
 * Returns `parity.length + 1` points (the start plus one per segment).
 */
export function computeCoralPath(
  parity: number[],
  params: CoralParams,
  rule: DirectionRule,
): Point[] {
  const base = params.rotation * DEG;
  let heading = base;
  let x = 0;
  let y = 0;
  const points: Point[] = [{ x, y }];

  // Aesthetic walks the already-computed parity sequence backwards. This only
  // reorders existing data — no parity is recomputed here (Principle #3).
  const walk = rule === 'aesthetic' ? [...parity].reverse() : parity;

  walk.forEach((bit, i) => {
    const turn = (bit === 1 ? params.oddAngle : params.evenAngle) * DEG;

    // Heading the segment is actually drawn along.
    let drawHeading = heading;
    switch (rule) {
      case 'relative':
      case 'rotate-before':
        heading += turn;
        drawHeading = heading;
        break;
      case 'absolute':
        heading = base + turn;
        drawHeading = heading;
        break;
      case 'rotate-after':
        drawHeading = heading; // draw along current heading...
        heading += turn; // ...then accumulate the turn for next time
        break;
      case 'alternating':
        heading += i % 2 === 0 ? turn : -turn;
        drawHeading = heading;
        break;
      case 'aesthetic':
        // Identical turn logic to `relative` — the sign comes from the angle
        // parameters themselves, so odd and even bend opposite ways when the two
        // angles have opposite signs. (Negating here instead would make both
        // parities turn the same way and curl every path into a circle.)
        // The *only* difference from `relative` is the reversed walk above.
        heading += turn;
        drawHeading = heading;
        break;
    }

    x += Math.cos(drawHeading) * params.lineLength;
    y += Math.sin(drawHeading) * params.lineLength;
    points.push({ x, y });
  });

  return points;
}

/** Axis-aligned bounding box of a path (falls back to a unit box when empty). */
export function pathBounds(points: Point[]): {
  minX: number;
  maxX: number;
  minY: number;
  maxY: number;
} {
  if (points.length === 0) {
    return { minX: 0, maxX: 1, minY: 0, maxY: 1 };
  }
  const xs = points.map((p) => p.x);
  const ys = points.map((p) => p.y);
  return {
    minX: Math.min(...xs),
    maxX: Math.max(...xs),
    minY: Math.min(...ys),
    maxY: Math.max(...ys),
  };
}
