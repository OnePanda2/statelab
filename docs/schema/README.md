# Trajectory Schema

**Current version: `1.0.0`**

The Trajectory Object is the single immutable artifact produced by one engine run.
Its authoritative definition lives in Rust at
[`crates/statelab-engine/src/trajectory.rs`](../../crates/statelab-engine/src/trajectory.rs),
mirrored for the frontend in [`src/types/trajectory.ts`](../../src/types/trajectory.ts).
Those two must be changed together.

## Versioning rules (§4.9 FROZEN)

- Fields never change meaning.
- Fields are never removed.
- Only **additive** evolution is allowed.
- Any breaking change requires a migration function **and** a version bump.
- Older exported datasets must remain readable by newer versions of the app.

## v1.0.0 — universal fields

| Field | Type (Rust / TS) | Notes |
|---|---|---|
| `trajectory_schema_version` | `String` / `string` | `"1.0.0"` |
| `system_id` | `String` / `string` | e.g. `"classic-collatz"` |
| `system_version` | `String` / `string` | version of that system implementation |
| `initial_state` | `String` / `string` | decimal string, **BigInt-safe — never a native number** |
| `state_sequence` | `Vec<String>` / `string[]` | every state incl. the initial one, in order |
| `iteration_count` | `u64` / `number` | number of transitions applied |
| `trajectory_status` | enum | `Converged` \| `CycleDetected` \| `IterationLimitReached` \| `SystemError` |
| `termination_reason` | `String` / `string` | human-readable |
| `cycle_information` | `Option<CycleInfo>` / `CycleInfo \| null` | present **only** when `CycleDetected` |
| `execution_metadata` | object | see below |
| `system_specific_metrics` | map | system-defined; see per-system docs |

### `CycleInfo`

| Field | Type | Notes |
|---|---|---|
| `cycle_start_index` | `number` | index in `state_sequence` where the cycle begins |
| `cycle_length` | `number` | states in the cycle |
| `repeated_state` | `string` | the revisited state, canonical string form |

### `execution_metadata`

| Field | Type | Notes |
|---|---|---|
| `computation_duration_ms` | `number` | wall-clock duration, rounded to microseconds so the field round-trips exactly through JSON (§7.4) |
| `engine_version` | `string` | engine that produced it |
| `cache_hit` | `boolean` | whether it was served from the memoization cache (§4.8) |
| `iteration_limit_used` | `number` | the `max_iterations` bound in effect (default **100,000** — see PROJECT_BRIEF Addendum B.1) |
| `timestamp` | `string` | RFC 3339 UTC |
| `platform` | `string` | `<arch>-<os>` |

## Classic Collatz `system_specific_metrics`

Computed exactly once, at trajectory-build time, by the system's Feature Extractor
(§4.4). A **missing key** must render as the literal `"Metric Not Supported"`
(§5.2); a key present with `null` is a genuine N/A (e.g. stopping time for n = 1).

| Key | Type | Notes |
|---|---|---|
| `stopping_time` | `number \| null` | first index whose value is below the start; `null` if never |
| `total_stopping_time` | `number \| null` | iterations to reach 1; `null` if it never did |
| `peak_value` | `string` | decimal string (BigInt-safe) |
| `peak_index` | `number` | index of the peak in `state_sequence` |
| `odd_count` / `even_count` | `number` | transitions by pre-transition parity |
| `odd_ratio` / `even_ratio` | `number \| null` | count ÷ `iteration_count` |
| `parity_sequence` | `number[]` | one bit per transition, odd = 1 (evaluated **before** transformation) |
| `maximum_bit_length` | `number` | largest bit-length in the sequence |
| `bit_length_evolution` | `number[]` | bit-length of each state, in order |
| `binary_transition_statistics` | `{increases, decreases, same}` | consecutive bit-length deltas |
| `run_length_statistics` | `number[]` | run lengths of identical parity bits |
| `average_growth` | `number \| null` | mean `next/current` over odd transitions |
| `average_decline` | `number \| null` | mean `next/current` over even transitions (exactly `0.5` for classic Collatz, since every even step is an exact halving) |

> **Documentation note.** The brief's prose says "14 metrics" while its §4.4 table
> enumerates the **15** listed above. The table is the authoritative enumeration
> and all 15 are implemented; the "14" label is an unreconciled inconsistency in
> the source document, flagged rather than silently resolved.

## Worked example

`n = 3` → `3 → 10 → 5 → 16 → 8 → 4 → 2 → 1` (7 transitions). The full JSON is in
Appendix B of [`PROJECT_BRIEF.md`](../../PROJECT_BRIEF.md), and is asserted
field-by-field in
[`tests/collatz_validation.rs`](../../crates/statelab-engine/tests/collatz_validation.rs).

## Migrations

The registry lives in
[`crates/statelab-engine/src/migration.rs`](../../crates/statelab-engine/src/migration.rs).

```rust
pub trait TrajectoryMigration {
    fn from_version(&self) -> &'static str;
    fn to_version(&self) -> &'static str;
    fn migrate(&self, old: serde_json::Value) -> Result<serde_json::Value, MigrationError>;
}
```

Migrations operate on `serde_json::Value`, not the typed struct, so a document
written against an **older** shape can be upgraded *before* it is deserialized.
`MigrationRegistry::migrate(value, target)` walks a chain of single-step
migrations, and errors rather than guessing if no path exists.

### Adding a schema version

1. Add the new field(s) to `Trajectory` in `trajectory.rs` **and** to
   `src/types/trajectory.ts`. Additive only — never repurpose or remove a field.
2. Bump `TRAJECTORY_SCHEMA_VERSION`.
3. Implement a `TrajectoryMigration` from the previous version to the new one that
   preserves every existing field and bumps `trajectory_schema_version`.
4. Register it, and add a round-trip test (§7.4) asserting old fields survive
   unchanged — see `dummy_v1_to_v2_round_trips` in
   [`tests/cache_and_migration.rs`](../../crates/statelab-engine/tests/cache_and_migration.rs).
5. Update this document.

## Version history

| Version | Date | Change |
|---|---|---|
| `1.0.0` | 2026-07 | Initial schema. |
