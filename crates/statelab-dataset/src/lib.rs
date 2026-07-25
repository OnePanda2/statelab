//! Dataset Explorer generators + streaming (§6.2), shared by every StateLab host.
//!
//! **Streaming is mandatory (FROZEN):** trajectories are processed one at a time.
//! Every generator is a **lazy iterator** of initial-state strings, and
//! [`for_each_summary`] runs each state through the engine, hands a compact
//! summary row to the caller, and drops the trajectory before advancing — so the
//! full set is never held in memory (O(1) in dataset size, plus one in-flight
//! trajectory).
//!
//! Generating initial states (ranges, primes, …) is host-side **input
//! generation**, not trajectory mathematics — the engine remains the single source
//! of truth for the latter (Principle #4). This crate lives outside the engine
//! precisely so that boundary stays visible.

use std::io::{self, Write};

use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use serde_json::json;
use statelab_engine::{
    ClassicCollatz, EngineConfig, InitialStateInput, StateEvolutionEngine, Trajectory,
};

/// Hard cap on how many items any single dataset stream will process, so an
/// unbounded range cannot run forever. The stream simply stops after this many.
pub const MAX_DATASET_ITEMS: usize = 200_000;

/// The 7 FROZEN dataset generators (§6.2).
///
/// Serialized internally tagged (`{"type": "powers-of-two", "count": 32}`) so a
/// host can accept it directly as a typed command argument.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum DatasetSpec {
    Range { start: u64, end: u64 },
    Random { count: u64, max: u64, seed: u64 },
    Primes { count: u64 },
    Even { start: u64, end: u64 },
    Odd { start: u64, end: u64 },
    PowersOfTwo { count: u32 },
    Csv { values: Vec<String> },
}

impl DatasetSpec {
    /// Splits raw CSV text (comma / whitespace / newline separated) into a spec.
    pub fn csv_from_text(raw: &str) -> Self {
        Self::Csv {
            values: raw
                .split([',', '\n', '\r', ' ', '\t'])
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
        }
    }

    /// Produces the lazy iterator of initial-state strings. Values are yielded on
    /// demand — nothing is materialized up front.
    pub fn states(self) -> Box<dyn Iterator<Item = String>> {
        match self {
            Self::Range { start, end } => Box::new((start.max(1)..=end).map(|n| n.to_string())),
            Self::Even { start, end } => Box::new(
                (start.max(1)..=end)
                    .filter(|n| n.is_multiple_of(2))
                    .map(|n| n.to_string()),
            ),
            Self::Odd { start, end } => Box::new(
                (start.max(1)..=end)
                    .filter(|n| !n.is_multiple_of(2))
                    .map(|n| n.to_string()),
            ),
            Self::PowersOfTwo { count } => {
                Box::new((0..count).map(|k| (BigUint::from(1u32) << k).to_string()))
            }
            Self::Primes { count } => {
                Box::new(PrimeIter::new().take(count as usize).map(|p| p.to_string()))
            }
            Self::Random { count, max, seed } => Box::new(
                Xorshift64::new(seed)
                    .take(count as usize)
                    .map(move |x| (x % max.max(1) + 1).to_string()),
            ),
            Self::Csv { values } => Box::new(values.into_iter()),
        }
    }
}

/// The compact per-trajectory summary row (never the full state sequence).
pub fn summarize(t: &Trajectory) -> serde_json::Value {
    let m = &t.system_specific_metrics;
    json!({
        "initial_state": t.initial_state,
        "iteration_count": t.iteration_count,
        "status": t.trajectory_status,
        "peak_value": m.get("peak_value"),
        "stopping_time": m.get("stopping_time"),
        "total_stopping_time": m.get("total_stopping_time"),
        "odd_count": m.get("odd_count"),
        "even_count": m.get("even_count"),
        "maximum_bit_length": m.get("maximum_bit_length"),
    })
}

/// Core streaming primitive: runs every generated initial state through the
/// engine and hands each summary row to `sink` as it is produced. Returns the
/// number of items processed.
///
/// Only one [`Trajectory`] exists at a time — it is summarized and dropped before
/// the next state is generated. `sink` returns `false` to stop early (a closed
/// connection, a cancelled request).
pub fn for_each_summary(
    spec: DatasetSpec,
    max_iterations: Option<u64>,
    mut sink: impl FnMut(serde_json::Value) -> bool,
) -> u64 {
    let system = ClassicCollatz;
    let config = match max_iterations {
        Some(max) => EngineConfig::with_max_iterations(max),
        None => EngineConfig::default(),
    };

    let mut processed = 0u64;
    for state in spec.states().take(MAX_DATASET_ITEMS) {
        let trajectory =
            StateEvolutionEngine::run(&system, &InitialStateInput::new(state), &config);
        let row = summarize(&trajectory);
        processed += 1;
        if !sink(row) {
            break;
        }
        // `trajectory` is dropped here — never accumulated.
    }
    processed
}

