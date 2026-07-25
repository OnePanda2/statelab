/**
 * Streaming aggregate for the Dataset Explorer (§6.2).
 *
 * As summary rows stream in, they are folded into a fixed-size aggregate — so the
 * app tracks results for an arbitrarily large dataset in **O(1) memory** (plus a
 * small bounded window of rows for the table). No full dataset is ever retained.
 */

import type { DatasetSummaryRow } from '@/lib/invoke';

export interface DatasetAggregate {
  count: number;
  converged: number;
  cycleDetected: number;
  iterationLimit: number;
  systemError: number;
  sumIterations: number;
  maxIterations: number;
  /** initial_state that produced the largest iteration_count so far. */
  longestInitialState: string;
  /** Largest peak value seen, compared by decimal-string magnitude. */
  largestPeak: string;
}

export function emptyAggregate(): DatasetAggregate {
  return {
    count: 0,
    converged: 0,
    cycleDetected: 0,
    iterationLimit: 0,
    systemError: 0,
    sumIterations: 0,
    maxIterations: 0,
    longestInitialState: '',
    largestPeak: '0',
  };
}

/** Mean iterations across all rows folded so far (0 when empty). */
export function meanIterations(agg: DatasetAggregate): number {
  return agg.count === 0 ? 0 : agg.sumIterations / agg.count;
}

/** Compares two non-negative decimal strings by magnitude (a > b ? 1 …). */
export function compareDecimal(a: string, b: string): number {
  const na = a.replace(/^0+(?=\d)/, '');
  const nb = b.replace(/^0+(?=\d)/, '');
  if (na.length !== nb.length) {
    return na.length < nb.length ? -1 : 1;
  }
  return na === nb ? 0 : na < nb ? -1 : 1;
}

/**
 * Folds one row into the aggregate, mutating it in place (called once per streamed
 * row, so allocation-free on the hot path). Returns the same object for chaining.
 */
export function accumulate(agg: DatasetAggregate, row: DatasetSummaryRow): DatasetAggregate {
  agg.count += 1;
  agg.sumIterations += row.iteration_count;

  switch (row.status) {
    case 'Converged':
      agg.converged += 1;
      break;
    case 'CycleDetected':
      agg.cycleDetected += 1;
      break;
    case 'IterationLimitReached':
      agg.iterationLimit += 1;
      break;
    case 'SystemError':
      agg.systemError += 1;
      break;
  }

  if (row.iteration_count > agg.maxIterations) {
    agg.maxIterations = row.iteration_count;
    agg.longestInitialState = row.initial_state;
  }
  if (row.peak_value !== null && compareDecimal(row.peak_value, agg.largestPeak) > 0) {
    agg.largestPeak = row.peak_value;
  }
  return agg;
}
