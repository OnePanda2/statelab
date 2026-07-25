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
    for (const rule of ['relative', 'absolute', 'rotate-before', 'rotate-after', 'alternating'] as const) {
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

  it('rotation offset rotates the whole drawing', () => {
    const straight = [0, 0, 0]; // three even (0°) segments -> a straight line
    const params: CoralParams = { oddAngle: 0, evenAngle: 0, lineLength: 1, rotation: 90 };
    const end = endpoint(computeCoralPath(straight, params, 'relative'));
    // Heading starts at 90° (up), so three unit steps land at (0, 3).
    expect(end.x).toBeCloseTo(0, 10);
    expect(end.y).toBeCloseTo(3, 10);
  });
});
