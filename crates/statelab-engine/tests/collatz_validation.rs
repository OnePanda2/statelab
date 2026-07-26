//! Classic Collatz correctness tests (§7.2, §10.2 step 4).
//!
//! Covers: the hand-verified n = 1/2/3 cases, the well-known n = 27 boundary case,
//! a programmatically-generated + internally self-consistency-checked sweep over
//! n = 1..=10_000, the Appendix B worked example, and schema round-tripping.

use serde_json::Value;

use statelab_engine::{
    ClassicCollatz, DeterministicSystem, EngineConfig, InitialStateInput, StateEvolutionEngine,
    Trajectory, TrajectoryStatus,
};

/// Runs one Collatz trajectory with a generous iteration limit.
fn run(n: u64) -> Trajectory {
    let system = ClassicCollatz;
    let config = EngineConfig::with_max_iterations(1_000_000);
    StateEvolutionEngine::run(&system, &InitialStateInput::new(n.to_string()), &config)
}

fn metric<'a>(t: &'a Trajectory, key: &str) -> &'a Value {
    t.system_specific_metrics
        .get(key)
        .unwrap_or_else(|| panic!("metric {key:?} missing"))
}

#[test]
fn hand_verified_small_cases() {
    // n = 1 follows the FROZEN transition-first generation order: 1 -> 4 -> 2 -> 1.
    let t1 = run(1);
    assert_eq!(t1.state_sequence, ["1", "4", "2", "1"]);
    assert_eq!(t1.iteration_count, 3);
    assert_eq!(t1.trajectory_status, TrajectoryStatus::Converged);
    // Stopping time is N/A for n = 1 (value never drops below the start).
    assert_eq!(*metric(&t1, "stopping_time"), Value::Null);
    assert_eq!(metric(&t1, "total_stopping_time").as_u64(), Some(3));

    // n = 2 -> 1.
    let t2 = run(2);
    assert_eq!(t2.state_sequence, ["2", "1"]);
    assert_eq!(t2.iteration_count, 1);
    assert_eq!(t2.trajectory_status, TrajectoryStatus::Converged);

    // n = 3, the Appendix B worked example.
    let t3 = run(3);
    assert_eq!(
        t3.state_sequence,
        ["3", "10", "5", "16", "8", "4", "2", "1"]
    );
    assert_eq!(t3.iteration_count, 7);
    assert_eq!(t3.trajectory_status, TrajectoryStatus::Converged);
    assert_eq!(t3.termination_reason, "Reached fixed value 1");
    assert!(t3.cycle_information.is_none());
}

#[test]
fn appendix_b_metrics_exact() {
    let t = run(3);
    assert_eq!(metric(&t, "stopping_time").as_u64(), Some(6));
    assert_eq!(metric(&t, "total_stopping_time").as_u64(), Some(7));
    assert_eq!(metric(&t, "peak_value"), &Value::String("16".into()));
    assert_eq!(metric(&t, "peak_index").as_u64(), Some(3));
    assert_eq!(metric(&t, "odd_count").as_u64(), Some(2));
    assert_eq!(metric(&t, "even_count").as_u64(), Some(5));
    assert_eq!(metric(&t, "maximum_bit_length").as_u64(), Some(5));
    assert_eq!(
        metric(&t, "parity_sequence"),
        &serde_json::json!([1, 0, 1, 0, 0, 0, 0])
    );
    assert_eq!(
        metric(&t, "bit_length_evolution"),
        &serde_json::json!([2, 4, 3, 5, 4, 3, 2, 1])
    );
    assert_eq!(
        metric(&t, "binary_transition_statistics"),
        &serde_json::json!({ "increases": 2, "decreases": 5, "same": 0 })
    );
    assert_eq!(
        metric(&t, "run_length_statistics"),
        &serde_json::json!([1, 1, 1, 4])
    );

    let odd_ratio = metric(&t, "odd_ratio").as_f64().expect("odd_ratio is f64");
    assert!((odd_ratio - 2.0 / 7.0).abs() < 1e-12);
    let even_ratio = metric(&t, "even_ratio")
        .as_f64()
        .expect("even_ratio is f64");
    assert!((even_ratio - 5.0 / 7.0).abs() < 1e-12);

    let growth = metric(&t, "average_growth")
        .as_f64()
        .expect("average_growth is f64");
    assert!((growth - (10.0 / 3.0 + 16.0 / 5.0) / 2.0).abs() < 1e-12);
    let decline = metric(&t, "average_decline")
        .as_f64()
        .expect("average_decline is f64");
    assert!((decline - 0.5).abs() < 1e-12);
}

#[test]
fn n_27_boundary_case() {
    // Classic large-peak case: 111 total stopping time, peak value 9232.
    let t = run(27);
    assert_eq!(t.trajectory_status, TrajectoryStatus::Converged);
    assert_eq!(t.iteration_count, 111);
    assert_eq!(metric(&t, "total_stopping_time").as_u64(), Some(111));
    assert_eq!(metric(&t, "peak_value"), &Value::String("9232".into()));
    // BigInt boundary: the peak is carried as a decimal string, never a native number.
    assert!(metric(&t, "peak_value").is_string());
}

