import { describe, expect, it } from 'vitest';
import { makeTrajectory } from '@/test/fixtures';
import { applyScale, seriesBounds, trajectoryToValues } from './chartCanvas';

describe('chartCanvas transforms', () => {
  it('converts a state sequence of decimal strings to f64 values', () => {
    const values = trajectoryToValues(makeTrajectory(['3', '10', '5', '16']));
    expect(values).toEqual([3, 10, 5, 16]);
  });

  it('leaves values unchanged under the linear scale', () => {
    expect(applyScale([1, 10, 100], 'linear')).toEqual([1, 10, 100]);
  });

  it('applies log10(v + 1) under the log scale', () => {
    const scaled = applyScale([0, 9, 99], 'log');
    expect(scaled[0]).toBeCloseTo(0, 10); // log10(1)
    expect(scaled[1]).toBeCloseTo(1, 10); // log10(10)
    expect(scaled[2]).toBeCloseTo(2, 10); // log10(100)
  });

  it('computes bounds anchored at zero, ignoring non-finite values', () => {
    expect(seriesBounds([3, 10, 5])).toEqual({ min: 0, max: 10 });
    expect(seriesBounds([])).toEqual({ min: 0, max: 1 });
    expect(seriesBounds([Infinity, 4])).toEqual({ min: 0, max: 4 });
  });
});
