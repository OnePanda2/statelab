import { describe, expect, it } from 'vitest';
import { APPENDIX_B_METRICS, makeTrajectory } from '@/test/fixtures';
import {
  COMPARISON_FEATURES,
  cosineSimilarity,
  extractFeatureVector,
  minMaxNormalizeSet,
  normalizeSeries,
  similarityMatrix,
} from './features';

describe('cosineSimilarity', () => {
  it('is 1 for identical direction, -1 for opposite, 0 for orthogonal', () => {
    expect(cosineSimilarity([1, 2, 3], [1, 2, 3])).toBeCloseTo(1, 10);
    expect(cosineSimilarity([1, 0], [-1, 0])).toBeCloseTo(-1, 10);
    expect(cosineSimilarity([1, 0], [0, 1])).toBeCloseTo(0, 10);
  });
  it('is scale-invariant', () => {
    expect(cosineSimilarity([1, 2, 3], [2, 4, 6])).toBeCloseTo(1, 10);
  });
  it('guards zero-magnitude vectors', () => {
    expect(cosineSimilarity([0, 0], [1, 1])).toBe(0);
  });
});

describe('extractFeatureVector', () => {
  it('pulls the fixed numeric features from the metrics, N/A → 0', () => {
    const t = makeTrajectory(['3', '10', '5', '16', '8', '4', '2', '1'], {
      system_specific_metrics: APPENDIX_B_METRICS,
    });
    const v = extractFeatureVector(t);
    expect(v.length).toBe(COMPARISON_FEATURES.length);
    // total_stopping_time=7, stopping_time=6, peak_index=3, max_bit=5, odd=2, even=5
    expect(v.slice(0, 6)).toEqual([7, 6, 3, 5, 2, 5]);
  });

  it('treats a null metric as 0', () => {
    const t = makeTrajectory(['1', '4', '2', '1'], {
      system_specific_metrics: { ...APPENDIX_B_METRICS, stopping_time: null },
    });
    const v = extractFeatureVector(t);
    expect(v[1]).toBe(0); // stopping_time index
  });
});

describe('minMaxNormalizeSet', () => {
  it('normalizes each feature column into [0,1]', () => {
    const out = minMaxNormalizeSet([
      [0, 10],
      [5, 20],
      [10, 30],
    ]);
    expect(out[0]).toEqual([0, 0]);
    expect(out[1]).toEqual([0.5, 0.5]);
    expect(out[2]).toEqual([1, 1]);
  });
  it('maps a constant column to 0', () => {
    expect(minMaxNormalizeSet([[5], [5]])).toEqual([[0], [0]]);
  });
  it('returns [] for an empty set', () => {
    expect(minMaxNormalizeSet([])).toEqual([]);
  });
});

describe('normalizeSeries', () => {
  it('scales a series to [0,1] by its own min/max', () => {
    expect(normalizeSeries([2, 4, 6])).toEqual([0, 0.5, 1]);
  });
  it('maps a flat series to 0', () => {
    expect(normalizeSeries([3, 3, 3])).toEqual([0, 0, 0]);
  });
});

describe('similarityMatrix', () => {
  it('is symmetric with a unit diagonal', () => {
    const m = similarityMatrix([
      [1, 2],
      [2, 1],
    ]);
    expect(m[0]?.[0]).toBeCloseTo(1, 10);
    expect(m[1]?.[1]).toBeCloseTo(1, 10);
    expect(m[0]?.[1]).toBeCloseTo(m[1]?.[0] ?? -1, 10);
  });
});
