/**
 * Feature Analysis metric registry + value formatting (§6.1).
 *
 * This module performs **no computation** over trajectories — it only describes
 * how to *label* and *format* the System-Specific Metrics that the engine already
 * embedded in the Trajectory Object. The formatter distinguishes three states:
 *
 *   - **absent** (`undefined`): the key is not in the metrics map → the FROZEN
 *     "Metric Not Supported" sentinel (§5.2). It is never computed here.
 *   - **N/A** (`null`): a present-but-not-applicable value (e.g. stopping time for
 *     n = 1).
 *   - **present**: an integer, float, decimal-string big integer, array, or object.
 */

import { METRIC_NOT_SUPPORTED, type MetricValue } from '@/types/trajectory';

/** A metric's display label and grouping, in presentation order. */
export interface MetricDescriptor {
  key: string;
  label: string;
  group: MetricGroup;
}

export type MetricGroup = 'Timing' | 'Extremes' | 'Parity' | 'Growth' | 'Sequences';

/** The presentation order of groups. */
export const METRIC_GROUPS: MetricGroup[] = ['Timing', 'Extremes', 'Parity', 'Growth', 'Sequences'];

/**
 * The Classic Collatz metrics, in display order. Mirrors the §4.4 table (all 15
 * defined metrics). New systems can supply their own descriptor lists later; the
 * Feature Analysis surface is generic over this list.
 */
export const COLLATZ_METRICS: MetricDescriptor[] = [
  { key: 'stopping_time', label: 'Stopping Time', group: 'Timing' },
  { key: 'total_stopping_time', label: 'Total Stopping Time', group: 'Timing' },
  { key: 'peak_value', label: 'Peak Value', group: 'Extremes' },
  { key: 'peak_index', label: 'Peak Index', group: 'Extremes' },
  { key: 'maximum_bit_length', label: 'Maximum Bit Length', group: 'Extremes' },
  { key: 'odd_count', label: 'Odd Count', group: 'Parity' },
  { key: 'even_count', label: 'Even Count', group: 'Parity' },
  { key: 'odd_ratio', label: 'Odd Ratio', group: 'Parity' },
  { key: 'even_ratio', label: 'Even Ratio', group: 'Parity' },
  { key: 'average_growth', label: 'Average Growth', group: 'Growth' },
  { key: 'average_decline', label: 'Average Decline', group: 'Growth' },
  { key: 'parity_sequence', label: 'Parity Sequence', group: 'Sequences' },
  { key: 'bit_length_evolution', label: 'Bit Length Evolution', group: 'Sequences' },
  { key: 'binary_transition_statistics', label: 'Binary Transition Statistics', group: 'Sequences' },
  { key: 'run_length_statistics', label: 'Run Length Statistics', group: 'Sequences' },
];

/** Whether a metric value is present at all (i.e. the key exists in the map). */
export function isSupported(value: MetricValue | undefined): boolean {
  return value !== undefined;
}

/** Whether a present value is an explicit N/A. */
export function isNotApplicable(value: MetricValue | undefined): boolean {
  return value === null;
}

const MAX_ARRAY_ITEMS = 60;

/** Formats a single scalar (leaf) metric value for display. */
function formatScalar(value: MetricValue): string {
  if (value === null) {
    return 'N/A';
  }
  if (typeof value === 'number') {
    return Number.isInteger(value) ? String(value) : value.toFixed(6);
  }
  if (typeof value === 'boolean') {
    return String(value);
  }
  if (typeof value === 'string') {
    return value;
  }
  if (Array.isArray(value)) {
    const shown = value.slice(0, MAX_ARRAY_ITEMS).map(formatScalar).join(', ');
    return value.length > MAX_ARRAY_ITEMS
      ? `[${shown}, … (${value.length} items)]`
      : `[${shown}]`;
  }
  return Object.entries(value)
    .map(([k, v]) => `${k}: ${formatScalar(v)}`)
    .join(', ');
}

/**
 * Formats a metric value for display. Absent (`undefined`) yields the FROZEN
 * "Metric Not Supported" string; everything else is rendered from what the engine
 * already produced — no substitute is computed.
 */
export function formatMetricValue(value: MetricValue | undefined): string {
  if (value === undefined) {
    return METRIC_NOT_SUPPORTED;
  }
  return formatScalar(value);
}
