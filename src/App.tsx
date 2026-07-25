/**
 * StateLab — minimal UI shell (Phase 3, §8).
 *
 * Definition of Done for this phase: entering a number produces the correct
 * Trajectory JSON on screen. **No charts yet** (those are Phase 4+). The UI is a
 * pure consumer: it triggers a run through the Research Controller and renders the
 * immutable Trajectory it gets back — it computes nothing itself.
 */

import { useMemo, useState } from 'react';
import { ResearchController } from '@/controllers/ResearchController';
import type { Trajectory } from '@/types/trajectory';
import { ValueChart } from '@/visualizations/value-chart/ValueChart';
import { LogChart } from '@/visualizations/log-chart/LogChart';
import { CoralPanel } from '@/visualizations/coral/CoralPanel';
import { FeatureAnalysis } from '@/modules/feature-analysis/FeatureAnalysis';
import { DatasetExplorer } from '@/modules/dataset-explorer/DatasetExplorer';
import { ComparisonLab } from '@/modules/comparison-lab/ComparisonLab';
import { ExportCenter } from '@/modules/export-center/ExportCenter';

const STATUS_STYLES: Record<string, string> = {
  Converged: 'bg-emerald-500/15 text-emerald-400',
  CycleDetected: 'bg-orange-500/15 text-orange-400',
  IterationLimitReached: 'bg-sky-500/15 text-sky-400',
  SystemError: 'bg-red-500/15 text-red-400',
};

