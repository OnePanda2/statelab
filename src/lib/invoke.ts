/**
 * The IPC boundary (§3.2). The frontend never computes a trajectory field itself —
 * it asks the engine and renders what comes back.
 *
 * This is the **single seam** between the frontend and the engine host, and it
 * supports two transports:
 *
 * - **Tauri** (`src-tauri`) — `invoke('run_trajectory', …)` over the native IPC
 *   bridge, with dataset rows streamed through a `Channel`.
 * - **Local server** (`crates/statelab-app`) — `fetch` to `/api/*`, with dataset
 *   rows streamed as NDJSON.
 *
 * The transport is detected at runtime, so one build runs in either host. Nothing
 * upstream (the Research Controller, the UI) knows or cares which is in use.
 */

import type { Trajectory } from '@/types/trajectory';

/** True when running inside the Tauri shell rather than a plain browser. */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/** Mirror of the Rust `EngineConfig` (§4.1 / §4.8). */
export interface EngineConfig {
  maxIterations: number;
  cacheMaxEntries: number;
}

/** Arguments for a single engine run — the same shape the Tauri command takes. */
export interface RunTrajectoryArgs {
  systemId: string;
  initialState: string;
  config: EngineConfig;
}

/**
 * Default engine configuration. **Must mirror the Rust `EngineConfig::default()`**
 * — a mismatch would silently make the frontend request a different bound than
 * the engine's own default.
 */
export const DEFAULT_ENGINE_CONFIG: EngineConfig = {
  maxIterations: 10_000_000,
  cacheMaxEntries: 1_024,
};

/**
 * Runs one trajectory through the engine host and returns the finalized,
 * immutable Trajectory Object. Throws only on transport failure; an invalid
 * `initialState` comes back as a well-formed `SystemError` trajectory, not an
 * exception.
 */
/** A deterministic system the engine can run, as reported by the host. */
export interface SystemDescriptor {
  id: string;
  label: string;
}

/**
 * Systems available in this build. Fetched from the host rather than hardcoded,
 * so the picker can never drift from the engine's own registry.
 */
export async function listSystems(): Promise<SystemDescriptor[]> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<SystemDescriptor[]>('list_systems');
  }
  const response = await fetch('/api/systems');
  if (!response.ok) {
    throw new Error(`engine host returned HTTP ${response.status}`);
  }
  return (await response.json()) as SystemDescriptor[];
}

export async function runTrajectory(args: RunTrajectoryArgs): Promise<Trajectory> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<Trajectory>('run_trajectory', {
      systemId: args.systemId,
      initialState: args.initialState,
      config: {
        maxIterations: args.config.maxIterations,
        cacheMaxEntries: args.config.cacheMaxEntries,
      },
    });
  }

  const params = new URLSearchParams({
    systemId: args.systemId,
    initialState: args.initialState,
    maxIterations: String(args.config.maxIterations),
  });

  const response = await fetch(`/api/run?${params.toString()}`);
  // The host reports an unknown system_id as an error object rather than a
  // Trajectory — surface it instead of letting a malformed object reach the UI.
  if (!response.ok) {
    throw new Error(`engine host returned HTTP ${response.status}`);
  }
  const payload = (await response.json()) as Trajectory | { error: string };
  if ('error' in payload) {
    throw new Error(payload.error);
  }
  return payload;
}

// ---- Dataset streaming (§6.2) ----

/** The 7 FROZEN dataset generators. */
export type DatasetGenerator =
  | 'range'
  | 'random'
  | 'primes'
  | 'even'
  | 'odd'
  | 'powers-of-two'
  | 'csv';

/** A dataset request: a generator, its params, and (for CSV) the pasted values. */
export interface DatasetSpec {
  generator: DatasetGenerator;
  params: Record<string, string>;
  csv?: string;
  maxIterations: number;
}

/** One streamed per-trajectory summary row (never the full state sequence). */
export interface DatasetSummaryRow {
  initial_state: string;
  iteration_count: number;
  status: Trajectory['trajectory_status'];
  peak_value: string | null;
  stopping_time: number | null;
  total_stopping_time: number | null;
  odd_count: number | null;
  even_count: number | null;
  maximum_bit_length: number | null;
}

/**
 * Streams a dataset from the host, invoking `onRow` for each summary row as it
 * arrives. The response is NDJSON delimited by connection close; rows are parsed
 * incrementally and handed off immediately — this client never accumulates the
 * full set (the FROZEN streaming rule, §6.2). Abort via `signal`.
 */
export async function streamDataset(
  spec: DatasetSpec,
  onRow: (row: DatasetSummaryRow) => void,
  signal?: AbortSignal,
): Promise<void> {
  if (isTauri()) {
    return streamDatasetViaTauri(spec, onRow, signal);
  }

  const params = new URLSearchParams({
    type: spec.generator,
    maxIterations: String(spec.maxIterations),
    ...spec.params,
  });
  const isCsv = spec.generator === 'csv';
  const response = await fetch(
    `/api/dataset?${params.toString()}`,
    isCsv ? { method: 'POST', body: spec.csv ?? '', signal } : { signal },
  );
  if (!response.ok || !response.body) {
    throw new Error(`dataset host returned HTTP ${response.status}`);
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  for (;;) {
    const { done, value } = await reader.read();
    if (done) {
      break;
    }
    buffer += decoder.decode(value, { stream: true });
    let newline = buffer.indexOf('\n');
    while (newline >= 0) {
      const line = buffer.slice(0, newline).trim();
      buffer = buffer.slice(newline + 1);
      if (line) {
        onRow(JSON.parse(line) as DatasetSummaryRow);
      }
      newline = buffer.indexOf('\n');
    }
  }
  const tail = buffer.trim();
  if (tail) {
    onRow(JSON.parse(tail) as DatasetSummaryRow);
  }
}

/**
 * Converts the UI's string-valued params into the typed `DatasetSpec` the Rust
 * command deserializes (internally tagged by `type`).
 */
function toTauriSpec(spec: DatasetSpec): Record<string, unknown> {
  const n = (key: string, fallback: number): number => {
    const parsed = Number(spec.params[key]);
    return Number.isFinite(parsed) ? parsed : fallback;
  };
  switch (spec.generator) {
    case 'range':
    case 'even':
    case 'odd':
      return { type: spec.generator, start: n('start', 1), end: n('end', 100) };
    case 'random':
      return {
        type: 'random',
        count: n('count', 100),
        max: Math.max(1, n('max', 10_000)),
        seed: n('seed', 42),
      };
    case 'primes':
      return { type: 'primes', count: n('count', 100) };
    case 'powers-of-two':
      return { type: 'powers-of-two', count: n('count', 32) };
    case 'csv':
      return {
        type: 'csv',
        values: (spec.csv ?? '')
          .split(/[,\s]+/)
          .map((v) => v.trim())
          .filter((v) => v.length > 0),
      };
  }
}

/** Tauri transport: rows arrive over an IPC `Channel`, one per trajectory. */
async function streamDatasetViaTauri(
  spec: DatasetSpec,
  onRow: (row: DatasetSummaryRow) => void,
  signal?: AbortSignal,
): Promise<void> {
  const { invoke, Channel } = await import('@tauri-apps/api/core');

  const channel = new Channel<DatasetSummaryRow>();
  let aborted = false;
  channel.onmessage = (row): void => {
    if (!aborted) {
      onRow(row);
    }
  };
  signal?.addEventListener('abort', () => {
    aborted = true;
  });

  await invoke('run_dataset', {
    spec: toTauriSpec(spec),
    maxIterations: spec.maxIterations,
    onRow: channel,
  });
}
