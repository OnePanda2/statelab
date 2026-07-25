/**
 * Comparison Lab feature math (§6.3).
 *
 * Derives a numeric **feature vector** from a trajectory's already-computed
 * System-Specific Metrics, and provides cosine similarity + min-max normalization
 * over sets of such vectors. Pure functions — no trajectory is recomputed; a
 * missing or non-numeric metric contributes 0.
 */

import type { MetricValue, Trajectory } from '@/types/trajectory';

/** The numeric metrics that make up a comparison feature vector, in fixed order. */
export const COMPARISON_FEATURES: { key: string; label: string }[] = [
  { key: 'total_stopping_time', label: 'Total stopping time' },
  { key: 'stopping_time', label: 'Stopping time' },
  { key: 'peak_index', label: 'Peak index' },
  { key: 'maximum_bit_length', label: 'Max bit length' },
  { key: 'odd_count', label: 'Odd count' },
  { key: 'even_count', label: 'Even count' },
  { key: 'odd_ratio', label: 'Odd ratio' },
  { key: 'even_ratio', label: 'Even ratio' },
  { key: 'average_growth', label: 'Average growth' },
  { key: 'average_decline', label: 'Average decline' },
];

/** Coerces a metric value to a finite number (non-numeric / N/A → 0). */
function toNumber(value: MetricValue | undefined): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : 0;
}

/**
 * Extracts the ordered numeric feature vector from a trajectory's metrics. Big
 * integers (e.g. peak value) are represented by their derived numeric features
 * (peak index, max bit length) rather than a lossy `f64`.
 */
export function extractFeatureVector(trajectory: Trajectory): number[] {
  return COMPARISON_FEATURES.map((f) => toNumber(trajectory.system_specific_metrics[f.key]));
}

/** Cosine similarity of two equal-length vectors; 0 if either has zero magnitude. */
export function cosineSimilarity(a: number[], b: number[]): number {
  const n = Math.min(a.length, b.length);
  let dot = 0;
  let normA = 0;
  let normB = 0;
  for (let i = 0; i < n; i++) {
    const x = a[i] ?? 0;
    const y = b[i] ?? 0;
    dot += x * y;
    normA += x * x;
    normB += y * y;
  }
  if (normA === 0 || normB === 0) {
    return 0;
  }
  return dot / (Math.sqrt(normA) * Math.sqrt(normB));
}

/**
 * Min-max normalizes a set of feature vectors **per feature** (column-wise) into
 * `[0, 1]`. A feature that is constant across the set maps to 0 (no spread).
 */
export function minMaxNormalizeSet(vectors: number[][]): number[][] {
  const first = vectors[0];
  if (!first) {
    return [];
  }
  const dim = first.length;
  const min = new Array<number>(dim).fill(Infinity);
  const max = new Array<number>(dim).fill(-Infinity);

  for (const v of vectors) {
    for (let i = 0; i < dim; i++) {
      const x = v[i] ?? 0;
      if (x < (min[i] ?? Infinity)) {
        min[i] = x;
      }
      if (x > (max[i] ?? -Infinity)) {
        max[i] = x;
      }
    }
  }

  return vectors.map((v) =>
    v.map((x, i) => {
      const lo = min[i] ?? 0;
      const hi = max[i] ?? 0;
      const span = hi - lo;
      return span === 0 ? 0 : (x - lo) / span;
    }),
  );
}

/** Min-max normalizes a single series into `[0, 1]` (its own min/max). */
export function normalizeSeries(values: number[]): number[] {
  const finite = values.filter((v) => Number.isFinite(v));
  if (finite.length === 0) {
    return values.map(() => 0);
  }
  const min = Math.min(...finite);
  const max = Math.max(...finite);
  const span = max - min;
  return values.map((v) => (span === 0 ? 0 : (v - min) / span));
}

/** Pairwise cosine similarity matrix over a set of feature vectors. */
export function similarityMatrix(vectors: number[][]): number[][] {
  return vectors.map((a) => vectors.map((b) => cosineSimilarity(a, b)));
}
