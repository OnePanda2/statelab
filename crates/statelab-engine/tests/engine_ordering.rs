//! Generic-engine tests using synthetic systems (§7.2, §7.3).
//!
//! These systems exist only to exercise the engine's guarantees independently of
//! Collatz:
//!   * the ordering guarantee (convergence is checked before cycle detection),
//!   * generic cycle detection against a deliberately constructed cycle,
//!   * a property-based check of status/iteration_count across randomized configs.

use proptest::prelude::*;

use statelab_engine::{
    DeterministicSystem, EngineConfig, InitialStateInput, RawTrajectory, StateEvolutionEngine,
    SystemMetrics, TerminationReason, TrajectoryHistory, TrajectoryStatus, ValidationCase,
    VisualizationHints,
};

/// Parses a `u64` initial state for the synthetic systems.
fn parse_u64(raw: &InitialStateInput) -> Result<u64, statelab_engine::ValidationError> {
    raw.raw
        .trim()
        .parse::<u64>()
        .map_err(|_| statelab_engine::ValidationError::new("expected a u64"))
}

fn hash_u64(state: &u64) -> u64 {
    *state
}

/// Counts down to 0; terminates (its own rule) when it reaches 0. Converges in
/// exactly `k` transitions from `k`, or hits the iteration limit first.
struct Countdown;

impl DeterministicSystem for Countdown {
    type State = u64;

    fn system_id(&self) -> &'static str {
        "test-countdown"
    }
    fn system_version(&self) -> &'static str {
        "1.0.0"
    }
    fn validate_initial_state(
        &self,
        raw: &InitialStateInput,
    ) -> Result<u64, statelab_engine::ValidationError> {
        parse_u64(raw)
    }
    fn transition(&self, state: &u64) -> u64 {
        state.saturating_sub(1)
    }
    fn is_terminated(&self, state: &u64, _h: &TrajectoryHistory<u64>) -> Option<TerminationReason> {
        (*state == 0).then(|| TerminationReason::new("reached zero"))
    }
    fn states_equal(&self, a: &u64, b: &u64) -> bool {
        a == b
    }
    fn state_hash(&self, s: &u64) -> u64 {
        hash_u64(s)
    }
    fn extract_features(&self, _raw: &RawTrajectory<'_, u64>) -> SystemMetrics {
        SystemMetrics::empty()
    }
    fn validation_dataset(&self) -> Vec<ValidationCase<u64>> {
        Vec::new()
    }
    fn visualization_hints(&self) -> Option<VisualizationHints> {
        None
    }
}

/// Never terminates; enters a fixed cycle 1 -> 2 -> 3 -> 1. From start 0 the
/// sequence is 0, 1, 2, 3, 1(!) — the second `1` closes the cycle.
struct Cyclic;

impl DeterministicSystem for Cyclic {
    type State = u64;

    fn system_id(&self) -> &'static str {
        "test-cyclic"
    }
    fn system_version(&self) -> &'static str {
        "1.0.0"
    }
    fn validate_initial_state(
        &self,
        raw: &InitialStateInput,
    ) -> Result<u64, statelab_engine::ValidationError> {
        parse_u64(raw)
    }
    fn transition(&self, state: &u64) -> u64 {
        match state {
            0 => 1,
            1 => 2,
            2 => 3,
            3 => 1,
            other => *other,
        }
    }
    fn is_terminated(&self, _s: &u64, _h: &TrajectoryHistory<u64>) -> Option<TerminationReason> {
        None // never terminates by its own rule -> the engine must detect the cycle
    }
    fn states_equal(&self, a: &u64, b: &u64) -> bool {
        a == b
    }
    fn state_hash(&self, s: &u64) -> u64 {
        hash_u64(s)
    }
    fn extract_features(&self, _raw: &RawTrajectory<'_, u64>) -> SystemMetrics {
        SystemMetrics::empty()
    }
    fn validation_dataset(&self) -> Vec<ValidationCase<u64>> {
        Vec::new()
    }
    fn visualization_hints(&self) -> Option<VisualizationHints> {
        None
    }
}

/// Alternates 0 <-> 1 and *also* terminates when it reaches 0. From start 0, the
/// step that produces the second `0` simultaneously (a) satisfies the system's own
/// termination rule and (b) would close the 0<->1 cycle. Termination must win.
struct TerminationBeatsCycle;

impl DeterministicSystem for TerminationBeatsCycle {
    type State = u64;

