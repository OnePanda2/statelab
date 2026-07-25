//! The Trajectory Object (§4.3 FROZEN fields; JSON shape is the serialization
//! IMPLEMENTATION DECISION) plus its terminal-status enum and execution metadata.
//!
//! A [`Trajectory`] is the single immutable output artifact of one engine run.
//! Once assembled it is never mutated by any consumer (Principle #5).

use std::fmt::Display;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::engine::EngineConfig;
use crate::system::{DeterministicSystem, InitialStateInput, SystemMetrics, TerminationReason};
use crate::ENGINE_VERSION;

/// Current Trajectory schema version (§4.9). Only additive evolution is allowed;
/// any breaking change requires a migration function and a version bump.
pub const TRAJECTORY_SCHEMA_VERSION: &str = "1.0.0";

/// Machine-readable terminal status (§4.7 FROZEN). Every run resolves to exactly
/// one of these four — `Unknown` was explicitly removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrajectoryStatus {
    /// The system's own success condition was met (checked before cycle detection).
    Converged,
    /// The engine's generic cycle detector found a revisited state.
    CycleDetected,
    /// The configured iteration limit was reached first.
    IterationLimitReached,
    /// Input validation (or another system-reported error) failed.
    SystemError,
}

/// Details of a detected cycle. Present only when
/// [`TrajectoryStatus::CycleDetected`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CycleInfo {
    /// Index in `state_sequence` where the cycle begins.
    pub cycle_start_index: usize,
    /// Number of states in the cycle.
    pub cycle_length: usize,
    /// The revisited state (decimal/canonical string form).
    pub repeated_state: String,
}

/// Reproducibility/audit metadata captured per run (§4.3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExecutionMetadata {
    /// Wall-clock duration of the engine run, in milliseconds.
    pub computation_duration_ms: f64,
    /// Version of the engine that produced this trajectory.
    pub engine_version: String,
    /// Whether this trajectory was served from the memoization cache.
    pub cache_hit: bool,
    /// The `max_iterations` bound in effect for this run.
    pub iteration_limit_used: u64,
    /// UTC timestamp (RFC 3339) when the run finished.
    pub timestamp: String,
    /// Target platform string (`<arch>-<os>`).
    pub platform: String,
}

/// The immutable record of one completed engine run (§4.3).
///
/// Field order below matches the JSON schema documented in Appendix B. All state
/// values are serialized as strings (BigInt-safe) — never native numbers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trajectory {
    /// Schema version of this record (§4.9).
    pub trajectory_schema_version: String,
    /// Identifier of the system that produced it, e.g. `"classic-collatz"`.
    pub system_id: String,
    /// Version of that system implementation.
    pub system_version: String,
    /// The validated initial state, as a string.
    pub initial_state: String,
    /// Every state including the initial one, in order, as strings.
    pub state_sequence: Vec<String>,
    /// Number of transitions applied.
    pub iteration_count: u64,
    /// Machine-readable terminal status.
    pub trajectory_status: TrajectoryStatus,
    /// Human-readable termination explanation.
    pub termination_reason: String,
    /// Cycle details, present only for [`TrajectoryStatus::CycleDetected`].
    pub cycle_information: Option<CycleInfo>,
    /// Reproducibility/audit metadata.
    pub execution_metadata: ExecutionMetadata,
    /// The system-defined, immutable metrics dictionary.
    pub system_specific_metrics: SystemMetrics,
}

/// Internal, engine-only description of *why* a run finished. Converted into the
/// public `(status, termination_reason, cycle_information)` triple by
/// [`Trajectory::assemble`]. Not part of the public schema.
pub(crate) enum TerminationDetail {
    Converged(TerminationReason),
    Cycle(CycleInfo),
    IterationLimit,
}

