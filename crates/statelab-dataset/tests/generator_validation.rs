//! Input-validation behaviour for all 7 dataset generators (§6.2).
//!
//! The remediation brief cites validation rules from a spec addendum that is not
//! present in this repository (see OPEN_QUESTIONS.md, OQ-1). These tests therefore
//! pin down what the code **actually does**, and each one notes whether that
//! conforms to the cited rule. Every rule cited turns out to be satisfied — but it
//! is now satisfied *demonstrably*, not by assumption.

use statelab_dataset::{for_each_summary, DatasetSpec};

fn states(spec: DatasetSpec) -> Vec<String> {
    spec.states().collect()
}

/// Runs a spec and returns `(initial_state, status)` for each row.
fn statuses(spec: DatasetSpec) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for_each_summary(spec, Some(100_000), |row| {
        out.push((
            row["initial_state"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            row["status"].as_str().unwrap_or_default().to_string(),
        ));
        true
    });
    out
}

// ---- Rule: positive integers only; 0 is invalid ----

#[test]
fn range_generators_never_emit_zero() {
    // `start` is clamped to 1, so a request beginning at 0 simply starts at 1
    // rather than emitting an invalid state.
    assert_eq!(
        states(DatasetSpec::Range { start: 0, end: 3 }),
        ["1", "2", "3"]
    );
    assert_eq!(
        states(DatasetSpec::Even { start: 0, end: 6 }),
        ["2", "4", "6"],
        "0 is even but not a valid initial state, so it must not appear"
    );
    assert_eq!(
        states(DatasetSpec::Odd { start: 0, end: 5 }),
        ["1", "3", "5"]
    );
}

#[test]
fn every_generator_emits_only_positive_integers() {
    let specs = vec![
        DatasetSpec::Range { start: 1, end: 50 },
        DatasetSpec::Even { start: 1, end: 50 },
        DatasetSpec::Odd { start: 1, end: 50 },
        DatasetSpec::Primes { count: 25 },
        DatasetSpec::PowersOfTwo { count: 40 },
        DatasetSpec::Random {
            count: 50,
            max: 1000,
            seed: 7,
        },
    ];
    for spec in specs {
        for s in states(spec.clone()) {
            assert!(
                !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()),
                "{spec:?} emitted a non-digit state {s:?}"
            );
            assert_ne!(s, "0", "{spec:?} emitted 0, which is not a valid state");
        }
    }
}

#[test]
fn random_is_bounded_below_by_one() {
    // The `% max + 1` shaping means a zero draw still yields 1, never 0.
    for s in states(DatasetSpec::Random {
        count: 500,
        max: 10,
        seed: 3,
    }) {
        let n: u64 = s.parse().expect("digits");
        assert!((1..=10).contains(&n), "value {n} out of range");
    }
}

// ---- Rule: negatives / floats / scientific notation / empty are invalid ----

#[test]
fn csv_rejects_malformed_values_by_reporting_them_not_silently_dropping_them() {
    // IMPLEMENTATION NOTE: the cited rule says malformed CSV rows are
    // "skipped-and-reported". This implementation *reports* them: a malformed
    // value reaches the engine, is rejected by `validate_initial_state`, and
    // surfaces as a `SystemError` row carrying the offending input. That is
    // strictly more informative than dropping it — the user sees which value
    // failed rather than a silently shorter dataset.
    let spec = DatasetSpec::csv_from_text("3, -5, 3.5, 1e3, abc, 0, 27");
    let rows = statuses(spec);

    let by_input = |want: &str| {
        rows.iter()
            .find(|(input, _)| input == want)
            .map(|(_, status)| status.clone())
            .unwrap_or_else(|| panic!("no row for {want:?}"))
    };

    assert_eq!(by_input("3"), "Converged");
    assert_eq!(by_input("27"), "Converged");
    assert_eq!(by_input("-5"), "SystemError", "negatives are invalid");
    assert_eq!(by_input("3.5"), "SystemError", "floats are invalid");
    assert_eq!(
        by_input("1e3"),
        "SystemError",
        "scientific notation is invalid"
    );
    assert_eq!(by_input("abc"), "SystemError", "non-numeric is invalid");
    assert_eq!(by_input("0"), "SystemError", "0 is invalid");
}

#[test]
fn csv_drops_empty_tokens_entirely() {
    // Empty values are not "invalid input" to report — they are separator noise
    // (trailing commas, blank lines) and are removed during splitting.
    assert_eq!(
        states(DatasetSpec::csv_from_text("3,,27,\n\n  ,\t,6")),
        ["3", "27", "6"]
    );
    assert!(states(DatasetSpec::csv_from_text("   \n , ,\t")).is_empty());
}

// ---- Rule: duplicates allowed by default ----

#[test]
fn csv_preserves_duplicates() {
    // Duplicates are meaningful for comparison work (e.g. weighting a value), so
    // they are kept rather than de-duplicated.
    assert_eq!(
        states(DatasetSpec::csv_from_text("7,7,7")),
        ["7", "7", "7"],
        "duplicates must be allowed by default"
    );
    assert_eq!(statuses(DatasetSpec::csv_from_text("7,7,7")).len(), 3);
}

// ---- Generator-specific correctness ----

#[test]
fn powers_of_two_stay_exact_beyond_u64() {
    // 2^64 and 2^100 are past native integer range; arbitrary precision (§4.5)
    // must hold for *input generation* too, not only for the transition loop.
    let v = states(DatasetSpec::PowersOfTwo { count: 101 });
    assert_eq!(v[64], "18446744073709551616");
    assert_eq!(v[100], "1267650600228229401496703205376");
}

#[test]
fn primes_are_prime_and_ascending() {
    let v = states(DatasetSpec::Primes { count: 100 });
    assert_eq!(&v[..5], ["2", "3", "5", "7", "11"]);
    let nums: Vec<u64> = v.iter().map(|s| s.parse().expect("digits")).collect();
    for w in nums.windows(2) {
        assert!(w[1] > w[0], "primes must ascend");
    }
    for &n in &nums {
        assert!(
            (2..n).take_while(|d| d * d <= n).all(|d| n % d != 0),
            "{n} is not prime"
        );
    }
}

#[test]
fn random_is_reproducible_for_a_given_seed() {
    // Principle #2: same seed, same set, indefinitely.
    let spec = || DatasetSpec::Random {
        count: 100,
        max: 5000,
        seed: 12345,
    };
    assert_eq!(states(spec()), states(spec()));
    // A different seed must actually differ, or "seeded" would be meaningless.
    let other = DatasetSpec::Random {
        count: 100,
        max: 5000,
        seed: 999,
    };
    assert_ne!(states(spec()), states(other));
}

#[test]
fn even_and_odd_partition_the_range_exactly() {
    let evens = states(DatasetSpec::Even { start: 1, end: 100 });
    let odds = states(DatasetSpec::Odd { start: 1, end: 100 });
    assert_eq!(evens.len() + odds.len(), 100);
    for e in &evens {
        assert_eq!(e.parse::<u64>().expect("digits") % 2, 0);
    }
    for o in &odds {
        assert_eq!(o.parse::<u64>().expect("digits") % 2, 1);
    }
}

#[test]
fn an_inverted_range_yields_nothing_rather_than_panicking() {
    assert!(states(DatasetSpec::Range { start: 50, end: 10 }).is_empty());
}