    fn system_id(&self) -> &'static str {
        "test-termination-beats-cycle"
    }
    fn system_version(&self) -> &'static str {
        "1.0.0"
    }
    fn validate_initial_state(
        &self,
        raw: &InitialStateInput,
    ) -> Result<u64, statelab_engine::ValidationError> {
        parse_u64(raw)
    }
    fn transition(&self, state: &u64) -> u64 {
        if *state == 0 {
            1
        } else {
            0
        }
    }
    fn is_terminated(&self, state: &u64, _h: &TrajectoryHistory<u64>) -> Option<TerminationReason> {
        (*state == 0).then(|| TerminationReason::new("reached zero"))
    }
    fn states_equal(&self, a: &u64, b: &u64) -> bool {
        a == b
    }
    fn state_hash(&self, s: &u64) -> u64 {
        hash_u64(s)
    }
    fn extract_features(&self, _raw: &RawTrajectory<'_, u64>) -> SystemMetrics {
        SystemMetrics::empty()
    }
    fn validation_dataset(&self) -> Vec<ValidationCase<u64>> {
        Vec::new()
    }
    fn visualization_hints(&self) -> Option<VisualizationHints> {
        None
    }
}

#[test]
fn cycle_is_detected_generically() {
    let config = EngineConfig::with_max_iterations(1_000);
    let t = StateEvolutionEngine::run(&Cyclic, &InitialStateInput::new("0"), &config);
    assert_eq!(t.trajectory_status, TrajectoryStatus::CycleDetected);
    let info = t.cycle_information.expect("cycle info present");
    // Sequence 0,1,2,3,1 -> the repeated state `1` first appeared at index 1,
    // and the cycle 1->2->3->1 spans 3 states.
    assert_eq!(info.cycle_start_index, 1);
    assert_eq!(info.cycle_length, 3);
    assert_eq!(info.repeated_state, "1");
}

#[test]
fn termination_beats_cycle_on_the_same_step() {
    // The critical §7.2 ordering-guarantee regression: convergence (step 2) is
    // checked before cycle detection (step 3), so this MUST be Converged.
    let config = EngineConfig::with_max_iterations(1_000);
    let t = StateEvolutionEngine::run(
        &TerminationBeatsCycle,
        &InitialStateInput::new("0"),
        &config,
    );
    assert_eq!(t.trajectory_status, TrajectoryStatus::Converged);
    assert!(t.cycle_information.is_none());
    // 0 -> 1 -> 0 : two transitions, the second producing the terminating 0.
    assert_eq!(t.iteration_count, 2);
}

/// OQ-2, resolved 2026-07-26 as option (b): **every trajectory applies at least
/// one transition**, even when the initial state already satisfies the system's
/// own termination rule.
///
/// This follows from the FROZEN §4.1 order — step 1 transitions, step 2 checks
/// convergence — and is now an intentional, signed-off rule rather than an
/// accidental consequence. See PROJECT_BRIEF Addendum A.2 / C and OPEN_QUESTIONS
/// OQ-2.
///
/// `TerminationBeatsCycle` starting at 0 is exactly this case: 0 already meets
/// its own termination condition, yet the engine must still go 0 -> 1 -> 0.
/// Pinned by a test because prose does not survive a refactor: implementing
/// option (a) (an initial-state "step 0" check) would make this fail, which is
/// the intended alarm.
#[test]
fn a_system_starting_at_its_own_fixed_point_still_transitions() {
    let config = EngineConfig::with_max_iterations(1_000);
    let t = StateEvolutionEngine::run(
        &TerminationBeatsCycle,
        &InitialStateInput::new("0"),
        &config,
    );

    assert_eq!(t.trajectory_status, TrajectoryStatus::Converged);
    assert_ne!(
        t.iteration_count, 0,
        "a zero-iteration trajectory would mean the engine checked termination \
         before transitioning, contradicting the FROZEN §4.1 order"
    );
    assert_eq!(t.iteration_count, 2, "0 -> 1 -> 0");
    assert_eq!(t.state_sequence, ["0", "1", "0"]);
    assert!(
        t.state_sequence.len() > 1,
        "the sequence must contain the round trip, not just the initial state"
    );
}

#[test]
fn iteration_limit_is_reported() {
    // Countdown from 100 with a limit of 10 can't reach 0 first.
    let config = EngineConfig::with_max_iterations(10);
    let t = StateEvolutionEngine::run(&Countdown, &InitialStateInput::new("100"), &config);
    assert_eq!(t.trajectory_status, TrajectoryStatus::IterationLimitReached);
    assert_eq!(t.iteration_count, 10);
}

proptest! {
    /// For the Countdown system the terminal status and iteration count are
    /// analytically known, so the engine's report can be checked against them
    /// across randomized (start, limit) pairs (§7.3).
    #[test]
    fn countdown_status_matches_analysis(k in 1u64..500, max in 1u64..1000) {
        let config = EngineConfig::with_max_iterations(max);
        let t = StateEvolutionEngine::run(&Countdown, &InitialStateInput::new(k.to_string()), &config);
        if max >= k {
            prop_assert_eq!(t.trajectory_status, TrajectoryStatus::Converged);
            prop_assert_eq!(t.iteration_count, k);
        } else {
            prop_assert_eq!(t.trajectory_status, TrajectoryStatus::IterationLimitReached);
            prop_assert_eq!(t.iteration_count, max);
        }
    }
}
