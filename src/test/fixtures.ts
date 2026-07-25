/** Test fixtures. */

import type { SystemMetrics, Trajectory } from '@/types/trajectory';

/** The Appendix B (n = 3) System-Specific Metrics, verbatim from the brief. */
export const APPENDIX_B_METRICS: SystemMetrics = {
  stopping_time: 6,
  total_stopping_time: 7,
  peak_value: '16',
  peak_index: 3,
  odd_count: 2,
  even_count: 5,
  odd_ratio: 0.2857142857142857,
  even_ratio: 0.7142857142857143,
  parity_sequence: [1, 0, 1, 0, 0, 0, 0],
  maximum_bit_length: 5,
  bit_length_evolution: [2, 4, 3, 5, 4, 3, 2, 1],
  binary_transition_statistics: { increases: 2, decreases: 5, same: 0 },
  run_length_statistics: [1, 1, 1, 4],
  average_growth: 3.2666666666666666,
  average_decline: 0.5,
};

/** Builds a minimal valid Trajectory from a state sequence, for component tests. */
export function makeTrajectory(sequence: string[], overrides: Partial<Trajectory> = {}): Trajectory {
  return {
    trajectory_schema_version: '1.0.0',
    system_id: 'classic-collatz',
    system_version: '1.0.0',
    initial_state: sequence[0] ?? '0',
    state_sequence: sequence,
    iteration_count: Math.max(0, sequence.length - 1),
    trajectory_status: 'Converged',
    termination_reason: 'Reached fixed value 1',
    cycle_information: null,
    execution_metadata: {
      computation_duration_ms: 0,
      engine_version: '1.0.0',
      cache_hit: false,
      iteration_limit_used: 100_000,
      timestamp: '2026-07-25T00:00:00Z',
      platform: 'test',
    },
    system_specific_metrics: {},
    ...overrides,
  };
}