impl Trajectory {
    /// Assembles the final immutable trajectory from a completed run.
    ///
    /// `S::State: Display` is required here (not on the trait) so states can be
    /// serialized into their BigInt-safe string form (§4.3).
    pub(crate) fn assemble<S>(
        system: &S,
        states: &[S::State],
        detail: TerminationDetail,
        metrics: SystemMetrics,
        config: &EngineConfig,
        started_at: Instant,
    ) -> Self
    where
        S: DeterministicSystem,
        S::State: Display,
    {
        let (trajectory_status, termination_reason, cycle_information) = match detail {
            TerminationDetail::Converged(reason) => (TrajectoryStatus::Converged, reason.0, None),
            TerminationDetail::Cycle(info) => {
                let reason = format!(
                    "Detected cycle of length {} beginning at index {}",
                    info.cycle_length, info.cycle_start_index
                );
                (TrajectoryStatus::CycleDetected, reason, Some(info))
            }
            TerminationDetail::IterationLimit => {
                let reason = format!(
                    "Reached iteration limit of {} iterations without termination",
                    config.max_iterations
                );
                (TrajectoryStatus::IterationLimitReached, reason, None)
            }
        };

        let state_sequence: Vec<String> = states.iter().map(|s| s.to_string()).collect();
        let initial_state = state_sequence.first().cloned().unwrap_or_default();
        let iteration_count = state_sequence.len().saturating_sub(1) as u64;

        Self {
            trajectory_schema_version: TRAJECTORY_SCHEMA_VERSION.to_string(),
            system_id: system.system_id().to_string(),
            system_version: system.system_version().to_string(),
            initial_state,
            state_sequence,
            iteration_count,
            trajectory_status,
            termination_reason,
            cycle_information,
            execution_metadata: ExecutionMetadata::capture(config, started_at, false),
            system_specific_metrics: metrics,
        }
    }

    /// Builds a `SystemError` trajectory for input that failed validation. No
    /// transitions were run; the raw input is echoed as the sole sequence entry.
    pub(crate) fn system_error<S>(
        system: &S,
        raw: &InitialStateInput,
        config: &EngineConfig,
        message: String,
        started_at: Instant,
    ) -> Self
    where
        S: DeterministicSystem,
    {
        Self {
            trajectory_schema_version: TRAJECTORY_SCHEMA_VERSION.to_string(),
            system_id: system.system_id().to_string(),
            system_version: system.system_version().to_string(),
            initial_state: raw.raw.clone(),
            state_sequence: vec![raw.raw.clone()],
            iteration_count: 0,
            trajectory_status: TrajectoryStatus::SystemError,
            termination_reason: message,
            cycle_information: None,
            execution_metadata: ExecutionMetadata::capture(config, started_at, false),
            system_specific_metrics: SystemMetrics::empty(),
        }
    }

    /// Returns a copy flagged as served from the memoization cache
    /// (`execution_metadata.cache_hit = true`). All mathematical content is
    /// unchanged — only this runtime-provenance flag differs, so a cached result
    /// stays byte-identical to a fresh computation everywhere it matters.
    pub fn mark_cache_hit(mut self) -> Self {
        self.execution_metadata.cache_hit = true;
        self
    }
}

impl ExecutionMetadata {
    fn capture(config: &EngineConfig, started_at: Instant, cache_hit: bool) -> Self {
        Self {
            computation_duration_ms: started_at.elapsed().as_secs_f64() * 1_000.0,
            engine_version: ENGINE_VERSION.to_string(),
            cache_hit,
            iteration_limit_used: config.max_iterations,
            timestamp: rfc3339_utc_now(),
            platform: format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
        }
    }
}

/// Formats the current wall-clock time as an RFC 3339 UTC string, dependency-free.
fn rfc3339_utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_utc(secs)
}

/// Converts Unix epoch seconds to `YYYY-MM-DDThh:mm:ssZ`.
fn format_utc(epoch_secs: u64) -> String {
    let days = (epoch_secs / 86_400) as i64;
    let rem = epoch_secs % 86_400;
    let (hour, minute, second) = (rem / 3_600, (rem % 3_600) / 60, rem % 60);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Howard Hinnant's `civil_from_days`: days since the Unix epoch -> (year, month, day).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if month <= 2 { year + 1 } else { year }, month, day)
}