export default function App(): JSX.Element {
  // One controller per session — the only layer allowed to trigger computation.
  const controller = useMemo(() => new ResearchController(), []);
  const [value, setValue] = useState('27');
  const [trajectory, setTrajectory] = useState<Trajectory | null>(null);
  // Every trajectory run this session, for the Coral overlay. Charts, metrics and
  // export always describe the most recent one; the Coral draws them all until
  // the user presses Reset.
  const [coralTrajectories, setCoralTrajectories] = useState<Trajectory[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [coralProgress, setCoralProgress] = useState<number | null>(null);
  const [coralError, setCoralError] = useState<string | null>(null);

  /**
   * Runs a contiguous range and appends it to the Coral overlay. Lives here, not
   * in the visualization: the Research Controller is the only layer permitted to
   * trigger computation (§5.1), and visualizations may not reach it (§5.2).
   */
  async function addCoralRange(from: number, to: number): Promise<void> {
    const count = to - from + 1;
    setCoralError(null);
    setCoralProgress(0);
    const batch: Trajectory[] = [];
    try {
      for (let n = from; n <= to; n++) {
        batch.push(await controller.run(String(n)));
        if (batch.length % 25 === 0) {
          setCoralProgress(Math.round((batch.length / count) * 100));
          // Yield so the progress indicator actually paints during a long run.
          await new Promise((resolve) => setTimeout(resolve, 0));
        }
      }
    } catch (e) {
      setCoralError(e instanceof Error ? e.message : String(e));
    } finally {
      // Keep whatever completed rather than discarding partial work.
      if (batch.length > 0) {
        setCoralTrajectories((prev) => [...prev, ...batch]);
      }
      setCoralProgress(null);
    }
  }

  async function run(): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      const result = await controller.run(value.trim());
      setTrajectory(result);
      setCoralTrajectories((prev) => [...prev, result]);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="min-h-screen bg-[#0e1116] text-slate-100">
      <header className="flex items-baseline gap-3 border-b border-slate-700/60 px-6 py-4">
        <h1 className="text-xl font-semibold tracking-wide">StateLab</h1>
        <span className="text-sm text-slate-400">
          deterministic state-evolution research platform · classic-collatz
        </span>
      </header>

      <main className="mx-auto max-w-4xl px-6 py-6">
        <div className="mb-5 flex flex-wrap items-end gap-3 rounded-xl border border-slate-700/60 bg-slate-900/40 p-4">
          <label className="flex flex-col gap-1">
            <span className="text-xs uppercase tracking-wide text-slate-400">Initial state (n)</span>
            <input
              className="w-64 rounded-lg border border-slate-700 bg-slate-800 px-3 py-2 font-mono text-base outline-none focus:border-sky-500"
              value={value}
              spellCheck={false}
              inputMode="numeric"
              onChange={(e) => setValue(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') void run();
              }}
            />
          </label>
          <button
            className="rounded-lg bg-sky-500 px-4 py-2 font-semibold text-slate-950 hover:brightness-110 disabled:opacity-60"
            onClick={() => void run()}
            disabled={busy}
          >
            {busy ? 'Running…' : 'Run trajectory'}
          </button>
          <span className="text-xs text-slate-400">
            Arbitrary precision — all mathematics runs in the Rust engine.
          </span>
        </div>

        {error && <p className="mb-4 text-red-400">Request failed: {error}</p>}

        {trajectory && (
          <section className="rounded-xl border border-slate-700/60 bg-slate-900/40 p-4">
            <div className="mb-3 flex flex-wrap items-center gap-x-6 gap-y-2">
              <Stat label="Status">
                <span
                  className={`rounded-full px-2.5 py-0.5 text-xs font-semibold ${
                    STATUS_STYLES[trajectory.trajectory_status] ?? 'bg-slate-600/30 text-slate-200'
                  }`}
                >
                  {trajectory.trajectory_status}
                </span>
              </Stat>
              <Stat label="Iterations">{trajectory.iteration_count}</Stat>
              <Stat label="Source">
                {trajectory.execution_metadata.cache_hit ? 'cache hit' : 'computed'}
              </Stat>
              <Stat label="Reason">{trajectory.termination_reason}</Stat>
            </div>

            <div className="mb-4 grid grid-cols-1 gap-4 lg:grid-cols-2">
              <ValueChart trajectory={trajectory} />
              <LogChart trajectory={trajectory} />
            </div>

            <h2 className="mb-2 text-xs uppercase tracking-wide text-slate-400">
              Feature analysis
            </h2>
            <div className="mb-4">
              <FeatureAnalysis trajectory={trajectory} />
            </div>

            <h2 className="mb-2 text-xs uppercase tracking-wide text-slate-400">
              Coral / branch visualization
            </h2>
            <div className="mb-4">
              <CoralPanel
                trajectories={coralTrajectories}
                onReset={() => {
                  setCoralTrajectories([]);
                  setCoralError(null);
                }}
                onAddRange={(from, to) => void addCoralRange(from, to)}
                progress={coralProgress}
                runError={coralError}
              />
            </div>

            <h2 className="mb-2 text-xs uppercase tracking-wide text-slate-400">Export center</h2>
            <div className="mb-4">
              <ExportCenter trajectory={trajectory} />
            </div>

            <details>
              <summary className="cursor-pointer text-xs uppercase tracking-wide text-slate-400">
                Trajectory Object (raw JSON)
              </summary>
              <pre className="mt-2 max-h-[28rem] overflow-auto rounded-lg bg-slate-950/70 p-3 font-mono text-xs leading-relaxed text-slate-200">
                {JSON.stringify(trajectory, null, 2)}
              </pre>
            </details>
          </section>
        )}

        <section className="mt-8">
          <h2 className="mb-2 text-xs uppercase tracking-wide text-slate-400">Comparison lab</h2>
          <ComparisonLab />
        </section>

        <section className="mt-8">
          <h2 className="mb-2 text-xs uppercase tracking-wide text-slate-400">Dataset explorer</h2>
          <DatasetExplorer />
        </section>
      </main>
    </div>
  );
}

function Stat({ label, children }: { label: string; children: React.ReactNode }): JSX.Element {
  return (
    <div>
      <div className="text-xs text-slate-400">{label}</div>
      <div className="text-lg font-bold tabular-nums">{children}</div>
    </div>
  );
}
