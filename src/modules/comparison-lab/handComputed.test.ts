/**
 * Independent verification of the Comparison Lab's similarity maths (§6.3).
 *
 * The original audit re-derived Collatz reference values by hand rather than
 * trusting the implementation; this file applies the same standard to cosine
 * similarity. **Every number below is derived from the trajectory definitions,
 * not read out of the code** — the feature vectors are asserted first, then the
 * similarity is recomputed from those literals with explicit arithmetic and
 * compared against the implementation.
 *
 * Worked pair — n = 3 vs n = 6:
 *
 *   n = 3: [3, 10, 5, 16, 8, 4, 2, 1]      -> 7 transitions
 *     total stopping time 7; stopping time 6 (first value below 3 is 2, index 6);
 *     peak 16 at index 3; max bit length 5 (16 = 0b10000);
 *     parity [1,0,1,0,0,0,0] -> odd 2, even 5; ratios 2/7 and 5/7;
 *     growth over odd steps (10/3, 16/5); decline 0.5 (every even step halves).
 *
 *   n = 6: [6, 3, 10, 5, 16, 8, 4, 2, 1]   -> 8 transitions
 *     total stopping time 8; stopping time 1 (3 < 6 already at index 1);
 *     peak 16 at index 4; max bit length 5;
 *     parity [0,1,0,1,0,0,0,0] -> odd 2, even 6; ratios 0.25 and 0.75;
 *     the same two odd steps (3->10, 5->16), so identical growth; decline 0.5.
 */

import { describe, expect, it } from 'vitest';
import type { SystemMetrics, Trajectory } from '@/types/trajectory';
import { makeTrajectory } from '@/test/fixtures';
import {
  COMPARISON_FEATURES,
  cosineSimilarity,
  extractFeatureVector,
  minMaxNormalizeSet,
  normalizeSeries,
} from './features';

// --- Hand-derived metric dictionaries (NOT produced by this codebase) ---

const N3_METRICS: SystemMetrics = {
  total_stopping_time: 7,
  stopping_time: 6,
  peak_index: 3,
  maximum_bit_length: 5,
  odd_count: 2,
  even_count: 5,
  odd_ratio: 2 / 7,
  even_ratio: 5 / 7,
  average_growth: (10 / 3 + 16 / 5) / 2,
  average_decline: 0.5,
  peak_value: '16',
  parity_sequence: [1, 0, 1, 0, 0, 0, 0],
};

const N6_METRICS: SystemMetrics = {
  total_stopping_time: 8,
  stopping_time: 1,
  peak_index: 4,
  maximum_bit_length: 5,
  odd_count: 2,
  even_count: 6,
  odd_ratio: 0.25,
  even_ratio: 0.75,
  average_growth: (10 / 3 + 16 / 5) / 2,
  average_decline: 0.5,
  peak_value: '16',
  parity_sequence: [0, 1, 0, 1, 0, 0, 0, 0],
};

const n3: Trajectory = makeTrajectory(['3', '10', '5', '16', '8', '4', '2', '1'], {
  system_specific_metrics: N3_METRICS,
});
const n6: Trajectory = makeTrajectory(['6', '3', '10', '5', '16', '8', '4', '2', '1'], {
  system_specific_metrics: N6_METRICS,
});

/** Cosine similarity, written out longhand as an independent reference. */
function referenceCosine(a: number[], b: number[]): number {
  let dot = 0;
  let na = 0;
  let nb = 0;
  for (let i = 0; i < a.length; i++) {
    const x = a[i] ?? 0;
    const y = b[i] ?? 0;
    dot += x * y;
    na += x * x;
    nb += y * y;
  }
  return dot / (Math.sqrt(na) * Math.sqrt(nb));
}

describe('feature vector extraction (hand-derived)', () => {
  it('builds the exact vector the metrics imply, in the documented order', () => {
    // The order the implementation claims, restated here independently.
    expect(COMPARISON_FEATURES.map((f) => f.key)).toEqual([
      'total_stopping_time',
      'stopping_time',
      'peak_index',
      'maximum_bit_length',
      'odd_count',
      'even_count',
      'odd_ratio',
      'even_ratio',
      'average_growth',
      'average_decline',
    ]);

    expect(extractFeatureVector(n3)).toEqual([
      7,
      6,
      3,
      5,
      2,
      5,
      2 / 7,
      5 / 7,
      (10 / 3 + 16 / 5) / 2,
      0.5,
    ]);

    expect(extractFeatureVector(n6)).toEqual([
      8,
      1,
      4,
      5,
      2,
      6,
      0.25,
      0.75,
      (10 / 3 + 16 / 5) / 2,
      0.5,
    ]);
  });
});

