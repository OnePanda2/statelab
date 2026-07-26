/**
 * Dataset Explorer (§6.2).
 *
 * Streams a generated dataset from the host one trajectory at a time, folding each
 * summary row into a fixed-size [`DatasetAggregate`](./aggregate) and keeping only
 * a small bounded window of rows for the table — so the UI tracks arbitrarily large
 * datasets without ever holding the full set in memory (the FROZEN streaming rule).
 */

import { useEffect, useRef, useState } from 'react';
import {
  streamDataset,
  type DatasetGenerator,
  type DatasetSpec,
  type DatasetSummaryRow,
} from '@/lib/invoke';
import {
  accumulate,
  emptyAggregate,
  meanIterations,
  type DatasetAggregate,
} from './aggregate';
import {
  buildDatasetExportMetadata,
  metadataAsComments,
  TRAJECTORY_SCHEMA_VERSION_FOR_EXPORT,
} from '@/modules/export-center/metadata';

/** Max rows retained for the on-screen table (aggregates still count them all). */
const ROW_WINDOW = 200;
/** Flush accumulated state to React this often (rows), to avoid per-row renders. */
const FLUSH_EVERY = 50;

const GENERATORS: { value: DatasetGenerator; label: string }[] = [
  { value: 'range', label: 'Range' },
  { value: 'random', label: 'Random Set' },
  { value: 'primes', label: 'Primes' },
  { value: 'even', label: 'Even' },
  { value: 'odd', label: 'Odd' },
  { value: 'powers-of-two', label: 'Powers of Two' },
  { value: 'csv', label: 'CSV import' },
];

