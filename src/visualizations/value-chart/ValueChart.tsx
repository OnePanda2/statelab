/**
 * Value Chart (§5.3): linear plot of iteration index (x) vs. state value (y),
 * rendered on Canvas 2D. A pure consumer of the immutable Trajectory.
 */

import type { Trajectory } from '@/types/trajectory';
import { TrajectoryChart } from '../TrajectoryChart';

export interface ValueChartProps {
  trajectory: Trajectory;
  height?: number;
}

export function ValueChart({ trajectory, height }: ValueChartProps): JSX.Element {
  return (
    <TrajectoryChart
      trajectory={trajectory}
      scale="linear"
      title="Value chart"
      height={height}
    />
  );
}
