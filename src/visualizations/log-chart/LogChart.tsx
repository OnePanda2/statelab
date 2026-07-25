/**
 * Logarithmic Chart (§5.4): the same underlying data as the Value Chart on a
 * log-scale y-axis. Same BigInt→f64 render-boundary rule applies. Pure consumer of
 * the immutable Trajectory.
 */

import type { Trajectory } from '@/types/trajectory';
import { TrajectoryChart } from '../TrajectoryChart';

export interface LogChartProps {
  trajectory: Trajectory;
  height?: number;
}

export function LogChart({ trajectory, height }: LogChartProps): JSX.Element {
  return (
    <TrajectoryChart
      trajectory={trajectory}
      scale="log"
      title="Logarithmic chart"
      height={height}
    />
  );
}
