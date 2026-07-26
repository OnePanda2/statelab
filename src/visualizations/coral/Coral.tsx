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
  /** Stroke width in CSS pixels for the analytical modes. */
  lineWidth?: number;
  /** Stroke colour for segments following an odd-parity transition. */
  oddColor?: string;
  /** Stroke colour for segments following an even-parity transition. */
  evenColor?: string;
  /** Pixel shift of the drawing away from dead-centre. */
  offsetX?: number;
  offsetY?: number;
  /**
   * Segments revealed per second. `null` (the default) means **instant** — the
   * whole path is drawn in one frame, exactly as before this control existed.
   */
  animationSpeed?: number | null;
  /** Bumping this restarts an in-progress animation. */
  animationNonce?: number;
  height?: number;
}

/** Defaults preserve the pre-existing hardcoded appearance exactly. */
export const DEFAULT_ODD_COLOR = '#f0883e'; // orange — odd-parity transition
export const DEFAULT_EVEN_COLOR = '#3fb950'; // green — even-parity transition
/**
 * IMPLEMENTATION DECISION (§5.5): one exposed "Line Width" drives both draw
 * modes, which previously hardcoded 1.2 (analytical) and 0.7 (aesthetic). The
 * slider is calibrated in analytical pixels and the aesthetic mode uses
 * `width * AESTHETIC_WIDTH_RATIO`, preserving the original 0.7/1.2 relationship.
 * So the default below reproduces both prior constants exactly, and moving the
 * slider scales both modes together.
 */
export const DEFAULT_LINE_WIDTH = 1.2;
const AESTHETIC_WIDTH_RATIO = 0.7 / 1.2;

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
  lineWidth = DEFAULT_LINE_WIDTH,
  oddColor = DEFAULT_ODD_COLOR,
  evenColor = DEFAULT_EVEN_COLOR,
  offsetX = 0,
  offsetY = 0,
  animationSpeed = null,
  animationNonce = 0,
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
    // Computed once per parameter change and shared by every frame: all paths
    // need one common fit-to-canvas transform or the overlay would not line up,
    // and re-deriving them per animation frame would be wasteful.
    const paths = parities.map((parity) => computeCoralPath(parity, params, rule));
    const longest = paths.reduce((max, p) => Math.max(max, p.length), 0);

    /** Draws the first `revealed` segments of every path (Infinity = all). */
    const draw = (revealed: number): void => {
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

      drawCoral(ctx, cssWidth, cssHeight, paths, parities, {
        opacity,
        scale,
        aesthetic,
        lineWidth,
        oddColor,
        evenColor,
        offsetX,
        offsetY,
        revealed,
      });
    };

    // BACKWARD COMPATIBILITY: with no animation speed set (the default) this is
    // the original single, instant, full-path render — same call, same output,
    // no rAF loop. The progressive reveal only engages once the user asks for it.
    if (animationSpeed === null || animationSpeed <= 0) {
      draw(Number.POSITIVE_INFINITY);
      let observer: ResizeObserver | undefined;
      if (typeof ResizeObserver !== 'undefined') {
        observer = new ResizeObserver(() => draw(Number.POSITIVE_INFINITY));
        observer.observe(canvas);
      }
      return () => observer?.disconnect();
    }

    // Progressive reveal: grow the drawn prefix at `animationSpeed` segments per
    // second until the longest path is fully drawn, then stop.
    let frame = 0;
    const startedAt = performance.now();
    const step = (now: number): void => {
      const revealed = ((now - startedAt) / 1000) * animationSpeed;
      draw(revealed);
      if (revealed < longest) {
        frame = requestAnimationFrame(step);
      }
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  }, [
    parities,
    params,
    rule,
    opacity,
    scale,
    height,
    lineWidth,
    oddColor,
    evenColor,
    offsetX,
    offsetY,
    animationSpeed,
    animationNonce,
  ]);

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

/** Everything the renderer needs beyond the geometry itself. */
interface DrawOptions {
  opacity: number;
  scale: number;
  aesthetic: boolean;
  lineWidth: number;
  oddColor: string;
  evenColor: string;
  offsetX: number;
  offsetY: number;
  /** How many leading segments of each path to draw; `Infinity` draws all. */
  revealed: number;
}

/** Draws all paths under one shared fit-to-canvas transform. */
function drawCoral(
  ctx: CanvasRenderingContext2D,
  width: number,
  height: number,
  paths: Point[][],
  parities: number[][],
  options: DrawOptions,
): void {
  const { opacity, scale, aesthetic, lineWidth, oddColor, evenColor, revealed } = options;
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
  // Centre offset is applied AFTER the auto-fit maths, so it shifts the finished
  // drawing without disturbing the fit-to-canvas scaling. (0, 0) is therefore
  // pixel-identical to the behaviour before this parameter existed.
  const ox = width / 2 + options.offsetX;
  const oy = height / 2 + options.offsetY;
  /** Number of segments to draw from a path of `len` points. */
  const limit = (len: number): number =>
    Number.isFinite(revealed) ? Math.max(0, Math.min(len - 1, Math.floor(revealed))) : len - 1;

  // Canvas y grows downward, so flip y to keep the turtle's +y pointing up.
  const projX = (x: number): number => ox + (x - cx) * s;
  const projY = (y: number): number => oy - (y - cy) * s;

  if (aesthetic) {
    // Monochrome hairlines: with hundreds overlaid, density does the drawing.
    // Alpha falls off as the count rises so a large overlay does not clip to white.
    const alpha = Math.max(0.05, Math.min(1, opacity / Math.sqrt(paths.length || 1)));
    ctx.strokeStyle = `rgba(255,255,255,${alpha.toFixed(3)})`;
    // Rounded to 3 dp: the bare product is subject to float noise
    // (1.2 * 0.7/1.2 = 0.6999999999999999), and a clean width is both easier to
    // reason about and exactly reproduces the previous hardcoded 0.7.
    ctx.lineWidth = Math.round(lineWidth * AESTHETIC_WIDTH_RATIO * 1000) / 1000;
    ctx.lineCap = 'round';
    ctx.globalAlpha = 1;
    for (const path of paths) {
      const count = limit(path.length);
      if (count < 1) {
        continue;
      }
      ctx.beginPath();
      for (let i = 0; i <= count; i++) {
        const p = path[i];
        if (!p) {
          continue;
        }
        const px = projX(p.x);
        const py = projY(p.y);
        if (i === 0) {
          ctx.moveTo(px, py);
        } else {
          ctx.lineTo(px, py);
        }
      }
      ctx.stroke();
    }
    return;
  }

  // Analytical modes: per-segment parity colouring, as before.
  ctx.globalAlpha = opacity;
  ctx.lineWidth = lineWidth;
  paths.forEach((path, pi) => {
    const parity = parities[pi] ?? [];
    const count = limit(path.length);
    for (let i = 0; i < count; i++) {
      const a = path[i];
      const b = path[i + 1];
      if (!a || !b) {
        continue;
      }
      ctx.strokeStyle = parity[i] === 1 ? oddColor : evenColor;
      ctx.beginPath();
      ctx.moveTo(projX(a.x), projY(a.y));
      ctx.lineTo(projX(b.x), projY(b.y));
      ctx.stroke();
    }
  });
  ctx.globalAlpha = 1;
}
