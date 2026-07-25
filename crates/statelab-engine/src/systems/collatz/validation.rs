//! Hand-verified Classic Collatz validation cases (§4.4 / §7.2).
//!
//! These small sequences are independently derivable by hand. Larger-range
//! self-consistency checking (n = 1..10_000) is done programmatically in the test
//! suite rather than by hand-copying published values (§4.4).

use num_bigint::BigUint;

use crate::system::{InitialStateInput, ValidationCase};
use crate::trajectory::TrajectoryStatus;

/// Builds the hand-verified validation dataset.
pub(crate) fn dataset() -> Vec<ValidationCase<BigUint>> {
    vec![
        case(
            "n = 1 (transitions 1->4->2->1 per the FROZEN transition-first order)",
            1,
            &[1, 4, 2, 1],
        ),
        case("n = 2 (2->1)", 2, &[2, 1]),
        case(
            "n = 3 (Appendix B worked example)",
            3,
            &[3, 10, 5, 16, 8, 4, 2, 1],
        ),
        case(
            "n = 6 (6->3 then the n=3 tail)",
            6,
            &[6, 3, 10, 5, 16, 8, 4, 2, 1],
        ),
        case(
            "n = 7",
            7,
            &[7, 22, 11, 34, 17, 52, 26, 13, 40, 20, 10, 5, 16, 8, 4, 2, 1],
        ),
    ]
}

/// Helper: all built-in cases converge, so `expected_status` is always `Converged`
/// and `expected_iteration_count` is `sequence.len() - 1`.
fn case(description: &str, start: u64, sequence: &[u64]) -> ValidationCase<BigUint> {
    let expected_sequence: Vec<BigUint> = sequence.iter().map(|&v| BigUint::from(v)).collect();
    ValidationCase {
        description: description.to_string(),
        input: InitialStateInput::new(start.to_string()),
        expected_status: TrajectoryStatus::Converged,
        expected_iteration_count: (sequence.len() - 1) as u64,
        expected_sequence: Some(expected_sequence),
    }
}
