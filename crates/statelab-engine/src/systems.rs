//! Built-in deterministic systems.
//!
//! Each submodule implements [`crate::system::DeterministicSystem`] for exactly
//! one system. The engine never references anything in here — adding a system is
//! purely additive (Principle #6).
//!
//! `bigint_metrics` is shared infrastructure rather than a system: every metric
//! in §4.4 is derived from a `BigUint` state sequence and its parity bits, so any
//! parity-driven big-integer system reuses one implementation instead of copying
//! it.

pub mod bigint_metrics;
pub mod collatz;
pub mod five_n_plus_one;

use crate::cache::TrajectoryCache;
use crate::engine::{EngineConfig, StateEvolutionEngine};
use crate::system::InitialStateInput;
use crate::trajectory::Trajectory;

/// A registered system: its stable id and a human-readable label for pickers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SystemInfo {
    pub id: &'static str,
    pub label: &'static str,
}

/// Every built-in system, for host UIs to enumerate.
///
/// IMPLEMENTATION DECISION (§4.2): dispatch-by-id lives here, beside the systems,
/// rather than in `engine.rs`. The frozen spec requires the *engine* to know
/// nothing about any named system (§2.2), and this preserves that exactly —
/// `engine.rs` is untouched and still only ever sees a generic
/// `impl DeterministicSystem`. Hosts need *some* id→system mapping to serve a
/// user-chosen system; centralising it here means adding a system touches one
/// list instead of every host.
pub const AVAILABLE_SYSTEMS: &[SystemInfo] = &[
    SystemInfo {
        id: "classic-collatz",
        label: "Classic Collatz (3n+1)",
    },
    SystemInfo {
        id: "five-n-plus-one",
        label: "5n+1",
    },
];

/// Whether `system_id` names a registered system.
pub fn is_known_system(system_id: &str) -> bool {
    AVAILABLE_SYSTEMS.iter().any(|s| s.id == system_id)
}

/// Runs `system_id` through the engine, returning `None` for an unknown id.
///
/// Returning `None` rather than silently substituting a default is deliberate:
/// quietly running a different system than the caller asked for would violate
/// Principle #4 (the engine is the single source of truth for what was computed).
pub fn run_by_id(
    system_id: &str,
    raw: &InitialStateInput,
    config: &EngineConfig,
) -> Option<Trajectory> {
    match system_id {
        "classic-collatz" => Some(StateEvolutionEngine::run(
            &collatz::ClassicCollatz,
            raw,
            config,
        )),
        "five-n-plus-one" => Some(StateEvolutionEngine::run(
            &five_n_plus_one::FiveNPlusOne,
            raw,
            config,
        )),
        _ => None,
    }
}

/// Cache-aware variant of [`run_by_id`]. The cache key already includes
/// `system_id` (§4.8 FROZEN), so entries for different systems cannot collide.
pub fn run_by_id_cached(
    system_id: &str,
    raw: &InitialStateInput,
    config: &EngineConfig,
    cache: &mut TrajectoryCache,
) -> Option<Trajectory> {
    match system_id {
        "classic-collatz" => Some(cache.get_or_compute(&collatz::ClassicCollatz, raw, config)),
        "five-n-plus-one" => {
            Some(cache.get_or_compute(&five_n_plus_one::FiveNPlusOne, raw, config))
        }
        _ => None,
    }
}
