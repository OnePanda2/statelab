//! Engine benchmarks (§7.7).
//!
//! The brief names two hot paths: **the engine loop** and **cycle detection**.
//! Both are benchmarked here, plus the memoization cache (hit vs. miss), so a
//! future optimization can be judged against a baseline rather than a guess.
//!
//! These measure performance only. Correctness is owned by the test suite —
//! Principle #1 means a faster-but-wrong change is never acceptable, so a
//! regression here is a discussion, while a test failure is a hard stop.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

use statelab_engine::{
    ClassicCollatz, DeterministicSystem, EngineConfig, InitialStateInput, RawTrajectory,
    StateEvolutionEngine, SystemMetrics, TerminationReason, TrajectoryCache, TrajectoryHistory,
    ValidationCase, ValidationError, VisualizationHints,
};

/// Benchmarks the full engine loop (transition → convergence → cycle → limit)
/// plus feature extraction, for trajectories of increasing length.
fn engine_loop(c: &mut Criterion) {
    let system = ClassicCollatz;
    let config = EngineConfig::with_max_iterations(1_000_000);

    let mut group = c.benchmark_group("engine_loop");
    // 27 -> 111 iterations, 871 -> 178, 6171 -> 261: a spread of trajectory lengths.
    for n in ["27", "871", "6171"] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            let input = InitialStateInput::new(n);
            b.iter(|| StateEvolutionEngine::run(black_box(&system), black_box(&input), &config));
        });
    }
    group.finish();
}

/// Benchmarks a large arbitrary-precision start, where `BigUint` arithmetic (not
/// loop overhead) dominates — the case that would regress first if the transition
/// loop ever dropped to native integers.
fn engine_loop_bigint(c: &mut Criterion) {
    let system = ClassicCollatz;
    let config = EngineConfig::with_max_iterations(1_000_000);
    // 2^128 + 1: comfortably beyond u64/u128 native range.
    let big = "340282366920938463463374607431768211457";
    let input = InitialStateInput::new(big);

    c.bench_function("engine_loop_bigint_2pow128", |b| {
        b.iter(|| StateEvolutionEngine::run(black_box(&system), black_box(&input), &config));
    });
}

// ---- Cycle detection ----

/// A synthetic system with a long non-repeating tail followed by a cycle, so the
/// engine's generic detector must populate and probe a large visited set. This is
/// the cycle-detection hot path in isolation (Collatz never legitimately cycles).
struct TailThenCycle {
    /// Length of the non-repeating run before the cycle begins.
    tail: u64,
    /// Number of distinct states in the cycle.
    cycle_len: u64,
}

impl DeterministicSystem for TailThenCycle {
    type State = u64;

    fn system_id(&self) -> &'static str {
        "bench-tail-then-cycle"
    }
    fn system_version(&self) -> &'static str {
        "1.0.0"
    }
    fn validate_initial_state(&self, raw: &InitialStateInput) -> Result<u64, ValidationError> {
        raw.raw
            .trim()
            .parse()
            .map_err(|_| ValidationError::new("expected a u64"))
    }
    fn transition(&self, state: &u64) -> u64 {
        let next = state + 1;
        // Past the tail, wrap within the cycle band instead of growing forever.
        if next >= self.tail + self.cycle_len {
            self.tail
        } else {
            next
        }
    }
    fn is_terminated(&self, _s: &u64, _h: &TrajectoryHistory<u64>) -> Option<TerminationReason> {
        None // never terminates by its own rule -> the detector must fire
    }
    fn states_equal(&self, a: &u64, b: &u64) -> bool {
        a == b
    }
    fn state_hash(&self, s: &u64) -> u64 {
        *s
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

/// Benchmarks generic cycle detection as the visited set grows.
fn cycle_detection(c: &mut Criterion) {
    let config = EngineConfig::with_max_iterations(1_000_000);
    let input = InitialStateInput::new("0");

    let mut group = c.benchmark_group("cycle_detection");
    for tail in [1_000u64, 10_000, 50_000] {
        let system = TailThenCycle {
            tail,
            cycle_len: 16,
        };
        group.bench_with_input(BenchmarkId::from_parameter(tail), &tail, |b, _| {
            b.iter(|| StateEvolutionEngine::run(black_box(&system), black_box(&input), &config));
        });
    }
    group.finish();
}

// ---- Cache ----

/// Compares a cold cache (recompute every time) against a warm one (§4.8), which
/// is the whole point of memoization.
fn cache_hit_vs_miss(c: &mut Criterion) {
    let system = ClassicCollatz;
    let config = EngineConfig::with_max_iterations(1_000_000);
    let input = InitialStateInput::new("6171");

    let mut group = c.benchmark_group("cache");
    group.bench_function("miss_every_time", |b| {
        b.iter(|| {
            // Capacity 0 disables retention: every call recomputes.
            let mut cache = TrajectoryCache::new(0);
            cache.get_or_compute(black_box(&system), black_box(&input), &config)
        });
    });
    group.bench_function("hit_after_warm", |b| {
        let mut cache = TrajectoryCache::new(16);
        cache.get_or_compute(&system, &input, &config); // warm it
        b.iter(|| cache.get_or_compute(black_box(&system), black_box(&input), &config));
    });
    group.finish();
}

criterion_group!(
    benches,
    engine_loop,
    engine_loop_bigint,
    cycle_detection,
    cache_hit_vs_miss
);
criterion_main!(benches);
