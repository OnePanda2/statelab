/**
 * Shared Canvas 2D chart component consumed by [`ValueChart`](./value-chart) and
 * [`LogChart`](./log-chart). Receives an **immutable** Trajectory and renders from
 * it alone (§5.2). It performs no mathematics beyond the render-boundary pixel
 * mapping in [`chartCanvas`](./chartCanvas).
 */

import { useEffect, useRef } from 'react';
import type { Trajectory } from '@/types/trajectory';
import { drawTrajectory, trajectoryToValues, type ScaleKind } from './chartCanvas';

export interface TrajectoryChartProps {
  /** The finalized, immutable Trajectory to plot. */
  trajectory: Trajectory;
  /** Vertical scale: linear (Value Chart) or log (Logarithmic Chart). */
  scale: ScaleKind;
  /** Heading shown above the canvas. */
  title: string;
  /** CSS pixel height of the canvas. */
  height?: number;
}

export function TrajectoryChart({
  trajectory,
  scale,
  title,
  height = 280,
}: TrajectoryChartProps): JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }

    const draw = (): void => {
      const ctx = canvas.getContext('2d');
      if (!ctx) {
        return; // e.g. jsdom in tests — nothing to draw, no crash
      }
      const dpr = window.devicePixelRatio || 1;
      const cssWidth = canvas.clientWidth || 600;
      const cssHeight = height;
      canvas.width = Math.round(cssWidth * dpr);
      canvas.height = Math.round(cssHeight * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

      // BigInt string -> f64 happens here, at the render call, and nowhere else.
      const values = trajectoryToValues(trajectory);
      drawTrajectory(ctx, cssWidth, cssHeight, values, scale);
    };

    draw();

    // Redraw on container resize (guarded for environments without ResizeObserver).
    let observer: ResizeObserver | undefined;
    if (typeof ResizeObserver !== 'undefined') {
      observer = new ResizeObserver(() => draw());
      observer.observe(canvas);
    }
    return () => observer?.disconnect();
  }, [trajectory, scale, height]);

  return (
    <figure className="m-0">
      <figcaption className="mb-2 text-xs uppercase tracking-wide text-slate-400">
        {title}
      </figcaption>
      <canvas
        ref={canvasRef}
        style={{ width: '100%', height, display: 'block' }}
        className="rounded-lg bg-slate-950/50"
        role="img"
        aria-label={`${title} for initial state ${trajectory.initial_state}`}
      />
    </figure>
  );
}
