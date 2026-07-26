# Performance

Correctness outranks performance (Principle #1). These numbers exist so an
optimization can be judged against a baseline — **never** so a correctness
guarantee can be traded away for speed. A benchmark regression is a discussion; a
test failure is a hard stop.

## Running the benchmarks

```bash
cargo bench -p statelab-engine --bench engine
```

Pass criterion flags after `--` for a quicker pass:

```bash
cargo bench -p statelab-engine --bench engine -- --warm-up-time 1 --measurement-time 3 --sample-size 20
```

The harness is [`crates/statelab-engine/benches/engine.rs`](../crates/statelab-engine/benches/engine.rs)
and covers the two hot paths the brief names (§7.7) — the **engine loop** and
**cycle detection** — plus the memoization cache.

## Baseline

Recorded 2026-07-25 on `x86_64-pc-windows-gnu`, release profile, criterion 0.5
(20 samples, 3 s measurement). Treat these as order-of-magnitude reference points,
not guarantees — they are machine-specific.

| Benchmark | Time (median) | Notes |
|---|---:|---|
| `engine_loop/27` | 108 µs | 111 iterations |
| `engine_loop/871` | 173 µs | 178 iterations |
| `engine_loop/6171` | 248 µs | 261 iterations |
| `engine_loop_bigint_2pow128` | 1.56 ms | 2^128 + 1 start; `BigUint` arithmetic dominates |
| `cycle_detection/1000` | 425 µs | 1 000-state tail before the cycle |
| `cycle_detection/10000` | 4.54 ms | |
| `cycle_detection/50000` | 28.1 ms | |
| `cache/miss_every_time` | 304 µs | capacity 0 — recomputes every call |
| `cache/hit_after_warm` | 47.5 µs | **~6.4× faster** than recomputing |

### Reading the numbers

- **Engine loop** scales with iteration count, as expected — roughly 1 µs per
  iteration including feature extraction.
- **Arbitrary precision costs real time.** The 2^128 case is ~6× a small-integer
  run of comparable length. This is the correct trade (§4.5 FROZEN): the loop
  stays exact, and `f64` appears only at the metrics/render boundary.
- **Cycle detection** grows roughly linearly with the visited-set size. The
  hash-indexed visited set (§4.6) is an IMPLEMENTATION DECISION, not architecture:
  if memory or time becomes a problem, a lower-footprint detector (e.g. Brent's)
  can replace it without touching any frozen layer (Appendix C, item 5).
- **The cache earns its place**: a hit is ~6.4× cheaper than a recompute, and the
  remaining cost is the Trajectory clone handed to the consumer, not mathematics.

## Iteration-limit cost — measured, not assumed

**The default is 100,000** (PROJECT_BRIEF Addendum B.1). It was briefly raised to
10,000,000 during the post-audit pass, on the strength of a spec citation that
turned out to be fabricated, then reverted. The measurements below are what
settled the engineering question independently of that citation, so they are kept.

Measured on the same machine, release profile:

| Case | Time | Note |
|---|---:|---|
| Classic Collatz, n = 27 | **0.27 ms** | converges in 111 steps; no limit is ever approached |
| Classic Collatz sweep 1..100,000 | **10.6 s** | longest orbit n = 77,031 at **350** iterations — three orders of magnitude below even the 100,000 default |
| 5n+1 from n = 7, limit 10,000 | 0.05 s | final value 319 digits |
| 5n+1 from n = 7, limit 50,000 | 1.81 s | 1,514 digits |
| 5n+1 from n = 7, limit 100,000 | 8.17 s | 3,099 digits |
| 5n+1 from n = 7, limit 200,000 | 38.9 s | 6,414 digits |

**For converging systems the limit is irrelevant.** Classic Collatz never
approaches it at any tested input — the longest orbit under 100,000 is 350 steps —
so its exact value costs nothing either way.

**For divergent systems a large limit is unreachable, which is why raising it
gained nothing.** A diverging orbit grows the state itself, so each step costs
more than the last: the timings above scale roughly **quadratically** (2× the
iterations ≈ 4.8× the time). Extrapolating, 5n+1 from n = 7 would need on the
order of **a day** of wall-clock time to reach 10,000,000 iterations. The binding
constraint on such a run is time, not the iteration count — so a higher bound buys
no additional exploration in any session a person would actually sit through.

Consequences either way:

- The Dataset Explorer's per-sweep iteration limit is **user-editable**, and
  should be lowered when exploring a system that can diverge, so one runaway item
  cannot stall an entire sweep.
- Tests that need `IterationLimitReached` must set a small explicit limit. The
  5n+1 suite uses 1,000 for exactly this reason.

## Other performance-relevant settings

- **Dataset Explorer streaming batch size** is implementer-tunable (§7.7) and
  currently processes one trajectory at a time, flushing each summary row — the
  backend holds O(1) memory regardless of dataset size (§6.2 FROZEN).
- **Cache eviction** is LRU with a configurable `cache_max_entries` bound (§4.8);
  it is exposed as config, never hardcoded.
