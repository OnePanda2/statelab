//! Classic Collatz Feature Extractor (§4.4).
//!
//! Every metric in the §4.4 table is computed here, exactly once, from the raw
//! completed state sequence. Arbitrary-precision integers are used throughout;
//! `f64` appears **only** where §4.4 explicitly calls for a ratio (growth /
//! decline / odd / even ratios) — the metric-extraction boundary permitted by §4.5.
//!
//! ## Metric-count note (flagged, not silently resolved)
//! The prose in the brief refers to "14 metrics", but the §4.4 table enumerates
//! **15** fully-defined metrics. The table is the authoritative enumeration, so
//! all 15 are implemented here (no behaviour was invented — every metric below is
//! taken verbatim from the table). The "14" label is surfaced to the human to
//! reconcile; it does not change *what* is computed.

use num_bigint::BigUint;
use num_traits::{One, ToPrimitive};
use serde_json::{json, Value};

use crate::system::{RawTrajectory, SystemMetrics};

/// Computes the full Classic Collatz metrics dictionary from a completed run.
pub(crate) fn extract(raw: &RawTrajectory<'_, BigUint>) -> SystemMetrics {
    let states = raw.states();
    let iteration_count = raw.iteration_count();

    // Defensive: the FROZEN generation order always applies at least one
    // transition before finalizing, so `states.len() >= 2` in practice. We still
    // guard the degenerate `len <= 1` case so extraction can never panic.
    let initial = match states.first() {
        Some(v) => v,
        None => return SystemMetrics::empty(),
    };

    // --- Parity sequence (evaluated on the PRE-transition value; odd=1, even=0) ---
    let parity: Vec<u8> = states
        .iter()
        .take(states.len().saturating_sub(1)) // one bit per transition
        .map(|pre| if pre.bit(0) { 1 } else { 0 })
        .collect();

    let odd_count = parity.iter().filter(|&&b| b == 1).count() as u64;
    let even_count = iteration_count - odd_count;

    // --- Peak value / index (across the full sequence, including the initial) ---
    let mut peak_value = initial;
    let mut peak_index = 0usize;
    for (idx, state) in states.iter().enumerate() {
        if state > peak_value {
            peak_value = state;
            peak_index = idx;
        }
    }

    // --- Bit-length evolution + maximum bit length ---
    let bit_lengths: Vec<u64> = states.iter().map(BigUint::bits).collect();
    let maximum_bit_length = bit_lengths.iter().copied().max().unwrap_or(0);

    // --- Binary transition statistics (consecutive bit-length deltas) ---
    let (mut increases, mut decreases, mut same) = (0u64, 0u64, 0u64);
    for pair in bit_lengths.windows(2) {
        match pair[1].cmp(&pair[0]) {
            std::cmp::Ordering::Greater => increases += 1,
            std::cmp::Ordering::Less => decreases += 1,
            std::cmp::Ordering::Equal => same += 1,
        }
    }

    // --- Run-length statistics over the parity sequence ---
    let run_lengths = run_lengths(&parity);

    // --- Stopping time: first index where the value drops below the start ---
    let stopping_time: Value = states
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, state)| *state < initial)
        .map(|(idx, _)| json!(idx as u64))
        .unwrap_or(Value::Null); // N/A (e.g. n = 1) — explicit null, not a missing key

    // --- Total stopping time: iterations to reach 1 (== iteration_count when the
    //     run converged, i.e. the final state is 1). N/A otherwise. ---
    let reached_one = states.last().map(|s| s.is_one()).unwrap_or(false);
    let total_stopping_time: Value = if reached_one {
        json!(iteration_count)
    } else {
        Value::Null
    };

    // --- Growth / decline / parity ratios (f64 at this boundary only, §4.5) ---
    let odd_ratio = ratio_or_null(odd_count, iteration_count);
    let even_ratio = ratio_or_null(even_count, iteration_count);
    let (average_growth, average_decline) = growth_decline(states, &parity);

    SystemMetrics::builder()
        .insert("stopping_time", stopping_time)
        .insert("total_stopping_time", total_stopping_time)
        .insert("peak_value", Value::String(peak_value.to_string()))
        .insert("peak_index", json!(peak_index as u64))
        .insert("odd_count", json!(odd_count))
        .insert("even_count", json!(even_count))
        .insert("odd_ratio", odd_ratio)
        .insert("even_ratio", even_ratio)
        .insert("parity_sequence", json!(parity))
        .insert("maximum_bit_length", json!(maximum_bit_length))
        .insert("bit_length_evolution", json!(bit_lengths))
        .insert(
            "binary_transition_statistics",
            json!({ "increases": increases, "decreases": decreases, "same": same }),
        )
        .insert("run_length_statistics", json!(run_lengths))
        .insert("average_growth", average_growth)
        .insert("average_decline", average_decline)
        .build()
}

/// Lengths of consecutive runs of identical parity bits.
fn run_lengths(parity: &[u8]) -> Vec<u64> {
    let mut runs = Vec::new();
    let mut iter = parity.iter();
    if let Some(&first) = iter.next() {
        let mut current = first;
        let mut len = 1u64;
        for &bit in iter {
            if bit == current {
                len += 1;
            } else {
                runs.push(len);
                current = bit;
                len = 1;
            }
        }
        runs.push(len);
    }
    runs
}

/// `numerator / denominator` as JSON `f64`, or JSON `null` when `denominator == 0`.
fn ratio_or_null(numerator: u64, denominator: u64) -> Value {
    if denominator == 0 {
        Value::Null
    } else {
        json!(numerator as f64 / denominator as f64)
    }
}

/// Mean of `next / current` over growth (odd) and decline (even) transitions.
/// Returns `(average_growth, average_decline)` as JSON `f64` (or `null` if the
/// corresponding transition class never occurred). For Classic Collatz the
/// decline mean is always exactly `0.5` — computed generically here, not hardcoded.
fn growth_decline(states: &[BigUint], parity: &[u8]) -> (Value, Value) {
    let mut growth_sum = 0.0f64;
    let mut growth_n = 0u64;
    let mut decline_sum = 0.0f64;
    let mut decline_n = 0u64;

    for (i, &bit) in parity.iter().enumerate() {
        let current = big_to_f64(&states[i]);
        let next = big_to_f64(&states[i + 1]);
        if current == 0.0 {
            continue; // unreachable for positive-integer Collatz; guards against div-by-zero
        }
        let r = next / current;
        if bit == 1 {
            growth_sum += r;
            growth_n += 1;
        } else {
            decline_sum += r;
            decline_n += 1;
        }
    }

    let growth = if growth_n == 0 {
        Value::Null
    } else {
        json!(growth_sum / growth_n as f64)
    };
    let decline = if decline_n == 0 {
        Value::Null
    } else {
        json!(decline_sum / decline_n as f64)
    };
    (growth, decline)
}

/// Lossy `BigUint -> f64`, saturating to infinity for values beyond `f64` range.
/// Only used at the ratio boundary (§4.5); the transition loop stays in `BigUint`.
fn big_to_f64(value: &BigUint) -> f64 {
    value.to_f64().unwrap_or(f64::INFINITY)
}
