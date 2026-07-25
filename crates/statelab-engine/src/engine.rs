//! The State Evolution Engine (§4.1 FROZEN): a generic driver that runs **any**
//! [`DeterministicSystem`] through the fixed Trajectory Generation Order.
//!
//! This module contains **no logic specific to any named system**. It must never
//! import anything from [`crate::systems`] (Part 9 isolation rule — the mirror of
//! the Rust-side `engine.rs` never imports `systems/`).

use std::fmt::Display;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::cycle_detection::CycleTracker;
use crate::system::{DeterministicSystem, InitialStateInput};
use crate::trajectory::{CycleInfo, TerminationDetail, Trajectory};

/// Engine run configuration. Part of the cache key (§4.8), hence `Hash + Eq`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EngineConfig {
    /// Upper bound on transitions before reporting
    /// [`crate::TrajectoryStatus::IterationLimitReached`]. Also bounds cycle-detector memory.
    pub max_iterations: u64,
    /// Maximum number of memoized trajectories retained by the LRU cache (§4.8).
    /// Exposed as configuration, never hardcoded.
    pub cache_max_entries: usize,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_iterations: 100_000,
            cache_max_entries: 1_024,
        }
    }
}

impl EngineConfig {
    /// Convenience constructor for a config with the default cache bound.
    pub fn with_max_iterations(max_iterations: u64) -> Self {
        Self {
            max_iterations,
            ..Self::default()
        }
    }
}

/// The generic driver. Stateless: all run state lives in local variables so runs
/// are independent and reproducible.
pub struct StateEvolutionEngine;

impl StateEvolutionEngine {
    /// Runs `system` from `initial_raw` under `config`, returning a finalized,
    /// immutable [`Trajectory`].
    ///
    /// ## Trajectory Generation Order (§4.1 FROZEN — exact sequence)
    /// 1. Generate next state (`system.transition`)
    /// 2. Check convergence (`system.is_terminated` — the system's own rule)
    /// 3. Check cycle detection (engine-generic, via `states_equal` / `state_hash`)
    /// 4. Check iteration limit (engine-generic)
    /// 5. Continue
    ///
    /// Because step 2 precedes step 3, a system whose own termination rule is met
    /// on a step that *would* also close a cycle reports `Converged`, never
    /// `CycleDetected` (this is why Classic Collatz reaching 1 converges).
    ///
    /// `S::State: Display` is required so states can be serialized into the
    /// Trajectory's BigInt-safe string form (§4.3); the frozen trait itself is not
    /// widened.
    pub fn run<S>(system: &S, initial_raw: &InitialStateInput, config: &EngineConfig) -> Trajectory
    where
        S: DeterministicSystem,
        S::State: Display,
    {
        let started_at = Instant::now();

        let initial_state = match system.validate_initial_state(initial_raw) {
            Ok(state) => state,
            Err(err) => {
                return Trajectory::system_error(
                    system,
                    initial_raw,
                    config,
                    err.message,
                    started_at,
                )
            }
        };

        let mut history = crate::system::TrajectoryHistory::new(initial_state.clone());
        let mut tracker = CycleTracker::new(&initial_state, |s| system.state_hash(s));

        loop {
            // 1. Generate next state.
            let next = system.transition(history.current());
            history.push(next.clone());
            let next_index = history.iteration_count() as usize; // index of `next` in the sequence

            // 2. Convergence — the system's own success condition (checked FIRST).
            if let Some(reason) = system.is_terminated(&next, &history) {
                return Self::finalize(
                    system,
                    &history,
                    TerminationDetail::Converged(reason),
                    config,
                    started_at,
                );
            }

            // 3. Cycle detection — engine-generic, only after convergence was ruled out.
            if let Some(hit) = tracker.check(
                &next,
                next_index,
                |a, b| system.states_equal(a, b),
                |s| system.state_hash(s),
            ) {
                let info = CycleInfo {
                    cycle_start_index: hit.start_index,
                    cycle_length: hit.length,
                    repeated_state: next.to_string(),
                };
                return Self::finalize(
                    system,
                    &history,
                    TerminationDetail::Cycle(info),
                    config,
                    started_at,
                );
            }

            // 4. Iteration limit — engine-generic.
            if history.iteration_count() >= config.max_iterations {
                return Self::finalize(
                    system,
                    &history,
                    TerminationDetail::IterationLimit,
                    config,
                    started_at,
                );
            }

            // 5. Continue.
        }
    }

    /// Feature Extraction stage + Trajectory assembly. After this returns, the
    /// Trajectory Object is immutable (Principle #5).
    fn finalize<S>(
        system: &S,
        history: &crate::system::TrajectoryHistory<S::State>,
        detail: TerminationDetail,
        config: &EngineConfig,
        started_at: Instant,
    ) -> Trajectory
    where
        S: DeterministicSystem,
        S::State: Display,
    {
        let raw = history.as_raw();
        let metrics = system.extract_features(&raw); // metrics computed exactly once, here
        Trajectory::assemble(
            system,
            history.states(),
            detail,
            metrics,
            config,
            started_at,
        )
    }
}