export function DatasetExplorer(): JSX.Element {
  const [generator, setGenerator] = useState<DatasetGenerator>('range');
  const [start, setStart] = useState('1');
  const [end, setEnd] = useState('1000');
  const [count, setCount] = useState('200');
  const [max, setMax] = useState('10000');
  const [seed, setSeed] = useState('42');
  const [csv, setCsv] = useState('3, 27, 97, 871');
  // Mirrors the engine default. User-editable per sweep — lower it when exploring
  // a system that can diverge, so one runaway item cannot stall the whole dataset.
  const [maxIterations, setMaxIterations] = useState('100000');

  const [running, setRunning] = useState(false);
  const [aggregate, setAggregate] = useState<DatasetAggregate>(emptyAggregate());
  const [rows, setRows] = useState<DatasetSummaryRow[]>([]);
  const [error, setError] = useState<string | null>(null);

  const aggRef = useRef<DatasetAggregate>(emptyAggregate());
  const rowsRef = useRef<DatasetSummaryRow[]>([]);
  const sinceFlush = useRef(0);
  const abortRef = useRef<AbortController | null>(null);

  useEffect(() => () => abortRef.current?.abort(), []);

  function buildSpec(): DatasetSpec {
    const mi = Number(maxIterations) || 100_000;
    switch (generator) {
      case 'range':
      case 'even':
      case 'odd':
        return { generator, params: { start, end }, maxIterations: mi };
      case 'random':
        return { generator, params: { count, max, seed }, maxIterations: mi };
      case 'primes':
      case 'powers-of-two':
        return { generator, params: { count }, maxIterations: mi };
      case 'csv':
        return { generator, params: {}, csv, maxIterations: mi };
    }
  }

  async function start_(): Promise<void> {
    setError(null);
    aggRef.current = emptyAggregate();
    rowsRef.current = [];
    sinceFlush.current = 0;
    setAggregate(aggRef.current);
    setRows([]);
    setRunning(true);

    const controller = new AbortController();
    abortRef.current = controller;

    try {
      await streamDataset(
        buildSpec(),
        (row) => {
          accumulate(aggRef.current, row);
          if (rowsRef.current.length < ROW_WINDOW) {
            rowsRef.current.push(row);
          }
          sinceFlush.current += 1;
          if (sinceFlush.current >= FLUSH_EVERY) {
            sinceFlush.current = 0;
            setAggregate({ ...aggRef.current });
            setRows([...rowsRef.current]);
          }
        },
        controller.signal,
      );
    } catch (e) {
      if (!(e instanceof DOMException && e.name === 'AbortError')) {
        setError(e instanceof Error ? e.message : String(e));
      }
    } finally {
      setAggregate({ ...aggRef.current });
      setRows([...rowsRef.current]);
      setRunning(false);
    }
  }

  function stop_(): void {
    abortRef.current?.abort();
  }

  /**
   * Streaming CSV export (§6.4). Re-runs the stream and appends one compact CSV
   * line per trajectory as it arrives — only summary rows are retained, never the
   * trajectories themselves, so the same no-giant-set-in-memory rule holds.
   */
  async function exportCsv(): Promise<void> {
    setError(null);
    setRunning(true);
    const spec = buildSpec();
    const metadata = buildDatasetExportMetadata(
      { type: spec.generator, ...spec.params, ...(spec.csv ? { values: spec.csv } : {}) },
      spec.maxIterations,
      {
        engineVersion: '1.0.0',
        schemaVersion: TRAJECTORY_SCHEMA_VERSION_FOR_EXPORT,
        platform: navigator.platform || 'unknown',
      },
      { module: 'dataset-explorer', format: 'csv' },
    );

    const lines: string[] = [
      metadataAsComments(metadata),
      'initial_state,iteration_count,status,peak_value,stopping_time,total_stopping_time,odd_count,even_count,maximum_bit_length',
    ];

    const controller = new AbortController();
    abortRef.current = controller;
    try {
      await streamDataset(
        spec,
        (row) => {
          lines.push(
            [
              row.initial_state,
              row.iteration_count,
              row.status,
              row.peak_value ?? '',
              row.stopping_time ?? '',
              row.total_stopping_time ?? '',
              row.odd_count ?? '',
              row.even_count ?? '',
              row.maximum_bit_length ?? '',
            ].join(','),
          );
        },
        controller.signal,
      );
      const blob = new Blob([`${lines.join('\n')}\n`], { type: 'text/csv' });
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = url;
      link.download = `statelab-dataset-${spec.generator}.csv`;
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      URL.revokeObjectURL(url);
    } catch (e) {
      if (!(e instanceof DOMException && e.name === 'AbortError')) {
        setError(e instanceof Error ? e.message : String(e));
      }
    } finally {
      setRunning(false);
    }
  }

  const showRange = generator === 'range' || generator === 'even' || generator === 'odd';
  const showCount = generator === 'primes' || generator === 'powers-of-two' || generator === 'random';

  return (
    <div className="rounded-xl border border-slate-700/60 bg-slate-900/40 p-4">
      <div className="mb-3 flex flex-wrap items-end gap-x-4 gap-y-3">
        <Field label="Generator">
          <select
            className="rounded-lg border border-slate-700 bg-slate-800 px-2 py-1.5 text-sm outline-none focus:border-sky-500"
            value={generator}
            onChange={(e) => setGenerator(e.target.value as DatasetGenerator)}
            disabled={running}
          >
            {GENERATORS.map((g) => (
              <option key={g.value} value={g.value}>
                {g.label}
              </option>
            ))}
          </select>
        </Field>

        {showRange && (
          <>
            <NumberField label="Start" value={start} onChange={setStart} disabled={running} />
            <NumberField label="End" value={end} onChange={setEnd} disabled={running} />
          </>
        )}
        {showCount && (
          <NumberField label="Count" value={count} onChange={setCount} disabled={running} />
        )}
        {generator === 'random' && (
          <>
            <NumberField label="Max" value={max} onChange={setMax} disabled={running} />
            <NumberField label="Seed" value={seed} onChange={setSeed} disabled={running} />
          </>
        )}
        {generator === 'csv' && (
          <Field label="Values (comma / space / newline separated)">
            <input
              className="w-80 rounded-lg border border-slate-700 bg-slate-800 px-3 py-1.5 font-mono text-sm outline-none focus:border-sky-500"
              value={csv}
              onChange={(e) => setCsv(e.target.value)}
              disabled={running}
            />
          </Field>
        )}
        <NumberField
          label="Iter. limit"
          value={maxIterations}
          onChange={setMaxIterations}
          disabled={running}
        />

        {running ? (
          <button
            className="rounded-lg border border-slate-600 px-4 py-2 text-sm font-medium hover:bg-slate-800"
            onClick={stop_}
          >
            Stop
          </button>
        ) : (
          <>
            <button
              className="rounded-lg bg-sky-500 px-4 py-2 text-sm font-semibold text-slate-950 hover:brightness-110"
              onClick={() => void start_()}
            >
              Run dataset
            </button>
            <button
              className="rounded-lg border border-slate-600 bg-slate-800 px-4 py-2 text-sm font-medium text-slate-100 hover:bg-slate-700"
              onClick={() => void exportCsv()}
              data-export="dataset-csv"
            >
              Export CSV (streaming)
            </button>
          </>
        )}
      </div>

      {error && <p className="mb-3 text-red-400">Dataset failed: {error}</p>}

      <div className="mb-3 flex flex-wrap gap-x-6 gap-y-2 text-sm">
        <Stat label="Processed">
          {aggregate.count.toLocaleString()}
          {running && <span className="ml-1 animate-pulse text-slate-500">…</span>}
        </Stat>
        <Stat label="Converged">{aggregate.converged.toLocaleString()}</Stat>
        <Stat label="Cycle">{aggregate.cycleDetected.toLocaleString()}</Stat>
        <Stat label="Iter. limit">{aggregate.iterationLimit.toLocaleString()}</Stat>
        <Stat label="Errors">{aggregate.systemError.toLocaleString()}</Stat>
        <Stat label="Mean iters">{meanIterations(aggregate).toFixed(2)}</Stat>
        <Stat label="Max iters">
          {aggregate.maxIterations.toLocaleString()}
          {aggregate.longestInitialState && (
            <span className="ml-1 text-xs text-slate-500">@ {truncate(aggregate.longestInitialState)}</span>
          )}
        </Stat>
        <Stat label="Largest peak">{truncate(aggregate.largestPeak)}</Stat>
      </div>

      <div className="max-h-80 overflow-auto rounded-lg border border-slate-700/50">
        <table className="w-full border-collapse text-xs">
          <thead className="sticky top-0 bg-slate-900 text-slate-400">
            <tr>
              <Th>Initial</Th>
              <Th>Iterations</Th>
              <Th>Status</Th>
              <Th>Peak</Th>
              <Th>Total stopping</Th>
            </tr>
          </thead>
          <tbody className="font-mono">
            {rows.map((r, i) => (
              <tr key={`${r.initial_state}-${i}`} className="border-t border-slate-800">
                <Td>{truncate(r.initial_state)}</Td>
                <Td>{r.iteration_count}</Td>
                <Td>{r.status}</Td>
                <Td>{r.peak_value !== null ? truncate(r.peak_value) : 'N/A'}</Td>
                <Td>{r.total_stopping_time ?? 'N/A'}</Td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="mt-2 text-xs text-slate-500">
        Streaming: each trajectory is summarized and released — the full set is never held in memory.
        Showing the first {ROW_WINDOW} rows; aggregates cover every processed item.
      </p>
    </div>
  );
}

function truncate(s: string): string {
  return s.length > 18 ? `${s.slice(0, 15)}…` : s;
}

function Field({ label, children }: { label: string; children: React.ReactNode }): JSX.Element {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-xs uppercase tracking-wide text-slate-400">{label}</span>
      {children}
    </label>
  );
}

function NumberField({
  label,
  value,
  onChange,
  disabled,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  disabled?: boolean;
}): JSX.Element {
  return (
    <Field label={label}>
      <input
        className="w-24 rounded-lg border border-slate-700 bg-slate-800 px-2 py-1.5 font-mono text-sm outline-none focus:border-sky-500"
        value={value}
        inputMode="numeric"
        onChange={(e) => onChange(e.target.value)}
        disabled={disabled}
      />
    </Field>
  );
}

function Stat({ label, children }: { label: string; children: React.ReactNode }): JSX.Element {
  return (
    <div>
      <div className="text-xs text-slate-400">{label}</div>
      <div className="font-semibold tabular-nums">{children}</div>
    </div>
  );
}

function Th({ children }: { children: React.ReactNode }): JSX.Element {
  return <th className="px-2 py-1.5 text-left font-medium">{children}</th>;
}

function Td({ children }: { children: React.ReactNode }): JSX.Element {
  return <td className="px-2 py-1 text-slate-300">{children}</td>;
}
