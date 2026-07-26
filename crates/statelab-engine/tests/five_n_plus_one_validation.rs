//! 5n+1 validation — the Principle #6 test (§1.3 #6).
//!
//! The point of this file is not 5n+1 itself. It is that a **second, real**
//! deterministic system produces all three non-error terminal statuses through
//! the *same generic engine*, with zero modification to `engine.rs`,
//! `system.rs`, `trajectory.rs`, `cycle_detection.rs` or `cache.rs`.
//!
//! Classic Collatz alone cannot demonstrate this: within any tested range it only
//! ever converges, so `CycleDetected` and `IterationLimitReached` were previously
//! only exercised by synthetic test doubles.

use statelab_engine::{
    DeterministicSystem, EngineConfig, FiveNPlusOne, InitialStateInput, StateEvolutionEngine,
    Trajectory, TrajectoryStatus,
};

/// Runs 5n+1 with an explicit limit (never the 10,000,000 production default —
/// these tests must stay fast).
fn run_with_limit(n: &str, max_iterations: u64) -> Trajectory {
    StateEvolutionEngine::run(
        &FiveNPlusOne,
        &InitialStateInput::new(n),
        &EngineConfig::with_max_iterations(max_iterations),
    )
}

fn run(n: &str) -> Trajectory {
    run_with_limit(n, 10_000)
}

#[test]
fn n_3_converges() {
    // 3 is odd -> 16, then a pure halving chain 16 -> 8 -> 4 -> 2 -> 1.
    let t = run("3");
    assert_eq!(t.trajectory_status, TrajectoryStatus::Converged);
    assert_eq!(t.state_sequence, ["3", "16", "8", "4", "2", "1"]);
    assert_eq!(t.iteration_count, 5);
    assert_eq!(t.termination_reason, "Reached fixed value 1");
    assert!(t.cycle_information.is_none());
}

#[test]
fn n_1_round_trips_like_collatz() {
    // 1 is odd, so the FROZEN transition-first order sends it out and back.
    // Documented in OPEN_QUESTIONS.md (OQ-2) — the same behaviour Collatz's n = 1
    // shows, which is the point: it is a property of the engine, not of Collatz.
    let t = run("1");
    assert_eq!(t.trajectory_status, TrajectoryStatus::Converged);
    assert_eq!(t.state_sequence, ["1", "6", "3", "16", "8", "4", "2", "1"]);
    assert_eq!(t.iteration_count, 7);
}

#[test]
fn n_13_is_a_cycle_the_generic_detector_finds() {
    // 13 -> 66 -> 33 -> 166 -> 83 -> 416 -> 208 -> 104 -> 52 -> 26 -> 13.
    // It never reaches 1, so `is_terminated` never fires and the engine's generic
    // cycle detection (§4.6) must catch it. The initial state is itself in the
    // cycle, so the cycle starts at index 0.
    let t = run("13");
    assert_eq!(t.trajectory_status, TrajectoryStatus::CycleDetected);
    assert_eq!(
        t.state_sequence,
        ["13", "66", "33", "166", "83", "416", "208", "104", "52", "26", "13"]
    );
    let info = t.cycle_information.expect("cycle info present");
    assert_eq!(info.cycle_start_index, 0);
    assert_eq!(info.cycle_length, 10);
    assert_eq!(info.repeated_state, "13");
}

#[test]
fn n_7_diverges_and_reports_the_iteration_limit() {
    // 7 appears to grow without bound under 5n+1. A tight limit keeps this fast
    // while still exercising the IterationLimitReached path on a *real* system.
    let t = run_with_limit("7", 1_000);
    assert_eq!(t.trajectory_status, TrajectoryStatus::IterationLimitReached);
    assert_eq!(t.iteration_count, 1_000);
    assert!(t.cycle_information.is_none());

    // Genuinely divergent, not merely slow: after 1000 steps the value dwarfs
    // anything a fixed-width integer could hold, which is why §4.5's
    // arbitrary-precision requirement is load-bearing here.
    let last = t.state_sequence.last().expect("non-empty");
    assert!(
        last.len() > 30,
        "expected explosive growth, got {} digits",
        last.len()
    );
}

#[test]
fn validation_dataset_passes() {
    let system = FiveNPlusOne;
    for case in system.validation_dataset() {
        let t = run(&case.input.raw);
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

#[test]
fn metrics_are_shared_with_collatz_and_stay_self_consistent() {
    // The extractor is shared (`systems::bigint_metrics`), so the same invariants
    // that hold for Collatz must hold here.
    let t = run("3");
    let m = &t.system_specific_metrics;
    let get = |k: &str| m.get(k).unwrap_or_else(|| panic!("metric {k} missing"));

    let odd = get("odd_count").as_u64().expect("odd u64");
    let even = get("even_count").as_u64().expect("even u64");
    assert_eq!(odd + even, t.iteration_count);

    // [3,16,8,4,2,1]: one odd step (3), four halvings.
    assert_eq!(odd, 1);
    assert_eq!(even, 4);
    assert_eq!(get("peak_value"), &serde_json::json!("16"));
    assert_eq!(get("peak_index").as_u64(), Some(1));
    assert_eq!(get("parity_sequence"), &serde_json::json!([1, 0, 0, 0, 0]));

    // Growth ratio for the single odd step: 16/3.
    let growth = get("average_growth").as_f64().expect("growth f64");
    assert!((growth - 16.0 / 3.0).abs() < 1e-12);

    // Every even step is an exact halving, in this system as in Collatz.
    let decline = get("average_decline").as_f64().expect("decline f64");
    assert!((decline - 0.5).abs() < 1e-12);
}

#[test]
fn a_non_converging_run_reports_null_total_stopping_time() {
    // Total Stopping Time is defined as "iterations required to reach 1". A cycle
    // never reaches 1, so the metric must be an explicit N/A rather than a
    // misleading number. Collatz could never exercise this path.
    let t = run("13");
    assert_eq!(
        t.system_specific_metrics.get("total_stopping_time"),
        Some(&serde_json::Value::Null)
    );
}

#[test]
fn systems_are_independent_at_the_same_initial_state() {
    // Same input, two systems, genuinely different trajectories — the engine is
    // dispatching through the trait, not special-casing anything.
    let five = run("3");
    let collatz = StateEvolutionEngine::run(
        &statelab_engine::ClassicCollatz,
        &InitialStateInput::new("3"),
        &EngineConfig::with_max_iterations(10_000),
    );
    assert_eq!(five.system_id, "five-n-plus-one");
    assert_eq!(collatz.system_id, "classic-collatz");
    assert_ne!(five.state_sequence, collatz.state_sequence);
    assert_eq!(five.iteration_count, 5);
    assert_eq!(collatz.iteration_count, 7);
}
