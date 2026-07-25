/**
 * Coral / Branch Visualization canvas (§5.5).
 *
 * A pure consumer of the immutable Trajectory. It reads the **already-computed**
 * `parity_sequence` from `system_specific_metrics` (Principle #3 — it never
 * recomputes parity from `state_sequence`) and renders the turtle path from
 * [`coralPath`](./coralPath). If the parity metric is absent it shows the FROZEN
 * "Metric Not Supported" fallback (§5.2) rather than a blank or a crash.
 */

import { useEffect, useMemo, useRef } from 'react';
import type { Trajectory } from '@/types/trajectory';
import { METRIC_NOT_SUPPORTED } from '@/types/trajectory';
import { computeCoralPath, pathBounds, type CoralParams, type DirectionRule, type Point } from './coralPath';

export interface CoralProps {
  trajectory: Trajectory;
  params: CoralParams;
  rule: DirectionRule;
  opacity: number;
  scale: number;
  height?: number;
}

const ODD_COLOR = '#f0883e'; // orange — segments following an odd-parity transition
const EVEN_COLOR = '#3fb950'; // green — segments following an even-parity transition

/** Reads the pre-computed parity sequence off the Trajectory (never recomputed). */
function readParity(trajectory: Trajectory): number[] | null {
  const raw = trajectory.system_specific_metrics['parity_sequence'];
  if (!Array.isArray(raw)) {
    return null;
  }
  return raw.map((v) => (v === 1 ? 1 : 0));
}

export function Coral({
  trajectory,
  params,
  rule,
  opacity,
  scale,
  height = 360,
}: CoralProps): JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const parity = useMemo(() => readParity(trajectory), [trajectory]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !parity) {
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
      ctx.clearRect(0, 0, cssWidth, cssHeight);
      if (parity.length === 0) {
        return;
      }
      const points = computeCoralPath(parity, params, rule);
      drawCoral(ctx, cssWidth, cssHeight, points, parity, opacity, scale);
    };

    draw();
    let observer: ResizeObserver | undefined;
    if (typeof ResizeObserver !== 'undefined') {
      observer = new ResizeObserver(() => draw());
      observer.observe(canvas);
    }
    return () => observer?.disconnect();
  }, [parity, params, rule, opacity, scale, height]);

  if (!parity) {
    return (
      <div
        className="flex items-center justify-center rounded-lg bg-slate-950/50 text-slate-500"
        style={{ height }}
      >
        {METRIC_NOT_SUPPORTED}
      </div>
    );
  }

  return (
    <canvas
      ref={canvasRef}
      style={{ width: '100%', height, display: 'block' }}
      className="rounded-lg bg-slate-950/50"
      role="img"
      aria-label={`Coral visualization for initial state ${trajectory.initial_state}`}
    />
  );
}

/** Draws the path, auto-fit to the canvas, then zoomed by the user `scale`. */
function drawCoral(
  ctx: CanvasRenderingContext2D,
  width: number,
  height: number,
  points: Point[],
  parity: number[],
  opacity: number,
  scale: number,
): void {
  if (points.length < 2) {
    return;
  }
  const bounds = pathBounds(points);
  const boxWidth = bounds.maxX - bounds.minX || 1;
  const boxHeight = bounds.maxY - bounds.minY || 1;
  const pad = 18;
  const fit = Math.min((width - 2 * pad) / boxWidth, (height - 2 * pad) / boxHeight);
  const s = fit * scale;
  const cx = (bounds.minX + bounds.maxX) / 2;
  const cy = (bounds.minY + bounds.maxY) / 2;
  const ox = width / 2;
  const oy = height / 2;

  // Canvas y grows downward, so flip y to keep the turtle's +y pointing up.
  const projX = (x: number): number => ox + (x - cx) * s;
  const projY = (y: number): number => oy - (y - cy) * s;

  ctx.globalAlpha = opacity;
  ctx.lineWidth = 1.2;
  for (let i = 0; i < points.length - 1; i++) {
    const a = points[i];
    const b = points[i + 1];
    if (!a || !b) {
      continue;
    }
    ctx.strokeStyle = parity[i] === 1 ? ODD_COLOR : EVEN_COLOR;
    ctx.beginPath();
    ctx.moveTo(projX(a.x), projY(a.y));
    ctx.lineTo(projX(b.x), projY(b.y));
    ctx.stroke();
  }
  ctx.globalAlpha = 1;
}
