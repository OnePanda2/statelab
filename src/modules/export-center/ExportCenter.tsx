/**
 * Export Center (§6.4).
 *
 * Exports the current Trajectory Object in the four FROZEN formats — **PNG, SVG,
 * CSV, JSON** — each embedding the full FROZEN metadata block. A pure consumer:
 * it serializes what the engine already produced and never recomputes a value.
 *
 * The PNG is rasterized from an **offscreen** canvas created at export time, and
 * the SVG is generated at export time only (§5.6) — neither is used for the
 * interactive views.
 */

import { useState } from 'react';
import type { Trajectory } from '@/types/trajectory';
import { drawTrajectory, trajectoryToValues } from '@/visualizations/chartCanvas';
import { buildExportMetadata, type RenderingParameters } from './metadata';
import { dataUrlToBytes, embedPngMetadata, toCsv, toJson, toSvg } from './exporters';

export interface ExportCenterProps {
  trajectory: Trajectory;
}

const PNG_WIDTH = 900;
const PNG_HEIGHT = 360;

export function ExportCenter({ trajectory }: ExportCenterProps): JSX.Element {
  const [logScale, setLogScale] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  function renderingParameters(format: string): RenderingParameters {
    return {
      chart: 'value',
      scale: logScale ? 'log' : 'linear',
      format,
      width: PNG_WIDTH,
      height: PNG_HEIGHT,
    };
  }

  function metadataFor(format: string): ReturnType<typeof buildExportMetadata> {
    return buildExportMetadata(
      trajectory,
      { type: 'single', initial_state: trajectory.initial_state },
      renderingParameters(format),
    );
  }

  function baseName(ext: string): string {
    return `statelab-${trajectory.system_id}-n${trajectory.initial_state}.${ext}`;
  }

  function exportJson(): void {
    downloadText(toJson(trajectory, metadataFor('json')), baseName('json'), 'application/json');
    setStatus('Exported JSON');
  }

  function exportCsv(): void {
    downloadText(toCsv(trajectory, metadataFor('csv')), baseName('csv'), 'text/csv');
    setStatus('Exported CSV');
  }

  function exportSvg(): void {
    const svg = toSvg(trajectory, metadataFor('svg'), {
      width: PNG_WIDTH,
      height: PNG_HEIGHT,
      logScale,
    });
    downloadText(svg, baseName('svg'), 'image/svg+xml');
    setStatus('Exported SVG');
  }

  function exportPng(): void {
    // Offscreen canvas: the export never depends on what is on screen.
    const canvas = document.createElement('canvas');
    canvas.width = PNG_WIDTH;
    canvas.height = PNG_HEIGHT;
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      setStatus('PNG export unavailable: no 2D context');
      return;
    }
    ctx.fillStyle = '#0e1116';
    ctx.fillRect(0, 0, PNG_WIDTH, PNG_HEIGHT);
    drawTrajectory(
      ctx,
      PNG_WIDTH,
      PNG_HEIGHT,
      trajectoryToValues(trajectory),
      logScale ? 'log' : 'linear',
    );

    try {
      const bytes = dataUrlToBytes(canvas.toDataURL('image/png'));
      const withMetadata = embedPngMetadata(bytes, metadataFor('png'));
      downloadBytes(withMetadata, baseName('png'), 'image/png');
      setStatus('Exported PNG (metadata in tEXt chunk)');
    } catch (e) {
      setStatus(`PNG export failed: ${e instanceof Error ? e.message : String(e)}`);
    }
  }

  return (
    <div className="rounded-xl border border-slate-700/60 bg-slate-900/40 p-4">
      <div className="flex flex-wrap items-center gap-3">
        <button className={BTN} onClick={exportPng} data-export="png">
          PNG
        </button>
        <button className={BTN} onClick={exportSvg} data-export="svg">
          SVG
        </button>
        <button className={BTN} onClick={exportCsv} data-export="csv">
          CSV
        </button>
        <button className={BTN} onClick={exportJson} data-export="json">
          JSON
        </button>
        <label className="flex items-center gap-2 text-xs text-slate-400">
          <input
            type="checkbox"
            className="accent-sky-500"
            checked={logScale}
            onChange={(e) => setLogScale(e.target.checked)}
          />
          Log scale (PNG / SVG)
        </label>
        {status && <span className="text-xs text-emerald-400">{status}</span>}
      </div>
      <p className="mt-2 text-xs text-slate-500">
        Every export embeds the full metadata block: app / engine / schema / visualization versions,
        iteration limit, cycle detection, dataset definition, rendering parameters, timestamp, and
        platform.
      </p>
    </div>
  );
}

const BTN =
  'rounded-lg border border-slate-600 bg-slate-800 px-4 py-2 text-sm font-semibold text-slate-100 hover:bg-slate-700';

function downloadText(content: string, filename: string, mime: string): void {
  downloadBlob(new Blob([content], { type: mime }), filename);
}

function downloadBytes(bytes: Uint8Array, filename: string, mime: string): void {
  // Copy into a fresh ArrayBuffer so the Blob never aliases a subarray view.
  const buffer = new ArrayBuffer(bytes.byteLength);
  new Uint8Array(buffer).set(bytes);
  downloadBlob(new Blob([buffer], { type: mime }), filename);
}

function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);
}
