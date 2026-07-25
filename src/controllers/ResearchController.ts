/**
 * Research Controller (§5.1).
 *
 * The only layer permitted to trigger a computation. It takes UI input, issues the
 * engine call across the IPC boundary ([`runTrajectory`](@/lib/invoke)), receives
 * the finalized Trajectory Object, and holds UI-session state (which trajectories
 * are loaded). It **never** holds or recomputes trajectory mathematics itself
 * (Principle #4) — it orchestrates only.
 */

import {
  DEFAULT_ENGINE_CONFIG,
  runTrajectory,
  type EngineConfig,
} from '@/lib/invoke';
import type { Trajectory } from '@/types/trajectory';

/** The first (and, for now, only) built-in deterministic system. */
export const CLASSIC_COLLATZ = 'classic-collatz';

export class ResearchController {
  private config: EngineConfig;
  /** Trajectories produced this session, most-recent last. */
  private readonly loaded: Trajectory[] = [];

  constructor(config: EngineConfig = DEFAULT_ENGINE_CONFIG) {
    this.config = config;
  }

  /** The engine configuration currently in effect. */
  getConfig(): EngineConfig {
    return this.config;
  }

  /** Replaces the engine configuration used for subsequent runs. */
  setConfig(config: EngineConfig): void {
    this.config = config;
  }

  /**
   * Runs one trajectory for the given raw initial state and records it in session
   * state. Returns the finalized, immutable Trajectory Object for consumers to
   * render. Does no mathematics of its own.
   */
  async run(initialState: string): Promise<Trajectory> {
    const trajectory = await runTrajectory({
      systemId: CLASSIC_COLLATZ,
      initialState,
      config: this.config,
    });
    this.loaded.push(trajectory);
    return trajectory;
  }

  /** All trajectories loaded this session (immutable copies). */
  getLoaded(): readonly Trajectory[] {
    return this.loaded;
  }
}
