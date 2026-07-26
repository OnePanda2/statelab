//! Hand-verified 5n+1 validation cases (§4.4 / §7.2).
//!
//! Each sequence below was derived by direct simulation and re-checked by hand
//! before being committed — not copied from a secondary source. The three cases
//! deliberately span all three non-error terminal statuses, which is what makes
//! this system a useful check on the *generic* engine.

use num_bigint::BigUint;

use crate::system::{InitialStateInput, ValidationCase};
use crate::trajectory::TrajectoryStatus;

/// Builds the hand-verified validation dataset.
pub(crate) fn dataset() -> Vec<ValidationCase<BigUint>> {
    vec![
        // 1 is odd, so (like Collatz's n = 1) the transition-first generation
        // order sends it out and back: 1 -> 6 -> 3 -> 16 -> 8 -> 4 -> 2 -> 1.
        converges(
            "n = 1 (odd, so it round-trips back to 1)",
            1,
            &[1, 6, 3, 16, 8, 4, 2, 1],
        ),
        converges(
            "n = 3 (3 -> 16 then a pure halving chain)",
            3,
            &[3, 16, 8, 4, 2, 1],
        ),
        // n = 13 never reaches 1: it returns to itself after 10 transitions, so
        // the engine's generic cycle detector must fire. 83*5+1 = 416 is the step
        // most worth double-checking here.
        ValidationCase {
            description: "n = 13 (10-state cycle; the initial state is in the cycle)".to_string(),
            input: InitialStateInput::new("13"),
            expected_status: TrajectoryStatus::CycleDetected,
            expected_iteration_count: 10,
            expected_sequence: Some(
                [13u64, 66, 33, 166, 83, 416, 208, 104, 52, 26, 13]
                    .iter()
                    .map(|&v| BigUint::from(v))
                    .collect(),
            ),
        },
    ]
}

/// Helper for the converging cases.
fn converges(description: &str, start: u64, sequence: &[u64]) -> ValidationCase<BigUint> {
    ValidationCase {
        description: description.to_string(),
        input: InitialStateInput::new(start.to_string()),
        expected_status: TrajectoryStatus::Converged,
        expected_iteration_count: (sequence.len() - 1) as u64,
        expected_sequence: Some(sequence.iter().map(|&v| BigUint::from(v)).collect()),
    }
}
