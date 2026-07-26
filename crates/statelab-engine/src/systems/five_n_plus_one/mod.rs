//! The 5n+1 system — the **second** built-in [`DeterministicSystem`].
//!
//! Its whole purpose is to test Principle #6 ("the architecture must support
//! future deterministic systems without redesign") against something real rather
//! than a synthetic test double. It was added with **zero changes** to
//! `engine.rs`, `system.rs`, `trajectory.rs`, `cycle_detection.rs` or `cache.rs`.
//!
//! Unlike Classic Collatz, 5n+1 exercises all three non-error terminal statuses:
//!   * `n = 3`  converges to 1,
//!   * `n = 13` enters a 10-state cycle that never reaches 1,
//!   * `n = 7`  appears to diverge without bound.
//!
//! That makes it a far better regression surface for the generic engine than
//! Collatz alone, which (within any tested range) only ever converges.

mod validation;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use num_bigint::BigUint;
use num_traits::{One, Zero};

use crate::system::{
    DeterministicSystem, InitialStateInput, RawTrajectory, SystemMetrics, TerminationReason,
    TrajectoryHistory, ValidationCase, ValidationError, VisualizationHints,
};
use crate::systems::bigint_metrics;

/// The 5n+1 system over positive arbitrary-precision integers.
///
/// - Transition (parity evaluated BEFORE transformation, mirroring §4.4):
///   odd → `5n + 1` (parity bit 1); even → `n / 2` (parity bit 0).
/// - Termination: `state == 1` → `Converged`.
#[derive(Clone, Copy, Debug, Default)]
pub struct FiveNPlusOne;

impl DeterministicSystem for FiveNPlusOne {
    type State = BigUint;

    fn system_id(&self) -> &'static str {
        "five-n-plus-one"
    }

    fn system_version(&self) -> &'static str {
        "1.0.0"
    }

    fn validate_initial_state(
        &self,
        raw: &InitialStateInput,
    ) -> Result<Self::State, ValidationError> {
        let trimmed = raw.raw.trim();
        // `BigUint::from_str` already rejects negatives (no sign) and non-digits.
        let value: BigUint = trimmed.parse().map_err(|_| {
            ValidationError::new(format!(
                "Initial state must be a positive integer; got {:?}",
                raw.raw
            ))
        })?;
        if value.is_zero() {
            return Err(ValidationError::new(
                "Initial state must be a positive integer (> 0); got 0",
            ));
        }
        Ok(value)
    }

    fn transition(&self, state: &Self::State) -> Self::State {
        // Parity is read BEFORE transforming, exactly as Classic Collatz does.
        if state.bit(0) {
            state * 5u32 + 1u32
        } else {
            state >> 1
        }
    }

    fn is_terminated(
        &self,
        state: &Self::State,
        _history: &TrajectoryHistory<Self::State>,
    ) -> Option<TerminationReason> {
        // IMPLEMENTATION DECISION (§4.2): the frozen spec defines a termination
        // rule only for Classic Collatz. `state == 1` mirrors it, which is the
        // conventional choice for 5n+1 and keeps the two systems comparable.
        // Note this is a *weaker* guarantee here than for Collatz: 5n+1 has known
        // cycles that never reach 1 (e.g. from n = 13) and apparently divergent
        // orbits (n = 7), so this rule legitimately does not fire for many inputs
        // — the engine's generic cycle detection and iteration limit handle those.
        if state.is_one() {
            Some(TerminationReason::new("Reached fixed value 1"))
        } else {
            None
        }
    }

    fn states_equal(&self, a: &Self::State, b: &Self::State) -> bool {
        a == b
    }

    fn state_hash(&self, state: &Self::State) -> u64 {
        let mut hasher = DefaultHasher::new();
        state.hash(&mut hasher);
        hasher.finish()
    }

    fn extract_features(&self, raw: &RawTrajectory<'_, Self::State>) -> SystemMetrics {
        // Shared with Classic Collatz: every metric is parity/sequence-derived and
        // none is specific to the 3n+1 rule. See `bigint_metrics`.
        bigint_metrics::extract(raw)
    }

    fn validation_dataset(&self) -> Vec<ValidationCase<Self::State>> {
        validation::dataset()
    }

    fn visualization_hints(&self) -> Option<VisualizationHints> {
        // IMPLEMENTATION DECISION (§4.4): None, as for Classic Collatz — coral
        // defaults are a UI/UX concern, not an engine one.
        None
    }
}
