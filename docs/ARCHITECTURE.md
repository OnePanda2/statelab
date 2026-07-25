# Architecture

The design is **frozen at v1.3.1** ([`PROJECT_BRIEF.md`](../PROJECT_BRIEF.md)).
This document maps that frozen architecture onto the files that implement it, so a
reader can check the two against each other.

## The layered pipeline (FROZEN)

```
UI
  ↓
Research Controller
  ↓
State Evolution Engine
  ↓
Deterministic System
  ↓
Trajectory Object
  ↓
Feature Extraction
  ↓
Visualization · Analysis · Comparison · Export
```

Visualization, Analysis, Comparison and Export are **independent consumers at the
same dependency depth** below the finalized Trajectory Object — not a sequential
runtime pipeline. What is strict is the dependency *direction*: no layer may reach
into or depend on anything below it.

## Layer → implementation

| Layer | Implementation | Isolation guarantee |
|---|---|---|
| UI | [`src/App.tsx`](../src/App.tsx), `src/components/` | Renders only; holds no engine state |
| Research Controller | [`src/controllers/ResearchController.ts`](../src/controllers/ResearchController.ts) | The **only** layer that triggers computation; contains no mathematics |
| IPC boundary | [`src/lib/invoke.ts`](../src/lib/invoke.ts) | Single seam to the host; swapping to Tauri `invoke()` touches this file alone |
| State Evolution Engine | [`crates/statelab-engine/src/engine.rs`](../crates/statelab-engine/src/engine.rs) | Generic driver; **never imports `systems/`** |
| Deterministic System | [`src/system.rs`](../crates/statelab-engine/src/system.rs) (trait), [`systems/collatz/`](../crates/statelab-engine/src/systems/collatz) | Knows nothing about the engine's internals |
| Trajectory Object | [`src/trajectory.rs`](../crates/statelab-engine/src/trajectory.rs) | Immutable once assembled |
| Feature Extraction | [`systems/collatz/metrics.rs`](../crates/statelab-engine/src/systems/collatz/metrics.rs) | Runs exactly once, at trajectory-build time |
| Cycle detection | [`cycle_detection.rs`](../crates/statelab-engine/src/cycle_detection.rs) | Fully generic via `states_equal` / `state_hash` |
| Cache | [`cache.rs`](../crates/statelab-engine/src/cache.rs) | Layered *on top* of the stateless engine |
| Migrations | [`migration.rs`](../crates/statelab-engine/src/migration.rs) | Additive-only, version-bumped |
| Visualization | [`src/visualizations/`](../src/visualizations) | Pure consumers; **may not import engine paths** |
| Analysis | [`src/modules/feature-analysis/`](../src/modules/feature-analysis) | Displays metrics; computes nothing |
| Comparison | [`src/modules/comparison-lab/`](../src/modules/comparison-lab) | Presentation math only |
| Export | [`src/modules/export-center/`](../src/modules/export-center) | Serializes; SVG generated only at export time |

## How the boundaries are enforced

These are not conventions — most are checked by a tool:

1. **The engine cannot depend on the shell.** `statelab-engine` is its own crate
   with **zero** Tauri/host dependency. The dependency can only point one way, and
   the compiler enforces it. Verify with:
   ```bash
   cargo tree -p statelab-engine
   ```
2. **The engine cannot know about a named system.** `engine.rs` never imports
   `systems/`; it drives everything through the `DeterministicSystem` trait.
3. **Visualizations cannot reach the engine.** An ESLint `no-restricted-imports`
   rule on `src/visualizations/**` blocks engine-computation imports
   ([`.eslintrc.cjs`](../.eslintrc.cjs)).
4. **The frontend never computes a trajectory field.** If a value is absent it
   renders the literal `"Metric Not Supported"` — never a substitute. Tested in
   [`FeatureAnalysis.test.tsx`](../src/modules/feature-analysis/FeatureAnalysis.test.tsx).
5. **Arbitrary precision is preserved.** `BigUint` throughout the transition loop,
   cycle detection, and comparisons; `f64` appears only at the metric-ratio and
   pixel-mapping boundaries (§4.5).

## The ordering guarantee

The Trajectory Generation Order is FROZEN and exact:

1. Generate next state (`system.transition`)
2. Check convergence (`system.is_terminated` — the system's **own** rule)
3. Check cycle detection (engine-generic)
4. Check iteration limit (engine-generic)
5. Continue

Because step 2 precedes step 3, a system whose own termination rule is met on a
step that *would also* close a cycle reports `Converged`, never `CycleDetected` —
this is why Classic Collatz reaching 1 converges. There is a dedicated regression
test using a synthetic system that hits both conditions on the same step
(`termination_beats_cycle_on_the_same_step` in
[`tests/engine_ordering.rs`](../crates/statelab-engine/tests/engine_ordering.rs)).

## Adding a new deterministic system

Principle #6 says this must require **no engine changes**. In practice:

1. Add a module under `crates/statelab-engine/src/systems/<name>/`.
2. Implement `DeterministicSystem` — state type, validation, transition,
   termination rule, equality/hash, feature extractor, validation dataset.
3. Supply a `validation_dataset()` of independently verifiable cases and test
   against it.
4. Nothing in `engine.rs`, `trajectory.rs`, `cycle_detection.rs`, or `cache.rs`
   should need to change. If it does, that is the signal to stop and report an
   architectural contradiction (§2.3) rather than to work around it.

## Host / shell

Full Tauri does not build on this machine's `windows-gnu` toolchain, so the
adopted host is [`crates/statelab-app`](../crates/statelab-app): a std-only local
server that embeds the engine, serves the production React build inlined into a
single file, and opens the browser — shipped as one double-click `StateLab.exe`.
It contains **no trajectory mathematics**, and its request/response shape matches
the planned Tauri command, so the migration is confined to `src/lib/invoke.ts` and
a thin command wrapper. See [`PACKAGING.md`](./PACKAGING.md).