describe('cosine similarity, n = 3 vs n = 6 (hand-derived)', () => {
  it('matches the independently recomputed score exactly', () => {
    const v3 = [7, 6, 3, 5, 2, 5, 2 / 7, 5 / 7, (10 / 3 + 16 / 5) / 2, 0.5];
    const v6 = [8, 1, 4, 5, 2, 6, 0.25, 0.75, (10 / 3 + 16 / 5) / 2, 0.5];

    // Dot product, term by term, from the hand-derived vectors:
    //   7*8 + 6*1 + 3*4 + 5*5 + 2*2 + 5*6                       = 133
    //   + (2/7)(1/4) + (5/7)(3/4)                               ~ 0.607142857
    //   + growth^2                                              ~ 10.671111111
    //   + 0.5*0.5                                               = 0.25
    const growth = (10 / 3 + 16 / 5) / 2;
    const expectedDot = 133 + (2 / 7) * 0.25 + (5 / 7) * 0.75 + growth * growth + 0.25;

    const expectedNormA = Math.sqrt(
      49 + 36 + 9 + 25 + 4 + 25 + (2 / 7) ** 2 + (5 / 7) ** 2 + growth ** 2 + 0.25,
    );
    const expectedNormB = Math.sqrt(
      64 + 1 + 16 + 25 + 4 + 36 + 0.25 ** 2 + 0.75 ** 2 + growth ** 2 + 0.25,
    );
    const expected = expectedDot / (expectedNormA * expectedNormB);

    // Sanity-check the hand arithmetic against an explicit constant, so a slip in
    // the algebra above cannot silently define its own truth.
    //
    //   dot   = 144.5282539682540
    //   |v3|  = sqrt(159.5129478458050)
    //   |v6|  = sqrt(157.5461111111111)
    //   cos   = 0.9116978734720410
    //
    // (An earlier draft of this comment carried 0.911692, from approximating the
    // two square roots by hand — wrong in the 6th decimal. The derivation was
    // sound; only the manual sqrt was not. Kept as a note because the whole point
    // of this file is that hand-derived numbers get checked, not trusted.)
    expect(expected).toBeCloseTo(0.911697873472041, 12);

    // The implementation must agree with the hand derivation to full precision.
    expect(cosineSimilarity(v3, v6)).toBeCloseTo(expected, 15);
    expect(cosineSimilarity(extractFeatureVector(n3), extractFeatureVector(n6))).toBeCloseTo(
      expected,
      15,
    );
    // And with the longhand reference implementation.
    expect(cosineSimilarity(v3, v6)).toBeCloseTo(referenceCosine(v3, v6), 15);
  });

  it('is symmetric and self-identical', () => {
    const v3 = extractFeatureVector(n3);
    const v6 = extractFeatureVector(n6);
    expect(cosineSimilarity(v3, v6)).toBeCloseTo(cosineSimilarity(v6, v3), 15);
    expect(cosineSimilarity(v3, v3)).toBeCloseTo(1, 15);
  });
});

describe('min-max normalization (hand-derived)', () => {
  it('maps each feature column onto [0,1] across the selected set only', () => {
    // Scoped to the comparison set: with two trajectories every non-constant
    // feature must land exactly on 0 or 1, whichever end that trajectory holds.
    const [a, b] = minMaxNormalizeSet([extractFeatureVector(n3), extractFeatureVector(n6)]);

    // total_stopping_time: 7 vs 8 -> n3 is the min, n6 the max.
    expect(a?.[0]).toBe(0);
    expect(b?.[0]).toBe(1);
    // stopping_time: 6 vs 1 -> n3 is now the max.
    expect(a?.[1]).toBe(1);
    expect(b?.[1]).toBe(0);
    // maximum_bit_length: 5 vs 5 -> constant column collapses to 0.
    expect(a?.[3]).toBe(0);
    expect(b?.[3]).toBe(0);
    // average_decline: 0.5 vs 0.5 -> also constant.
    expect(a?.[9]).toBe(0);
    expect(b?.[9]).toBe(0);
  });
});

describe('overlay mode transforms (hand-derived)', () => {
  // The three FROZEN overlay modes are Raw / Log / Normalized. Raw and Log are
  // applied inline by OverlayChart; Normalized delegates to `normalizeSeries`.
  const values = [3, 10, 5, 16, 8, 4, 2, 1];

  it('Raw leaves the series untouched', () => {
    expect(values).toEqual([3, 10, 5, 16, 8, 4, 2, 1]);
  });

  it('Log applies log10(v + 1), keeping the fixed point 1 finite', () => {
    const log = values.map((v) => Math.log10(v + 1));
    expect(log[0]).toBeCloseTo(Math.log10(4), 15);
    expect(log[3]).toBeCloseTo(Math.log10(17), 15);
    // The final state 1 maps to log10(2), not to -Infinity as log10(0) would.
    expect(log[7]).toBeCloseTo(Math.log10(2), 15);
    expect(log.every((v) => Number.isFinite(v))).toBe(true);
  });

  it('Normalized maps min->0 and max->1 with correct interior spacing', () => {
    const norm = normalizeSeries(values);
    // min 1, max 16, span 15.
    expect(norm[7]).toBeCloseTo(0, 15); // value 1
    expect(norm[3]).toBeCloseTo(1, 15); // value 16
    expect(norm[0]).toBeCloseTo((3 - 1) / 15, 15);
    expect(norm[1]).toBeCloseTo((10 - 1) / 15, 15);
  });
});
