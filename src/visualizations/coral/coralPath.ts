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

/** The 5 FROZEN direction rules (§5.5); `relative` is the default. */
export type DirectionRule =
  | 'relative'
  | 'absolute'
  | 'rotate-before'
  | 'rotate-after'
  | 'alternating';

/** Human-readable labels for the direction-rule selector. */
export const DIRECTION_RULES: { value: DirectionRule; label: string }[] = [
  { value: 'relative', label: 'Relative (default)' },
  { value: 'absolute', label: 'Absolute' },
  { value: 'rotate-before', label: 'Rotate Before Drawing' },
  { value: 'rotate-after', label: 'Rotate After Drawing' },
  { value: 'alternating', label: 'Alternating' },
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

  parity.forEach((bit, i) => {
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
