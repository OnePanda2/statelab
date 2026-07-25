/**
 * TypeScript mirror of the Rust Trajectory schema (§4.3), schema version 1.0.0.
 *
 * FROZEN rules this mirror obeys:
 *  - Every state value is a decimal **string** (BigInt-safe) — never a native
 *    `number`. The frontend must never parse these into `number` before the
 *    render boundary (§4.5), and never compute a trajectory field itself: a value
 *    absent from the JSON renders as "Metric Not Supported" (§5.2), it is not
 *    derived client-side.
 *  - `system_specific_metrics` is an open, system-defined map. Missing keys are
 *    rendered as "Metric Not Supported", never silently omitted from the UI.
 *
 * Keep this file in lockstep with `crates/statelab-engine/src/trajectory.rs`.
 * Any additive schema change bumps `TRAJECTORY_SCHEMA_VERSION` (§4.9).
 */

export const TRAJECTORY_SCHEMA_VERSION = '1.0.0';

/** Machine-readable terminal status (§4.7). Exactly one of these four. */
export type TrajectoryStatus =
  | 'Converged'
  | 'CycleDetected'
  | 'IterationLimitReached'
  | 'SystemError';

/** Cycle details; present only when `trajectory_status === 'CycleDetected'`. */
export interface CycleInfo {
  cycle_start_index: number;
  cycle_length: number;
  /** The revisited state, as a decimal string. */
  repeated_state: string;
}

/** Reproducibility / audit metadata (§4.3). */
export interface ExecutionMetadata {
  computation_duration_ms: number;
  engine_version: string;
  cache_hit: boolean;
  iteration_limit_used: number;
  /** RFC 3339 UTC timestamp. */
  timestamp: string;
  /** `<arch>-<os>` platform string. */
  platform: string;
}

/**
 * A metric value. Intentionally permissive: integers, decimal-string big
 * integers, ratios, arrays, and nested objects all appear. `null` denotes an
 * explicit N/A (e.g. stopping time for n = 1) — distinct from an absent key,
 * which renders as "Metric Not Supported".
 */
export type MetricValue =
  | number
  | string
  | boolean
  | null
  | MetricValue[]
  | { [key: string]: MetricValue };

/** The system-defined, immutable metrics dictionary. */
export type SystemMetrics = Record<string, MetricValue>;

/** The immutable record of one completed engine run (§4.3). */
export interface Trajectory {
  trajectory_schema_version: string;
  system_id: string;
  system_version: string;
  /** Initial state as a decimal string. */
  initial_state: string;
  /** Every state including the initial one, in order, as decimal strings. */
  state_sequence: string[];
  iteration_count: number;
  trajectory_status: TrajectoryStatus;
  termination_reason: string;
  cycle_information: CycleInfo | null;
  execution_metadata: ExecutionMetadata;
  system_specific_metrics: SystemMetrics;
}

/**
 * The FROZEN fallback string consumers render when a metric key is absent —
 * rather than computing a substitute (Principle #3 / §5.2).
 */
export const METRIC_NOT_SUPPORTED = 'Metric Not Supported';

/**
 * Reads a metric by key. `undefined` means the key is **absent** — the consumer
 * should render {@link METRIC_NOT_SUPPORTED}. A present key whose value is `null`
 * is a real N/A (e.g. stopping time for n = 1), distinct from absent.
 */
export function readMetric(metrics: SystemMetrics, key: string): MetricValue | undefined {
  return metrics[key];
}
