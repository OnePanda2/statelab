/**
 * StateLab — minimal UI shell (Phase 3, §8).
 *
 * Definition of Done for this phase: entering a number produces the correct
 * Trajectory JSON on screen. **No charts yet** (those are Phase 4+). The UI is a
 * pure consumer: it triggers a run through the Research Controller and renders the
 * immutable Trajectory it gets back — it computes nothing itself.
 */

import { useEffect, useMemo, useState } from 'react';
import { ResearchController } from '@/controllers/ResearchController';
import { listSystems, type SystemDescriptor } from '@/lib/invoke';
import { useTheme } from '@/lib/theme';
import { ThemeToggle } from '@/components/ThemeToggle';
import type { Trajectory } from '@/types/trajectory';
import { ValueChart } from '@/visualizations/value-chart/ValueChart';
import { LogChart } from '@/visualizations/log-chart/LogChart';
import { CoralPanel } from '@/visualizations/coral/CoralPanel';
import { FeatureAnalysis } from '@/modules/feature-analysis/FeatureAnalysis';
import { DatasetExplorer } from '@/modules/dataset-explorer/DatasetExplorer';
import { ComparisonLab } from '@/modules/comparison-lab/ComparisonLab';
import { ExportCenter } from '@/modules/export-center/ExportCenter';

/** Terminal status → design-system pill variant. */
const STATUS_PILL: Record<string, string> = {
  Converged: 'sl-pill sl-pill--success',
  CycleDetected: 'sl-pill sl-pill--warning',
  IterationLimitReached: 'sl-pill sl-pill--accent',
  SystemError: 'sl-pill sl-pill--danger',
};

export default function App(): JSX.Element {
  // One controller per session — the only layer allowed to trigger computation.
  const controller = useMemo(() => new ResearchController(), []);
  const { theme, toggleTheme } = useTheme();
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
  // Systems come from the host's own registry, so the picker cannot drift from
  // what the engine can actually run.
  const [systems, setSystems] = useState<SystemDescriptor[]>([]);
  const [systemId, setSystemId] = useState('classic-collatz');

  useEffect(() => {
    listSystems()
      .then(setSystems)
      .catch(() => {
        // A host too old to expose /api/systems still runs Classic Collatz.
        setSystems([{ id: 'classic-collatz', label: 'Classic Collatz (3n+1)' }]);
      });
  }, []);

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
      controller.setSystemId(systemId);
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
    <div className="min-h-screen">
      <main className="mx-auto max-w-5xl px-8 py-10">
        {/* Toolbar: floats above the workspace, holds identity, the run
            controls' context and the theme toggle. Deliberately sparse. */}
        <header className="sl-toolbar mb-8">
          <h1 className="text-[length:var(--sl-text-xl)] font-semibold tracking-tight">StateLab</h1>
          <span className="sl-pill sl-pill--neutral">{systemId}</span>
          <span className="flex-1" />
          <span className="sl-hint hidden md:inline">
            deterministic state-evolution research platform
          </span>
          <ThemeToggle theme={theme} onToggle={toggleTheme} />
        </header>

        <div className="sl-panel mb-8 flex flex-wrap items-end gap-4">
          <label className="sl-field">
            <span className="sl-label">System</span>
            <select
              className="sl-select"
              value={systemId}
              onChange={(e) => setSystemId(e.target.value)}
              data-testid="system-select"
            >
              {systems.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.label}
                </option>
              ))}
            </select>
          </label>
          <label className="sl-field">
            <span className="sl-label">Initial state (n)</span>
            <input
              className="sl-input sl-input--mono w-64"
              value={value}
              spellCheck={false}
              inputMode="numeric"
              onChange={(e) => setValue(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') void run();
              }}
            />
          </label>
          <button className="sl-btn sl-btn--primary" onClick={() => void run()} disabled={busy}>
            {busy ? 'Running…' : 'Run trajectory'}
          </button>
          <span className="sl-hint max-w-[15rem]">
            Arbitrary precision — all mathematics runs in the Rust engine.
          </span>
        </div>

        {error && <p className="sl-error mb-6">Request failed: {error}</p>}

        {trajectory && (
          <section className="sl-panel">
            <div className="mb-6 flex flex-wrap items-center gap-x-10 gap-y-4">
              <Stat label="Status">
                <span className={STATUS_PILL[trajectory.trajectory_status] ?? 'sl-pill sl-pill--neutral'}>
                  {trajectory.trajectory_status}
                </span>
              </Stat>
              <Stat label="Iterations">{trajectory.iteration_count}</Stat>
              <Stat label="Source">
                {trajectory.execution_metadata.cache_hit ? 'cache hit' : 'computed'}
              </Stat>
              <Stat label="Reason">{trajectory.termination_reason}</Stat>
            </div>

            <div className="mb-8 grid grid-cols-1 gap-6 lg:grid-cols-2">
              <ValueChart trajectory={trajectory} />
              <LogChart trajectory={trajectory} />
            </div>

            <h2 className="sl-section-title">Feature analysis</h2>
            <div className="mb-8">
              <FeatureAnalysis trajectory={trajectory} />
            </div>

            <h2 className="sl-section-title">Coral / branch visualization</h2>
            <div className="mb-8">
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

            <h2 className="sl-section-title">Export center</h2>
            <div className="mb-8">
              <ExportCenter trajectory={trajectory} />
            </div>

            <details>
              <summary className="sl-section-title mb-0 cursor-pointer select-none">
                Trajectory Object (raw JSON)
              </summary>
              <pre className="sl-code sl-scroll mt-4 max-h-[28rem]">
                {JSON.stringify(trajectory, null, 2)}
              </pre>
            </details>
          </section>
        )}

        <section className="mt-10">
          <h2 className="sl-section-title">Comparison lab</h2>
          <ComparisonLab />
        </section>

        <section className="mt-10">
          <h2 className="sl-section-title">Dataset explorer</h2>
          <DatasetExplorer />
        </section>
      </main>
    </div>
  );
}

function Stat({ label, children }: { label: string; children: React.ReactNode }): JSX.Element {
  return (
    <div>
      <div className="sl-stat__label">{label}</div>
      <div className="sl-stat__value">{children}</div>
    </div>
  );
}
