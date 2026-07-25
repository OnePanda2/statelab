/**
 * Multi-trajectory overlay chart for the Comparison Lab (§6.3), with the three
 * FROZEN overlay modes: **Raw**, **Log**, **Normalized**. Pure consumer of the
 * immutable trajectories; the BigInt→f64 conversion happens only in the render
 * call (§4.5). x is each trajectory's progress in `[0,1]`, so different-length
 * trajectories align for shape comparison.
 */

import { useEffect, useRef } from 'react';
import type { Trajectory } from '@/types/trajectory';
import { normalizeSeries } from './features';

export type OverlayMode = 'raw' | 'log' | 'normalized';

export interface OverlaySeries {
  trajectory: Trajectory;
  color: string;
}

export interface OverlayChartProps {
  series: OverlaySeries[];
  mode: OverlayMode;
  height?: number;
}

export function OverlayChart({ series, mode, height = 300 }: OverlayChartProps): JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }
    const draw = (): void => {
      const ctx = canvas.getContext('2d');
      if (!ctx) {
        return;
      }
      const dpr = window.devicePixelRatio || 1;
      const cssWidth = canvas.clientWidth || 600;
      const cssHeight = height;
      canvas.width = Math.round(cssWidth * dpr);
      canvas.height = Math.round(cssHeight * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

      // BigInt string -> f64 at the render boundary, then per-mode transform.
      const transformed = series.map((s) => {
        const values = s.trajectory.state_sequence.map((v) => Number(v));
        if (mode === 'normalized') {
          return { color: s.color, values: normalizeSeries(values) };
        }
        if (mode === 'log') {
          return { color: s.color, values: values.map((v) => Math.log10(v + 1)) };
        }
        return { color: s.color, values };
      });

      drawOverlay(ctx, cssWidth, cssHeight, transformed);
    };

    draw();
    let observer: ResizeObserver | undefined;
    if (typeof ResizeObserver !== 'undefined') {
      observer = new ResizeObserver(() => draw());
      observer.observe(canvas);
    }
    return () => observer?.disconnect();
  }, [series, mode, height]);

  return (
    <canvas
      ref={canvasRef}
      style={{ width: '100%', height, display: 'block' }}
      className="rounded-lg bg-slate-950/50"
      role="img"
      aria-label={`Overlay comparison chart (${mode} mode) of ${series.length} trajectories`}
    />
  );
}

function drawOverlay(
  ctx: CanvasRenderingContext2D,
  width: number,
  height: number,
  series: { color: string; values: number[] }[],
): void {
  ctx.clearRect(0, 0, width, height);
  const all = series.flatMap((s) => s.values).filter((v) => Number.isFinite(v));
  if (all.length === 0) {
    return;
  }
  const min = Math.min(...all, 0);
  const max = Math.max(...all);
  const span = max - min || 1;
  const pad = { l: 10, r: 10, t: 14, b: 10 };

  // Grid.
  ctx.strokeStyle = 'rgba(255,255,255,0.06)';
  ctx.lineWidth = 1;
  for (let g = 0; g <= 4; g++) {
    const gy = pad.t + (g / 4) * (height - pad.t - pad.b);
    ctx.beginPath();
    ctx.moveTo(pad.l, gy);
    ctx.lineTo(width - pad.r, gy);
    ctx.stroke();
  }

  for (const s of series) {
    const n = s.values.length;
    if (n === 0) {
      continue;
    }
    ctx.strokeStyle = s.color;
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    let started = false;
    s.values.forEach((v, i) => {
      if (!Number.isFinite(v)) {
        return;
      }
      const x = pad.l + (n <= 1 ? 0 : (i / (n - 1)) * (width - pad.l - pad.r));
      const y = height - pad.b - ((v - min) / span) * (height - pad.t - pad.b);
      if (started) {
        ctx.lineTo(x, y);
      } else {
        ctx.moveTo(x, y);
        started = true;
      }
    });
    ctx.stroke();
  }
}
