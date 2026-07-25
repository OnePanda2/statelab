/**
 * Coral / Branch Visualization canvas (§5.5).
 *
 * A pure consumer of immutable Trajectories. It reads each one's
 * **already-computed** `parity_sequence` from `system_specific_metrics`
 * (Principle #3 — it never recomputes parity from `state_sequence`) and renders
 * the turtle path from [`coralPath`](./coralPath). If a trajectory's parity
 * metric is absent it shows the FROZEN "Metric Not Supported" fallback (§5.2)
 * rather than a blank or a crash.
 *
 * Accepts **many** trajectories: all are drawn from a common origin, which is
 * what makes the `aesthetic` rule form a tree (see `computeCoralPath`).
 */

import { useEffect, useMemo, useRef } from 'react';
import type { Trajectory } from '@/types/trajectory';
import { METRIC_NOT_SUPPORTED } from '@/types/trajectory';
import {
  computeCoralPath,
  pathBounds,
  type CoralParams,
  type DirectionRule,
  type Point,
} from './coralPath';

export interface CoralProps {
  /** Trajectories to overlay. All share one origin. */
  trajectories: Trajectory[];
  params: CoralParams;
  rule: DirectionRule;
  opacity: number;
  scale: number;
  height?: number;
}

const ODD_COLOR = '#f0883e'; // orange — segments following an odd-parity transition
const EVEN_COLOR = '#3fb950'; // green — segments following an even-parity transition

/** Reads the pre-computed parity sequence off a Trajectory (never recomputed). */
function readParity(trajectory: Trajectory): number[] | null {
  const raw = trajectory.system_specific_metrics['parity_sequence'];
  if (!Array.isArray(raw)) {
    return null;
  }
  return raw.map((v) => (v === 1 ? 1 : 0));
}

export function Coral({
  trajectories,
  params,
  rule,
  opacity,
  scale,
  height = 360,
}: CoralProps): JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const parities = useMemo(
    () => trajectories.map(readParity).filter((p): p is number[] => p !== null && p.length > 0),
    [trajectories],
  );

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || parities.length === 0) {
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

      const aesthetic = rule === 'aesthetic';
      if (aesthetic) {
        // The tree reads best on true black.
        ctx.fillStyle = '#000000';
        ctx.fillRect(0, 0, cssWidth, cssHeight);
      }

      // Compute every path first so they can share one fit-to-canvas transform —
      // without a common frame the overlay would not line up.
      const paths = parities.map((parity) => computeCoralPath(parity, params, rule));
      drawCoral(ctx, cssWidth, cssHeight, paths, parities, opacity, scale, aesthetic);
    };

    draw();
    let observer: ResizeObserver | undefined;
    if (typeof ResizeObserver !== 'undefined') {
      observer = new ResizeObserver(() => draw());
      observer.observe(canvas);
    }
    return () => observer?.disconnect();
  }, [parities, params, rule, opacity, scale, height]);

  if (trajectories.length === 0) {
    return (
      <div
        className="flex items-center justify-center rounded-lg bg-slate-950/50 text-slate-500"
        style={{ height }}
      >
        Run a trajectory to draw the coral.
      </div>
    );
  }

  if (parities.length === 0) {
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
      aria-label={`Coral visualization of ${trajectories.length} trajector${
        trajectories.length === 1 ? 'y' : 'ies'
      }`}
    />
  );
}

/** Draws all paths under one shared fit-to-canvas transform. */
function drawCoral(
  ctx: CanvasRenderingContext2D,
  width: number,
  height: number,
  paths: Point[][],
  parities: number[][],
  opacity: number,
  scale: number,
  aesthetic: boolean,
): void {
  const all = paths.flat();
  if (all.length < 2) {
    return;
  }
  const bounds = pathBounds(all);
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

  if (aesthetic) {
    // Monochrome hairlines: with hundreds overlaid, density does the drawing.
    // Alpha falls off as the count rises so a large overlay does not clip to white.
    const alpha = Math.max(0.05, Math.min(1, opacity / Math.sqrt(paths.length || 1)));
    ctx.strokeStyle = `rgba(255,255,255,${alpha.toFixed(3)})`;
    ctx.lineWidth = 0.7;
    ctx.lineCap = 'round';
    ctx.globalAlpha = 1;
    for (const path of paths) {
      ctx.beginPath();
      path.forEach((p, i) => {
        const px = projX(p.x);
        const py = projY(p.y);
        if (i === 0) {
          ctx.moveTo(px, py);
        } else {
          ctx.lineTo(px, py);
        }
      });
      ctx.stroke();
    }
    return;
  }

  // Analytical modes: per-segment parity colouring, as before.
  ctx.globalAlpha = opacity;
  ctx.lineWidth = 1.2;
  paths.forEach((path, pi) => {
    const parity = parities[pi] ?? [];
    for (let i = 0; i < path.length - 1; i++) {
      const a = path[i];
      const b = path[i + 1];
      if (!a || !b) {
        continue;
      }
      ctx.strokeStyle = parity[i] === 1 ? ODD_COLOR : EVEN_COLOR;
      ctx.beginPath();
      ctx.moveTo(projX(a.x), projY(a.y));
      ctx.lineTo(projX(b.x), projY(b.y));
      ctx.stroke();
    }
  });
  ctx.globalAlpha = 1;
}
