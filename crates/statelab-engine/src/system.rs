//! The Deterministic System interface (§4.2 FROZEN) and its supporting value types.
//!
//! The engine communicates with a concrete system **only** through the
//! [`DeterministicSystem`] trait. Nothing about Collatz — or any other named
//! system — may leak into [`crate::engine`].

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::trajectory::TrajectoryStatus;

/// A metric value embedded in [`SystemMetrics`]. Modelled as a JSON value so the
/// per-system metrics dictionary is fully general (integers, decimal-string
/// big integers, ratios, arrays, nested objects) without the engine knowing the
/// shape of any particular system's metrics.
pub type MetricValue = serde_json::Value;

/// Raw, unvalidated initial-state input as it arrives from the UI (BigInt-safe:
/// always carried as a string, never a native number — §4.3).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InitialStateInput {
    /// The raw user-entered representation (e.g. a decimal string `"27"`).
    pub raw: String,
}

impl InitialStateInput {
    /// Wraps a raw input string.
    pub fn new(raw: impl Into<String>) -> Self {
        Self { raw: raw.into() }
    }
}

impl From<&str> for InitialStateInput {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for InitialStateInput {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Rejection produced when raw input cannot be parsed/validated into a system's
/// initial state. Surfaces to the UI as a [`TrajectoryStatus::SystemError`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationError {
    /// Human-readable reason the input was rejected.
    pub message: String,
}

impl ValidationError {
    /// Builds a validation error with the given human-readable message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ValidationError {}

/// A system's own success/convergence explanation, returned by
/// [`DeterministicSystem::is_terminated`]. Human-readable; becomes the
/// Trajectory's `termination_reason` for a converged run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminationReason(pub String);

impl TerminationReason {
    /// Builds a termination reason from any string-like value.
    pub fn new(reason: impl Into<String>) -> Self {
        Self(reason.into())
    }
}

/// The ordered history of states accumulated during a single run. Invariant:
/// **never empty** — always constructed with the initial state at index 0.
#[derive(Clone, Debug)]
pub struct TrajectoryHistory<S> {
    states: Vec<S>,
}

impl<S: Clone> TrajectoryHistory<S> {
    /// Creates a history seeded with the (already validated) initial state.
    pub fn new(initial: S) -> Self {
        Self {
            states: vec![initial],
        }
    }

    /// Appends the next state produced by a transition.
    pub fn push(&mut self, state: S) {
        self.states.push(state);
    }

    /// The most recent state. Never panics: the history is non-empty by construction.
    pub fn current(&self) -> &S {
        match self.states.last() {
            Some(state) => state,
            // Unreachable: `states` starts with the initial state and only grows.
            None => unreachable!("TrajectoryHistory invariant violated: history is empty"),
        }
    }

    /// Number of transitions applied so far (states beyond the initial one).
    pub fn iteration_count(&self) -> u64 {
        self.states.len().saturating_sub(1) as u64
    }

    /// The full ordered state slice, including the initial state.
    pub fn states(&self) -> &[S] {
        &self.states
    }

    /// A borrowed view for the Feature Extraction stage.
    pub fn as_raw(&self) -> RawTrajectory<'_, S> {
        RawTrajectory::new(&self.states)
    }
}

/// A read-only borrow of a completed run's raw states, handed to a system's
/// Feature Extractor. Borrowing (not owning) keeps the trait free of allocation
/// and makes it explicit that feature extraction never mutates the history.
#[derive(Clone, Copy, Debug)]
pub struct RawTrajectory<'a, S> {
    states: &'a [S],
}

impl<'a, S> RawTrajectory<'a, S> {
    /// Wraps a state slice (the initial state is at index 0).
    pub fn new(states: &'a [S]) -> Self {
        Self { states }
    }

    /// The full ordered state slice, including the initial state.
    pub fn states(&self) -> &'a [S] {
        self.states
    }

    /// Number of transitions applied (states beyond the initial one).
    pub fn iteration_count(&self) -> u64 {
        self.states.len().saturating_sub(1) as u64
    }
}

/// The immutable, system-defined metrics dictionary embedded in a Trajectory.
///
/// Serializes transparently as a JSON object. Consumers that need a key that is
/// absent must render the literal string `"Metric Not Supported"` — they must
/// never compute a substitute (Principle #3, §5.2). Once built it is never mutated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SystemMetrics {
    values: BTreeMap<String, MetricValue>,
}

impl SystemMetrics {
    /// Starts building a metrics dictionary.
    pub fn builder() -> SystemMetricsBuilder {
        SystemMetricsBuilder {
            values: BTreeMap::new(),
        }
    }

