import { describe, expect, it } from 'vitest';
import { computeCoralPath, type CoralParams } from './coralPath';

// The Appendix B (n = 3) parity sequence, taken as-is from the Trajectory Object.
const APPENDIX_B_PARITY = [1, 0, 1, 0, 0, 0, 0];

// Right-angle turns, unit segments, heading starting along +x, so the geometry is
// hand-checkable. (Canvas y-flip happens later, at draw time; here +y is "up".)
const RIGHT_ANGLE: CoralParams = { oddAngle: 90, evenAngle: 0, lineLength: 1, rotation: 0 };

function endpoint(points: { x: number; y: number }[]): { x: number; y: number } {
  return points[points.length - 1] ?? { x: NaN, y: NaN };
}

describe('computeCoralPath (Appendix B parity)', () => {
  it('produces one point per transition plus the start', () => {
    for (const rule of [
      'relative',
      'absolute',
      'rotate-before',
      'rotate-after',
      'alternating',
      'aesthetic',
    ] as const) {
      const path = computeCoralPath(APPENDIX_B_PARITY, RIGHT_ANGLE, rule);
      expect(path.length).toBe(APPENDIX_B_PARITY.length + 1); // 8
      expect(path[0]).toEqual({ x: 0, y: 0 });
    }
  });

  it('relative: cumulative right turns trace the hand-computed path to (-5, 2)', () => {
    const path = computeCoralPath(APPENDIX_B_PARITY, RIGHT_ANGLE, 'relative');
    const end = endpoint(path);
    expect(end.x).toBeCloseTo(-5, 10);
    expect(end.y).toBeCloseTo(2, 10);
  });

  it('absolute: non-cumulative angles trace to (5, 2)', () => {
    const end = endpoint(computeCoralPath(APPENDIX_B_PARITY, RIGHT_ANGLE, 'absolute'));
    expect(end.x).toBeCloseTo(5, 10);
    expect(end.y).toBeCloseTo(2, 10);
  });

  it('rotate-after: draw-then-turn traces to (-3, 2)', () => {
    const end = endpoint(computeCoralPath(APPENDIX_B_PARITY, RIGHT_ANGLE, 'rotate-after'));
    expect(end.x).toBeCloseTo(-3, 10);
    expect(end.y).toBeCloseTo(2, 10);
  });

  it('rotate-before matches relative for this system', () => {
    const rel = endpoint(computeCoralPath(APPENDIX_B_PARITY, RIGHT_ANGLE, 'relative'));
    const before = endpoint(computeCoralPath(APPENDIX_B_PARITY, RIGHT_ANGLE, 'rotate-before'));
    expect(before).toEqual(rel);
  });

  it('alternating differs from relative when turns alternate sign', () => {
    const params: CoralParams = { oddAngle: 90, evenAngle: 90, lineLength: 1, rotation: 0 };
    const rel = endpoint(computeCoralPath(APPENDIX_B_PARITY, params, 'relative'));
    const alt = endpoint(computeCoralPath(APPENDIX_B_PARITY, params, 'alternating'));
    // relative closes toward a loop near (-1, 0); alternating fans out to (3, 4).
    expect(alt).not.toEqual(rel);
    expect(alt.x).toBeCloseTo(3, 10);
    expect(alt.y).toBeCloseTo(4, 10);
  });

  it('aesthetic walks the parity sequence in reverse', () => {
    // A parity sequence that is NOT a palindrome, so direction is observable.
    const parity = [1, 0, 0];
    const params: CoralParams = { oddAngle: 90, evenAngle: 0, lineLength: 1, rotation: 0 };
    // Aesthetic signs the turn by parity (odd +, even −) and reverses the walk,
    // so [1,0,0] is traversed as [0,0,1]: two straight steps, then a left turn.
    const path = computeCoralPath(parity, params, 'aesthetic');
    const end = endpoint(path);
    expect(end.x).toBeCloseTo(2, 10);
    expect(end.y).toBeCloseTo(1, 10);

    // Feeding the already-reversed sequence walks [1,0,0]: turn 90° first, then
    // two straight steps -> (0,3). A different endpoint confirms the traversal
    // order genuinely matters (the sequence is not being read symmetrically).
    const reversedInput = computeCoralPath([...parity].reverse(), params, 'aesthetic');
    expect(endpoint(reversedInput).x).toBeCloseTo(0, 10);
    expect(endpoint(reversedInput).y).toBeCloseTo(3, 10);
  });

  it('aesthetic gives trajectories with a shared tail a shared trunk', () => {
    // Every Collatz trajectory ends ...4 -> 2 -> 1, i.e. a run of even steps.
    // Reversed, that common tail leads, so the first segments must coincide —
    // this is exactly what makes many overlaid paths form a trunk.
    const params: CoralParams = { oddAngle: 20, evenAngle: -12, lineLength: 3, rotation: -90 };
    const a = computeCoralPath([1, 0, 1, 0, 0, 0, 0], params, 'aesthetic'); // n=3
    const b = computeCoralPath([1, 1, 0, 1, 0, 0, 0, 0], params, 'aesthetic'); // longer, same tail
    // Both reversed sequences begin [0,0,0,0,...] -> first four segments identical.
    for (let i = 0; i <= 4; i++) {
      expect(a[i]?.x).toBeCloseTo(b[i]?.x ?? NaN, 10);
      expect(a[i]?.y).toBeCloseTo(b[i]?.y ?? NaN, 10);
    }
  });

  it('rotation offset rotates the whole drawing', () => {
    const straight = [0, 0, 0]; // three even (0°) segments -> a straight line
    const params: CoralParams = { oddAngle: 0, evenAngle: 0, lineLength: 1, rotation: 90 };
    const end = endpoint(computeCoralPath(straight, params, 'relative'));
    // Heading starts at 90° (up), so three unit steps land at (0, 3).
    expect(end.x).toBeCloseTo(0, 10);
    expect(end.y).toBeCloseTo(3, 10);
  });
});
