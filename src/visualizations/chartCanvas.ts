/**
 * Canvas 2D drawing primitives shared by the Value Chart (§5.3) and Logarithmic
 * Chart (§5.4).
 *
 * These functions are **pure consumers** of already-computed trajectory data — the
 * only "mathematics" here is the render-boundary work §4.5 explicitly permits:
 * converting a `BigInt` decimal string to `f64` for pixel mapping, and the log
 * display transform. No trajectory field is computed or corrected here.
 *
 * This module lives under `src/visualizations/`, so the ESLint import-boundary rule
 * forbids it from importing any engine-computation path.
 */

import type { Trajectory } from '@/types/trajectory';

/** Which vertical scale a chart draws in. */
export type ScaleKind = 'linear' | 'log';

/** Colours for a chart. Defaults suit the app's dark theme. */
export interface ChartStyle {
  line: string;
  point: string;
  grid: string;
}

export const DEFAULT_CHART_STYLE: ChartStyle = {
  line: '#58a6ff',
  point: '#58a6ff',
  grid: 'rgba(255,255,255,0.06)',
};

/**
 * Converts a trajectory's `state_sequence` (decimal strings) to `f64` values.
 *
 * **This is the BigInt→f64 boundary (§4.5/§5.3).** It exists only so the canvas can
 * map values to pixels; the engine's own values remain arbitrary-precision. Values
 * beyond `f64` range collapse to `Infinity` and are dropped from the plot (a known,
 * acceptable limitation of pixel rendering — the exact value still lives in the
 * Trajectory Object).
 */
export function trajectoryToValues(trajectory: Trajectory): number[] {
  return trajectory.state_sequence.map((s) => Number(s));
}

/**
 * Applies the vertical scale transform. `linear` is identity; `log` uses
 * `log10(v + 1)` so the fixed point `1` and any `0` states map to finite values.
 */
export function applyScale(values: number[], scale: ScaleKind): number[] {
  if (scale === 'linear') {
    return values;
  }
  return values.map((v) => Math.log10(v + 1));
}

/** Inclusive numeric bounds of a series, with a sensible fallback when empty. */
export function seriesBounds(values: number[]): { min: number; max: number } {
  const finite = values.filter((v) => Number.isFinite(v));
  if (finite.length === 0) {
    return { min: 0, max: 1 };
  }
  return { min: Math.min(...finite, 0), max: Math.max(...finite) };
}

/**
 * Draws the trajectory as a line-with-points chart into `ctx`, sized in CSS
 * pixels (`width` x `height`). `values` are the raw `f64` values; the scale
 * transform is applied here. Safe to call with an empty series (draws nothing).
 */
export function drawTrajectory(
  ctx: CanvasRenderingContext2D,
  width: number,
  height: number,
  values: number[],
  scale: ScaleKind,
  style: ChartStyle = DEFAULT_CHART_STYLE,
): void {
  ctx.clearRect(0, 0, width, height);
  if (values.length === 0) {
    return;
  }

  const scaled = applyScale(values, scale);
  const { min, max } = seriesBounds(scaled);
  const pad = { l: 10, r: 10, t: 14, b: 10 };
  const span = max - min || 1;
  const n = scaled.length;

  const xAt = (i: number): number =>
    pad.l + (n <= 1 ? 0 : (i / (n - 1)) * (width - pad.l - pad.r));
  const yAt = (v: number): number =>
    height - pad.b - ((v - min) / span) * (height - pad.t - pad.b);

  // Horizontal grid.
  ctx.strokeStyle = style.grid;
  ctx.lineWidth = 1;
  for (let g = 0; g <= 4; g++) {
    const gy = pad.t + (g / 4) * (height - pad.t - pad.b);
    ctx.beginPath();
    ctx.moveTo(pad.l, gy);
    ctx.lineTo(width - pad.r, gy);
    ctx.stroke();
  }

  // Trajectory line.
  ctx.strokeStyle = style.line;
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  let started = false;
  scaled.forEach((v, i) => {
    if (!Number.isFinite(v)) {
      return;
    }
    const x = xAt(i);
    const y = yAt(v);
    if (started) {
      ctx.lineTo(x, y);
    } else {
      ctx.moveTo(x, y);
      started = true;
    }
  });
  ctx.stroke();

  // Points (skipped when dense, to keep the line readable).
  if (n <= 400) {
    ctx.fillStyle = style.point;
    scaled.forEach((v, i) => {
      if (!Number.isFinite(v)) {
        return;
      }
      ctx.beginPath();
      ctx.arc(xAt(i), yAt(v), 2, 0, Math.PI * 2);
      ctx.fill();
    });
  }
}