    /// An empty metrics dictionary (used for `SystemError` trajectories).
    pub fn empty() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }

    /// Looks up a metric by key. `None` means the consumer should render
    /// `"Metric Not Supported"`.
    pub fn get(&self, key: &str) -> Option<&MetricValue> {
        self.values.get(key)
    }

    /// Iterates the metric keys in stable (sorted) order.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.values.keys()
    }

    /// Number of metrics present.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether no metrics are present.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// Builder for [`SystemMetrics`]. The only way to populate a metrics dictionary;
/// guarantees the finished [`SystemMetrics`] is immutable.
#[derive(Clone, Debug, Default)]
pub struct SystemMetricsBuilder {
    values: BTreeMap<String, MetricValue>,
}

impl SystemMetricsBuilder {
    /// Inserts a metric. Any JSON-serializable value is accepted.
    pub fn insert(mut self, key: impl Into<String>, value: impl Into<MetricValue>) -> Self {
        self.values.insert(key.into(), value.into());
        self
    }

    /// Finalizes the immutable metrics dictionary.
    pub fn build(self) -> SystemMetrics {
        SystemMetrics {
            values: self.values,
        }
    }
}

/// An authoritative, independently-verifiable case used for regression testing a
/// system against its own [`DeterministicSystem::validation_dataset`].
#[derive(Clone, Debug)]
pub struct ValidationCase<S> {
    /// What this case checks (for test output).
    pub description: String,
    /// Raw input to feed the engine.
    pub input: InitialStateInput,
    /// Expected terminal status.
    pub expected_status: TrajectoryStatus,
    /// Expected number of transitions.
    pub expected_iteration_count: u64,
    /// Optional full expected state sequence (including the initial state).
    pub expected_sequence: Option<Vec<S>>,
}

/// Optional, per-system rendering defaults (e.g. suggested Coral angles).
/// Visualizations must render correctly when this is `None` (§4.2).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VisualizationHints {
    /// Suggested Coral turn angle after an odd-parity transition.
    pub coral_odd_angle: Option<f64>,
    /// Suggested Coral turn angle after an even-parity transition.
    pub coral_even_angle: Option<f64>,
    /// Suggested Coral segment length.
    pub coral_line_length: Option<f64>,
}

/// The pluggable interface every deterministic system implements (§4.2 FROZEN).
///
/// The engine drives a system **only** through this trait. Implementors must keep
/// [`transition`](DeterministicSystem::transition) a pure function with no side
/// effects so that runs are reproducible.
///
/// > Implementation note: the engine additionally requires `State: Display` at its
/// > own driver boundary (to serialize states into the Trajectory's BigInt-safe
/// > string form, §4.3). That bound lives on the engine's `run`/`finalize`
/// > functions, not here, so the trait text stays exactly as frozen in §4.2.
pub trait DeterministicSystem {
    /// The state representation for this system. Equality and hashing are provided
    /// via [`states_equal`](Self::states_equal) / [`state_hash`](Self::state_hash)
    /// so the engine's generic cycle detector can work without knowing the shape.
    type State: Clone;

    /// Stable machine identifier, e.g. `"classic-collatz"`.
    fn system_id(&self) -> &'static str;

    /// Semantic version of this system implementation, e.g. `"1.0.0"`.
    fn system_version(&self) -> &'static str;

    /// Parses/validates raw user input into a valid initial state, or rejects it.
    fn validate_initial_state(
        &self,
        raw: &InitialStateInput,
    ) -> Result<Self::State, ValidationError>;

    /// Produces the next state from the current one. **Pure**; no side effects.
    fn transition(&self, state: &Self::State) -> Self::State;

    /// This system's own success/convergence condition. `Some(reason)` terminates
    /// the run as [`TrajectoryStatus::Converged`]; `None` means "not yet".
    fn is_terminated(
        &self,
        state: &Self::State,
        history: &TrajectoryHistory<Self::State>,
    ) -> Option<TerminationReason>;

    /// Structural equality of two states (used by generic cycle detection).
    fn states_equal(&self, a: &Self::State, b: &Self::State) -> bool;

    /// A hash of a state, consistent with [`states_equal`](Self::states_equal):
    /// equal states must hash equally. Used to index the cycle detector's buckets.
    fn state_hash(&self, state: &Self::State) -> u64;

    /// Computes this system's System-Specific Metrics from a completed raw run.
    /// Called exactly once, at trajectory-build time; never re-run downstream.
    fn extract_features(&self, raw: &RawTrajectory<'_, Self::State>) -> SystemMetrics;

    /// Authoritative cases used for regression testing this system.
    fn validation_dataset(&self) -> Vec<ValidationCase<Self::State>>;

    /// Optional visualization defaults. `None` is valid; visualizations must not
    /// depend on this being present.
    fn visualization_hints(&self) -> Option<VisualizationHints>;
}