/// Writes one NDJSON summary line per trajectory to `out`, flushing after each so
/// a client consumes the stream incrementally.
pub fn stream_dataset<W: Write>(
    spec: DatasetSpec,
    max_iterations: Option<u64>,
    out: &mut W,
) -> io::Result<u64> {
    let mut error: Option<io::Error> = None;
    let processed = for_each_summary(spec, max_iterations, |row| {
        let mut write = || -> io::Result<()> {
            out.write_all(row.to_string().as_bytes())?;
            out.write_all(b"\n")?;
            out.flush()
        };
        match write() {
            Ok(()) => true,
            Err(e) => {
                error = Some(e);
                false
            }
        }
    });
    match error {
        Some(e) => Err(e),
        None => Ok(processed),
    }
}

/// A lazy, seeded xorshift64 generator — deterministic per seed, so Random Sets
/// are reproducible (Principle #2).
struct Xorshift64 {
    state: u64,
}

impl Xorshift64 {
    fn new(seed: u64) -> Self {
        Self {
            // A zero seed would stay zero forever; nudge it to a fixed nonzero value.
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }
}

impl Iterator for Xorshift64 {
    type Item = u64;
    fn next(&mut self) -> Option<u64> {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        Some(x)
    }
}

/// A lazy iterator over the primes, ascending, by trial division.
struct PrimeIter {
    candidate: u64,
}

impl PrimeIter {
    fn new() -> Self {
        Self { candidate: 1 }
    }
}

impl Iterator for PrimeIter {
    type Item = u64;
    fn next(&mut self) -> Option<u64> {
        loop {
            self.candidate += 1;
            if is_prime(self.candidate) {
                return Some(self.candidate);
            }
        }
    }
}

fn is_prime(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    if n.is_multiple_of(2) {
        return n == 2;
    }
    let mut d = 3u64;
    while d.saturating_mul(d) <= n {
        if n.is_multiple_of(d) {
            return false;
        }
        d += 2;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(spec: DatasetSpec) -> Vec<String> {
        spec.states().collect()
    }

    #[test]
    fn range_is_inclusive_and_skips_zero() {
        assert_eq!(
            collect(DatasetSpec::Range { start: 1, end: 5 }),
            ["1", "2", "3", "4", "5"]
        );
        assert_eq!(
            collect(DatasetSpec::Range { start: 0, end: 3 }),
            ["1", "2", "3"]
        );
    }

    #[test]
    fn even_and_odd_filter_correctly() {
        assert_eq!(
            collect(DatasetSpec::Even { start: 1, end: 10 }),
            ["2", "4", "6", "8", "10"]
        );
        assert_eq!(
            collect(DatasetSpec::Odd { start: 1, end: 10 }),
            ["1", "3", "5", "7", "9"]
        );
    }

    #[test]
    fn powers_of_two_use_arbitrary_precision() {
        assert_eq!(
            collect(DatasetSpec::PowersOfTwo { count: 5 }),
            ["1", "2", "4", "8", "16"]
        );
        // 2^100 exceeds u64 — confirms BigUint is used, not a native integer.
        let big = collect(DatasetSpec::PowersOfTwo { count: 101 });
        assert_eq!(big[100], "1267650600228229401496703205376");
    }

    #[test]
    fn primes_are_the_first_primes() {
        assert_eq!(
            collect(DatasetSpec::Primes { count: 5 }),
            ["2", "3", "5", "7", "11"]
        );
    }

    #[test]
    fn random_is_bounded_and_reproducible() {
        let spec = || DatasetSpec::Random {
            count: 20,
            max: 100,
            seed: 42,
        };
        assert_eq!(collect(spec()), collect(spec()), "same seed must reproduce");
        for v in collect(spec()) {
            let n: u64 = v.parse().unwrap();
            assert!((1..=100).contains(&n), "value {n} out of range");
        }
    }

    #[test]
    fn csv_splits_and_trims() {
        assert_eq!(
            collect(DatasetSpec::csv_from_text("3, 27,\n 6 ,,7")),
            ["3", "27", "6", "7"]
        );
    }

    #[test]
    fn stream_writes_one_ndjson_line_per_item() {
        let mut buf: Vec<u8> = Vec::new();
        let count = stream_dataset(
            DatasetSpec::Range { start: 1, end: 3 },
            Some(100_000),
            &mut buf,
        )
        .expect("stream ok");
        assert_eq!(count, 3);
        let text = String::from_utf8(buf).unwrap();
        assert_eq!(text.lines().count(), 3);
        for line in text.lines() {
            let v: serde_json::Value = serde_json::from_str(line).expect("valid json line");
            assert!(v.get("initial_state").is_some());
            assert!(v.get("iteration_count").is_some());
        }
    }

    #[test]
    fn sink_can_stop_the_stream_early() {
        let mut seen = 0;
        let processed = for_each_summary(
            DatasetSpec::Range {
                start: 1,
                end: 1000,
            },
            None,
            |_| {
                seen += 1;
                seen < 5 // stop after the 5th row
            },
        );
        assert_eq!(seen, 5);
        assert_eq!(processed, 5);
    }

    #[test]
    fn spec_round_trips_through_serde() {
        let spec = DatasetSpec::PowersOfTwo { count: 32 };
        let json = serde_json::to_string(&spec).unwrap();
        assert!(json.contains("powers-of-two"));
        let back: DatasetSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(spec, back);
    }
}
