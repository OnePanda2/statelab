/**
 * Comparison Lab (§6.3).
 *
 * Compares multiple trajectories: an overlay chart with Raw / Log / Normalized
 * modes, a feature table, and a **cosine-similarity matrix** computed over the
 * **min-max-normalized** feature vectors. Trajectories are produced through a
 * Research Controller (§5.1 — the only layer allowed to trigger computation); this
 * module performs only presentation math (similarity/normalization at the
 * consumer boundary).
 */

import { useMemo, useState } from 'react';
import { ResearchController } from '@/controllers/ResearchController';
import { OverlayChart, type OverlayMode, type OverlaySeries } from './OverlayChart';
import {
  COMPARISON_FEATURES,
  extractFeatureVector,
  minMaxNormalizeSet,
  similarityMatrix,
} from './features';

const PALETTE = ['#58a6ff', '#f0883e', '#3fb950', '#bc8cff', '#f778ba', '#e3b341'];
const MODES: { value: OverlayMode; label: string }[] = [
  { value: 'raw', label: 'Raw' },
  { value: 'log', label: 'Log' },
  { value: 'normalized', label: 'Normalized' },
];

export function ComparisonLab(): JSX.Element {
  const controller = useMemo(() => new ResearchController(), []);
  const [entries, setEntries] = useState<OverlaySeries[]>([]);
  const [value, setValue] = useState('27');
  const [mode, setMode] = useState<OverlayMode>('raw');
  const [featureNorm, setFeatureNorm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function add(): Promise<void> {
    const n = value.trim();
    if (!n) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const trajectory = await controller.run(n);
      setEntries((prev) =>
        prev.some((e) => e.trajectory.initial_state === trajectory.initial_state)
          ? prev
          : [...prev, { trajectory, color: PALETTE[prev.length % PALETTE.length] ?? '#58a6ff' }],
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  function remove(initialState: string): void {
    setEntries((prev) => prev.filter((e) => e.trajectory.initial_state !== initialState));
  }

  const vectors = entries.map((e) => extractFeatureVector(e.trajectory));
  const normalized = minMaxNormalizeSet(vectors);
  // Cosine similarity is computed over the raw feature vectors (§6.3); min-max
  // normalization is a separate capability, shown via the feature-table toggle
  // and the overlay's Normalized mode.
  const matrix = similarityMatrix(vectors);
  const displayVectors = featureNorm ? normalized : vectors;

  return (
    <div className="rounded-xl border border-slate-700/60 bg-slate-900/40 p-4">
      <div className="mb-3 flex flex-wrap items-end gap-3">
        <label className="flex flex-col gap-1">
          <span className="text-xs uppercase tracking-wide text-slate-400">Add trajectory (n)</span>
          <input
            className="w-40 rounded-lg border border-slate-700 bg-slate-800 px-3 py-1.5 font-mono text-sm outline-none focus:border-sky-500"
            value={value}
            inputMode="numeric"
            spellCheck={false}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') void add();
            }}
          />
        </label>
        <button
          className="rounded-lg bg-sky-500 px-4 py-2 text-sm font-semibold text-slate-950 hover:brightness-110 disabled:opacity-60"
          onClick={() => void add()}
          disabled={busy}
        >
          {busy ? 'Running…' : 'Add'}
        </button>
        <div className="flex flex-wrap gap-2">
          {entries.map((e) => (
            <span
              key={e.trajectory.initial_state}
              className="inline-flex items-center gap-1.5 rounded-full border border-slate-700 bg-slate-800 px-2.5 py-1 text-xs"
            >
              <span className="h-2.5 w-2.5 rounded-full" style={{ background: e.color }} />
              n={e.trajectory.initial_state}
              <button
                className="ml-1 text-slate-500 hover:text-slate-200"
                onClick={() => remove(e.trajectory.initial_state)}
                aria-label={`remove ${e.trajectory.initial_state}`}
              >
                ×
              </button>
            </span>
          ))}
        </div>
      </div>

      {error && <p className="mb-3 text-red-400">Failed: {error}</p>}

      {entries.length === 0 ? (
        <p className="text-sm text-slate-500">Add two or more trajectories to compare them.</p>
      ) : (
        <>
          <div className="mb-2 flex items-center gap-2">
            <span className="text-xs uppercase tracking-wide text-slate-400">Overlay</span>
            <div className="inline-flex overflow-hidden rounded-lg border border-slate-700">
              {MODES.map((m) => (
                <button
                  key={m.value}
                  className={`px-3 py-1 text-xs ${
                    mode === m.value ? 'bg-sky-500 text-slate-950' : 'bg-slate-800 text-slate-300'
                  }`}
                  onClick={() => setMode(m.value)}
                >
                  {m.label}
                </button>
              ))}
            </div>
          </div>

          <OverlayChart series={entries} mode={mode} />

          <div className="mt-4 grid grid-cols-1 gap-4 lg:grid-cols-2">
            <FeatureTable
              entries={entries}
              vectors={displayVectors}
              normalized={featureNorm}
              onToggle={() => setFeatureNorm((v) => !v)}
            />
            {entries.length >= 2 && <SimilarityMatrix entries={entries} matrix={matrix} />}
          </div>
        </>
      )}
    </div>
  );
}

function FeatureTable({
  entries,
  vectors,
  normalized,
  onToggle,
}: {
  entries: OverlaySeries[];
  vectors: number[][];
  normalized: boolean;
  onToggle: () => void;
}): JSX.Element {
  return (
    <div>
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-xs uppercase tracking-wide text-slate-400">Feature comparison</h3>
        <button
          className="rounded-lg border border-slate-700 px-2 py-0.5 text-xs text-slate-300 hover:bg-slate-800"
          onClick={onToggle}
        >
          {normalized ? 'Min-max normalized' : 'Raw values'}
        </button>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full border-collapse text-xs">
          <thead className="text-slate-400">
            <tr>
              <th className="px-2 py-1 text-left font-medium">Feature</th>
              {entries.map((e) => (
                <th key={e.trajectory.initial_state} className="px-2 py-1 text-right font-medium">
                  <span className="inline-block h-2 w-2 rounded-full" style={{ background: e.color }} /> n=
                  {e.trajectory.initial_state}
                </th>
              ))}
            </tr>
          </thead>
          <tbody className="font-mono">
            {COMPARISON_FEATURES.map((f, fi) => (
              <tr key={f.key} className="border-t border-slate-800">
                <td className="px-2 py-1 text-slate-400">{f.label}</td>
                {vectors.map((v, ci) => (
                  <td key={ci} className="px-2 py-1 text-right text-slate-200">
                    {formatNumber(v[fi] ?? 0)}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function SimilarityMatrix({
  entries,
  matrix,
}: {
  entries: OverlaySeries[];
  matrix: number[][];
}): JSX.Element {
  return (
    <div>
      <h3 className="mb-2 text-xs uppercase tracking-wide text-slate-400">
        Cosine similarity <span className="text-slate-500">(raw feature vectors)</span>
      </h3>
      <div className="overflow-x-auto">
        <table className="border-collapse text-xs">
          <thead className="text-slate-400">
            <tr>
              <th className="px-2 py-1" />
              {entries.map((e) => (
                <th key={e.trajectory.initial_state} className="px-2 py-1 font-medium">
                  {e.trajectory.initial_state}
                </th>
              ))}
            </tr>
          </thead>
          <tbody className="font-mono">
            {entries.map((rowEntry, r) => (
              <tr key={rowEntry.trajectory.initial_state}>
                <td className="px-2 py-1 text-slate-400">{rowEntry.trajectory.initial_state}</td>
                {entries.map((_, c) => {
                  const sim = matrix[r]?.[c] ?? 0;
                  return (
                    <td
                      key={c}
                      className="px-2 py-1 text-center text-slate-100"
                      style={{ background: `rgba(88,166,255,${Math.max(0, sim).toFixed(3)})` }}
                    >
                      {sim.toFixed(3)}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function formatNumber(v: number): string {
  return Number.isInteger(v) ? String(v) : v.toFixed(4);
}
