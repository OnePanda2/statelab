//! Classic Collatz — the first built-in [`DeterministicSystem`] (§4.4).
//!
//! Nothing here is privileged: Collatz is one instance of the general interface.
//! The engine never names it.

mod metrics;
mod validation;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use num_bigint::BigUint;
use num_traits::{One, Zero};

use crate::system::{
    DeterministicSystem, InitialStateInput, RawTrajectory, SystemMetrics, TerminationReason,
    TrajectoryHistory, ValidationCase, ValidationError, VisualizationHints,
};

/// The Classic Collatz (3n+1) system over positive arbitrary-precision integers.
///
/// - State: `BigUint` (decimal string at the JSON boundary — never a native number).
/// - Transition (parity evaluated BEFORE transformation, FROZEN):
///   odd -> `3n + 1` (parity bit 1); even -> `n / 2` (parity bit 0).
/// - Termination: `state == 1` -> `Converged`, reason `"Reached fixed value 1"`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClassicCollatz;

impl DeterministicSystem for ClassicCollatz {
    type State = BigUint;

    fn system_id(&self) -> &'static str {
        "classic-collatz"
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
        // Parity is read BEFORE transforming (FROZEN). `bit(0) == true` means odd.
        if state.bit(0) {
            state * 3u32 + 1u32
        } else {
            state >> 1
        }
    }

    fn is_terminated(
        &self,
        state: &Self::State,
        _history: &TrajectoryHistory<Self::State>,
    ) -> Option<TerminationReason> {
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
        // `DefaultHasher::new()` uses fixed keys (not `RandomState`), so this is
        // deterministic within and across runs — adequate for cycle-bucket indexing.
        let mut hasher = DefaultHasher::new();
        state.hash(&mut hasher);
        hasher.finish()
    }

    fn extract_features(&self, raw: &RawTrajectory<'_, Self::State>) -> SystemMetrics {
        metrics::extract(raw)
    }

    fn validation_dataset(&self) -> Vec<ValidationCase<Self::State>> {
        validation::dataset()
    }

    fn visualization_hints(&self) -> Option<VisualizationHints> {
        // IMPLEMENTATION DECISION §4.4: None for the MVP. Coral defaults are a
        // UI/UX decision, not an engine one — visualizations must work without hints.
        None
    }
}