#[test]
fn validation_dataset_passes() {
    let system = ClassicCollatz;
    let config = EngineConfig::with_max_iterations(1_000_000);
    for case in system.validation_dataset() {
        let t = StateEvolutionEngine::run(&system, &case.input, &config);
        assert_eq!(
            t.trajectory_status, case.expected_status,
            "status mismatch for case: {}",
            case.description
        );
        assert_eq!(
            t.iteration_count, case.expected_iteration_count,
            "iteration_count mismatch for case: {}",
            case.description
        );
        if let Some(expected) = &case.expected_sequence {
            let expected_strings: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
            assert_eq!(
                t.state_sequence, expected_strings,
                "sequence mismatch for case: {}",
                case.description
            );
        }
    }
}

/// Programmatic sweep with internal self-consistency checks (§4.4). No published
/// values are hand-copied — each trajectory is validated against invariants that
/// must hold for every converged Collatz run.
#[test]
fn programmatic_self_consistency_1_to_10000() {
    for n in 1u64..=10_000 {
        let t = run(n);

        assert_eq!(
            t.trajectory_status,
            TrajectoryStatus::Converged,
            "n = {n} did not converge within the iteration limit"
        );

        // Sequence framing.
        assert_eq!(
            t.state_sequence.first().map(String::as_str),
            Some(n.to_string().as_str())
        );
        assert_eq!(t.state_sequence.last().map(String::as_str), Some("1"));
        assert_eq!(t.state_sequence.len() as u64, t.iteration_count + 1);

        // Total Stopping Time must equal iteration_count for a converged run.
        assert_eq!(
            metric(&t, "total_stopping_time").as_u64(),
            Some(t.iteration_count),
            "total_stopping_time != iteration_count for n = {n}"
        );

        // Odd Count + Even Count must equal iteration_count.
        let odd = metric(&t, "odd_count").as_u64().expect("odd_count u64");
        let even = metric(&t, "even_count").as_u64().expect("even_count u64");
        assert_eq!(
            odd + even,
            t.iteration_count,
            "odd+even != iterations for n = {n}"
        );

        // Parity sequence has exactly one bit per transition.
        let parity_len = metric(&t, "parity_sequence")
            .as_array()
            .map(|a| a.len() as u64)
            .expect("parity_sequence array");
        assert_eq!(
            parity_len, t.iteration_count,
            "parity length wrong for n = {n}"
        );

        // Bit-length evolution has one entry per state.
        let bit_len = metric(&t, "bit_length_evolution")
            .as_array()
            .map(|a| a.len() as u64)
            .expect("bit_length_evolution array");
        assert_eq!(
            bit_len,
            t.iteration_count + 1,
            "bit-length length wrong for n = {n}"
        );

        // Stopping time: null for n = 1, otherwise a real index into a smaller value.
        let stopping = metric(&t, "stopping_time");
        if n == 1 {
            assert_eq!(
                *stopping,
                Value::Null,
                "n = 1 should have N/A stopping time"
            );
        } else if let Some(idx) = stopping.as_u64() {
            assert!(
                idx >= 1 && idx <= t.iteration_count,
                "stopping_time out of range for n = {n}"
            );
            let value_at: u128 = t.state_sequence[idx as usize].parse().expect("small value");
            assert!(
                value_at < n as u128,
                "stopping-time value not below start for n = {n}"
            );
        }
    }
}

#[test]
fn invalid_inputs_report_system_error() {
    let system = ClassicCollatz;
    let config = EngineConfig::default();
    for bad in ["0", "-5", "abc", "", "3.5", "  "] {
        let t = StateEvolutionEngine::run(&system, &InitialStateInput::new(bad), &config);
        assert_eq!(
            t.trajectory_status,
            TrajectoryStatus::SystemError,
            "input {bad:?} should be a SystemError"
        );
        assert!(t.system_specific_metrics.is_empty());
    }
}

/// The duration field is the only nondeterministic value in a Trajectory, and an
/// unrounded `f64` is not guaranteed to survive a JSON round trip bit-exactly —
/// which made `schema_round_trips` flaky. Capture rounds it to microseconds, so
/// re-serializing must now be exactly stable for any run.
#[test]
fn execution_duration_round_trips_exactly() {
    for n in [1u64, 3, 27, 871] {
        let t = run(n);
        let ms = t.execution_metadata.computation_duration_ms;
        let reparsed: f64 =
            serde_json::from_str(&serde_json::to_string(&ms).expect("ser")).expect("de");
        assert_eq!(
            ms, reparsed,
            "duration {ms} for n={n} did not round-trip exactly"
        );
        // Rounded to microsecond resolution: 1000x the value is a whole number.
        assert_eq!((ms * 1_000.0).fract(), 0.0, "duration {ms} is not rounded");
    }
}

#[test]
fn schema_round_trips() {
    let t = run(27);
    let json = serde_json::to_string(&t).expect("serialize");
    let back: Trajectory = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(
        t, back,
        "Trajectory did not survive a serialize/deserialize round trip"
    );

    // Value-level equality is order-independent and confirms exact JSON shape.
    let a: Value = serde_json::from_str(&json).expect("parse a");
    let b: Value = serde_json::to_value(&back).expect("parse b");
    assert_eq!(a, b);

    assert_eq!(t.trajectory_schema_version, "1.0.0");
    assert_eq!(t.system_id, "classic-collatz");
}
