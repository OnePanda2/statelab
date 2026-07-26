# StateLab — Master Implementation Prompt
### A Complete Software Design Document for Claude Code

Derived from: Frozen Architecture v1.3.1 (Collatz Research Lab → StateLab design sessions)
Document version: 1.0
Audience: A Claude Code session with **zero prior context** on this project.

---

## HOW TO USE THIS DOCUMENT

Give this entire document to Claude Code at the start of a fresh session (paste it in, or save it as `PROJECT_BRIEF.md` / `CLAUDE.md` in the repo root and reference it). It is self-contained. Nothing about this project should be assumed, recalled, or invented beyond what is written here.

This document is organized into 10 parts plus 3 appendices:

1. Project Vision & Research Philosophy
2. Frozen Architecture (Read-Only)
3. Technology Stack & Repository Design
4. Core Engine Design
5. UI & Visualization System
6. Research Modules
7. Testing, Validation & Performance
8. Implementation Roadmap
9. Coding Standards
10. Claude Code Rules & First Build Task

Appendix A — Glossary
Appendix B — Fully Worked Trajectory Example (n = 3)
Appendix C — Register of Implementation Decisions

Throughout this document, blocks marked **IMPLEMENTATION DECISION** are defaults chosen to fill gaps the frozen architecture intentionally left open (tooling, algorithms, folder layout). They are not architecture. Claude Code may build against them directly. They are also collected in Appendix C so a human reviewer can override any one of them without touching anything else.

Blocks marked **FROZEN** are direct restatements of decisions already made and are not open for reinterpretation.

---

## PART 1 — PROJECT VISION & RESEARCH PHILOSOPHY

### 1.1 Origin

The project began as *shuffl*, a VLC companion app for intelligently randomizing movie playback. Research into randomness led to deterministic chaotic systems and the Collatz Conjecture, which reframed the interesting question from "can Collatz generate randomness?" to **"what mathematical information exists inside deterministic state evolution?"** That reframing is the project. It is now called **StateLab**.

### 1.2 What StateLab is — and is not

| StateLab **is** | StateLab is **not** |
|---|---|
| A deterministic state-evolution research platform | A Collatz-only application |
| A tool for observing trajectory structure, feature extraction, and nonlinear transformations across arbitrary deterministic systems | A randomness or PRNG product |
| Extensible to future deterministic systems without redesign | Making any claim about cryptographic usefulness or statistical randomness |
| Built to be mathematically correct and reproducible first | Built to be fast first |

Classic Collatz is the **first built-in deterministic system**, not the point of the application. The engine must never know it exists as a special case.

### 1.3 Guiding Principles (FROZEN)

1. **Mathematical correctness is more important than performance.** Every optimization is subordinate to correctness; never trade a correctness guarantee for speed.
2. **Reproducibility is more important than convenience.** Given the same system, version, config, and initial state, the same Trajectory Object must be producible byte-for-byte, indefinitely.
3. **Visualization never performs mathematics.** Charts and renderers are pure consumers of already-computed data. If a visualization needs a number it doesn't have, it asks for "Metric Not Supported" — it never computes one itself.
4. **The computation engine is the single source of truth.** There is exactly one place trajectories are computed. Nothing downstream recomputes or "corrects" engine output.
5. **Every module consumes immutable trajectory objects.** Once produced, a Trajectory Object is never mutated by any consumer.
6. **The architecture must support future deterministic systems without redesign.** Every design decision in Part 4 is judged against: "would adding Cellular Automata as a system require changing this?" If yes, it's wrong.
7. **Classic Collatz is only the first implementation.** Treat every Collatz-specific detail as an instance of a general interface, never as a special case baked into the engine.

### 1.4 Long-term research trajectory

The long-term objective is to investigate deterministic state evolution, feature extraction, nonlinear transformations, and trajectory analysis across systems — and to explore whether such systems might eventually inform new architectures for deterministic state-evolution engines or PRNG research. **No claims about randomness or cryptographic usefulness are made anywhere in the product, UI copy, or documentation.** The application exists to observe mathematics, not to productize a claim.

---

## PART 2 — FROZEN ARCHITECTURE (READ-ONLY)

> **Architecture status: FROZEN at v1.3.1.** Claude Code must not redesign, simplify, or reinterpret it. Implementation details not covered here (folder layout, specific algorithms, testing frameworks) are open — see the **IMPLEMENTATION DECISION** blocks throughout this document — but the layering, the interface, and the object model below are not.

### 2.1 The layered pipeline (FROZEN)

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
Visualization  ·  Analysis  ·  Comparison  ·  Export
```

**Interpretation note (flagged, not a silent assumption):** Visualization, Analysis, Comparison, and Export are three independent consumer modules sitting at the same dependency depth downstream of the finalized Trajectory Object — not a strict sequential runtime pipeline. A user can run Comparison without ever opening Visualization. What *is* strict and non-negotiable is the dependency direction: no layer may reach into or depend on anything below it in the diagram. If this reading is wrong, that's a question for the human, not something to silently reinterpret differently mid-build.

### 2.2 Per-layer responsibility and isolation boundary

| Layer | Responsibility | Must never do |
|---|---|---|
| **UI** | Collect user input, render output | Compute trajectories, hold engine state |
| **Research Controller** | Orchestrate: take UI input → call engine → receive Trajectory Object → hand off to consumers | Contain any mathematics itself |
| **State Evolution Engine** | Generic driver: run the fixed generation order (§4.1), assemble the Trajectory Object | Contain any logic specific to a named system (e.g. Collatz) |
| **Deterministic System** | Implement the system interface (§4.2) for one specific system | Know anything about the engine's internals or other systems |
| **Trajectory Object** | Immutable record of one completed run | Be mutated by any consumer, ever |
| **Feature Extraction** | Compute System-Specific Metrics via the system's Feature Extractor, finalize the Trajectory Object | Be re-run downstream (metrics are computed exactly once, at trajectory-build time) |
| **Visualization / Analysis / Comparison / Export** | Consume the finalized Trajectory Object | Perform mathematics; each must degrade gracefully to "Metric Not Supported" rather than computing a substitute |

### 2.3 Ambiguity protocol (FROZEN)

If implementation reveals a genuine mathematical or engineering contradiction in this document:

**STOP. Report it. Never invent behaviour.**

This is the only condition under which architecture may be revisited. Stylistic preference, convenience, or "a simpler way to do it" is never sufficient grounds. See Part 10 for the exact reporting format.

---

## PART 3 — TECHNOLOGY STACK & REPOSITORY DESIGN

### 3.1 Stack (FROZEN)

| Layer | Technology |
|---|---|
| Frontend | React + TypeScript + Tailwind |
| Backend | Rust |
| Desktop shell | Tauri |
| Interactive rendering | Canvas 2D (WebGL is a future migration target, not part of this build) |
| Export rendering | SVG (generated only at export time — never rendered interactively) |
| Platform | Local desktop application |

### 3.2 IPC boundary

The Research Controller lives on the frontend. It calls into the Rust backend through Tauri commands (`invoke("run_trajectory", { systemId, initialState, config })`), which drive the State Evolution Engine and return a fully finalized Trajectory Object serialized as JSON matching the schema in §4.3. The frontend never computes a trajectory field itself — if a value is missing from the JSON, the frontend renders "Metric Not Supported," it does not derive the value client-side.

### 3.3 Repository layout — **IMPLEMENTATION DECISION**

```
statelab/
├── src-tauri/                     # Rust backend (Tauri shell)
│   ├── src/
│   │   ├── main.rs
│   │   └── commands/              # Tauri IPC command handlers only —
│   │       └── mod.rs             #   thin wrappers around statelab-engine
│   ├── Cargo.toml
│   └── tauri.conf.json
│
├── crates/
│   └── statelab-engine/           # Pure Rust library, ZERO Tauri dependency
│       ├── src/
│       │   ├── lib.rs
│       │   ├── engine.rs          # State Evolution Engine (generic driver)
│       │   ├── system.rs          # DeterministicSystem trait
│       │   ├── trajectory.rs      # Trajectory Object + schema versioning
│       │   ├── cycle_detection.rs # Generic cycle detection
│       │   ├── cache.rs           # Memoization
│       │   └── systems/
│       │       └── collatz/
│       │           ├── mod.rs
│       │           ├── metrics.rs
│       │           └── validation.rs
│       └── Cargo.toml
│
├── src/                           # React frontend
│   ├── types/                     # TS mirror of the Trajectory schema
│   ├── controllers/               # Research Controller
│   ├── visualizations/
│   │   ├── value-chart/
│   │   ├── log-chart/
│   │   └── coral/
│   ├── modules/
│   │   ├── feature-analysis/
│   │   ├── dataset-explorer/
│   │   ├── comparison-lab/
│   │   └── export-center/
│   ├── components/
│   ├── hooks/
│   ├── App.tsx
│   └── main.tsx
│
├── docs/
│   └── schema/                    # Versioned Trajectory schema docs + migrations
│
├── tests/
│   └── integration/
│
├── package.json
├── vite.config.ts
├── tailwind.config.ts
└── tsconfig.json
```

**Why the engine is its own crate:** `statelab-engine` must compile and test with zero knowledge of Tauri. This makes the "only the engine computes mathematics" rule a *compiler-enforced* boundary, not a convention — the Tauri app crate can only ever call into the engine, never the reverse, and the engine is trivially portable to a future CLI or WASM build without touching the desktop shell.

### 3.4 Tooling — **IMPLEMENTATION DECISION**

| Concern | Choice |
|---|---|
| JS package manager | pnpm |
| Frontend bundler | Vite (standard Tauri pairing) |
| Rust workspace | Cargo workspace with `statelab-engine` (lib) + `src-tauri` (bin) as members |
| Rust lint/format | `rustfmt` (default config) + `clippy::all` |
| Rust arbitrary precision | `num-bigint` |
| Rust property testing | `proptest` |
| Rust benchmarking | `criterion` |
| TS lint/format | ESLint + Prettier, `strict: true` in tsconfig, no `any` |
| Frontend test runner | Vitest + React Testing Library |

None of the above is architecture. Swap any row without asking permission to "unfreeze" anything else — just note the change in Appendix C's spirit (a comment in the repo is enough; you don't need to come back to this document).

---

## PART 4 — CORE ENGINE DESIGN

### 4.1 State Evolution Engine — the generic driver (FROZEN)

The engine contains **no Collatz-specific logic anywhere**. It only knows how to drive any type implementing `DeterministicSystem`.

**Trajectory Generation Order (FROZEN, exact sequence):**

```
1. Generate next state           (system.transition)
2. Check convergence              (system.is_terminated  → system's own success condition)
3. Check cycle detection          (engine-generic, using system.states_equal / system.state_hash)
4. Check iteration limit          (engine-generic)
5. Continue (loop back to 1)
```

This order is why classic Collatz reaching 1 must report **Converged**, never **CycleDetected**: the system's own termination rule (step 2) is always checked before the engine's generic cycle detector (step 3) ever gets a chance to flag anything.

Pseudocode:

```rust
fn run<S: DeterministicSystem>(system: &S, initial_raw: &InitialStateInput, config: &EngineConfig)
    -> Trajectory
{
    let initial_state = system.validate_initial_state(initial_raw)?; // error → SystemError
    let mut history = TrajectoryHistory::new(initial_state.clone());
    let mut visited = CycleTracker::new(config.max_iterations);

    loop {
        let next = system.transition(history.current());          // 1. generate next state
        history.push(next.clone());

        if let Some(reason) = system.is_terminated(&next, &history) {
            return finalize(system, history, TrajectoryStatus::Converged, reason);  // 2.
        }

        if let Some(cycle_info) = visited.check(&next, |a, b| system.states_equal(a, b),
                                                       |s| system.state_hash(s)) {
            return finalize(system, history, TrajectoryStatus::CycleDetected, cycle_info); // 3.
        }

        if history.iteration_count() >= config.max_iterations {
            return finalize(system, history, TrajectoryStatus::IterationLimitReached, None); // 4.
        }
        // 5. continue
    }
}

fn finalize<S: DeterministicSystem>(system: &S, history: TrajectoryHistory<S::State>,
                                     status: TrajectoryStatus, term: impl Into<TerminationDetail>) -> Trajectory {
    let metrics = system.extract_features(&history.as_raw());   // Feature Extraction stage
    Trajectory::assemble(system, history, status, term, metrics) // object is now immutable
}
```

### 4.2 Deterministic System Interface (FROZEN — every member below is mandatory)

```rust
pub trait DeterministicSystem {
    /// The state representation for this system. Must support equality and hashing
    /// so the engine's generic cycle detector can use it without knowing what it is.
    type State: Clone;

    fn system_id(&self) -> &'static str;
    fn system_version(&self) -> &'static str;

    /// Parses/validates raw user input into a valid initial State, or rejects it.
    fn validate_initial_state(&self, raw: &InitialStateInput) -> Result<Self::State, ValidationError>;

    /// Produces the next state from the current one. Pure function, no side effects.
    fn transition(&self, state: &Self::State) -> Self::State;

    /// This system's own success/convergence condition (e.g. Collatz: state == 1).
    /// Returns None if not yet terminated by this system's own rule.
    fn is_terminated(&self, state: &Self::State, history: &TrajectoryHistory<Self::State>)
        -> Option<TerminationReason>;

    fn states_equal(&self, a: &Self::State, b: &Self::State) -> bool;
    fn state_hash(&self, state: &Self::State) -> u64;

    /// Computes this system's System-Specific Metrics from a completed raw trajectory.
    fn extract_features(&self, raw: &RawTrajectory<Self::State>) -> SystemMetrics;

    /// Hand-verified or otherwise authoritative cases used for regression testing.
    fn validation_dataset(&self) -> Vec<ValidationCase<Self::State>>;

    /// Optional per-system defaults for visualization (e.g. suggested Coral angles).
    /// May be None — visualizations must work correctly with no hints supplied.
    fn visualization_hints(&self) -> Option<VisualizationHints>;
}
```

The engine communicates with a system **only** through this trait. Nothing about Collatz, or any other named system, may leak into `engine.rs`.

### 4.3 Trajectory Object (FROZEN fields; JSON shape below is an IMPLEMENTATION DECISION for serialization)

**Universal fields:**

| Field | Type (Rust / TS) | Notes |
|---|---|---|
| `trajectory_schema_version` | `String` / `string` | e.g. `"1.0.0"` — see §4.9 |
| `system_id` | `String` / `string` | e.g. `"classic-collatz"` |
| `system_version` | `String` / `string` | e.g. `"1.0.0"` |
| `initial_state` | system-defined, serialized as string (BigInt-safe) | Never a native JS number |
| `state_sequence` | `Vec<String>` / `string[]` | Every state including initial, serialized as strings |
| `iteration_count` | `u64` / `number` | Number of transitions applied |
| `trajectory_status` | enum (§4.7) | Machine-readable |
| `termination_reason` | `String` / `string` | Human-readable |
| `cycle_information` | `Option<CycleInfo>` / `CycleInfo \| null` | Present only if status is `CycleDetected` |
| `execution_metadata` | object | Duration, cache hit/miss, iteration limit used, timestamp, platform |
| `system_specific_metrics` | immutable map, system-defined | Never mutated after creation; missing keys render as "Metric Not Supported," never omitted silently from the schema |

A full worked JSON example is in **Appendix B**.

### 4.4 Classic Collatz System

- **State type:** arbitrary-precision unsigned integer (`num_bigint::BigUint` in Rust; `BigInt` at the TS/JSON boundary — serialized as a decimal string, never a native number, to avoid precision loss).
- **Initial state validator:** must be a positive integer (> 0). Anything else is a `ValidationError`.
- **Transition (parity evaluated BEFORE transformation, FROZEN):**
  - odd → `3n + 1` (parity bit **1**)
  - even → `n / 2` (parity bit **0**)
- **Termination rule:** `state == 1` → `TrajectoryStatus::Converged`, reason `"Reached fixed value 1"`.
- **State equality / hash:** standard `BigUint` equality and hash.

**System-specific metrics — exact definitions (fills gaps the frozen doc named but didn't fully formalize; not a redesign):**

| Metric | Definition |
|---|---|
| Stopping Time | Index of the first iteration where the value becomes smaller than the starting number. `N/A` if it never does (e.g. n = 1). |
| Total Stopping Time | Number of iterations required to reach 1 (equals `iteration_count` for a Converged trajectory). |
| Peak Value | Maximum value across the full state sequence, including the initial state. |
| Peak Index | Index within `state_sequence` where the Peak Value occurs. |
| Odd Count | Number of transitions where the pre-transition value was odd. |
| Even Count | Number of transitions where the pre-transition value was even. |
| Odd Ratio | `Odd Count / iteration_count`. |
| Even Ratio | `Even Count / iteration_count`. |
| Parity Sequence | Ordered bit sequence, one bit per transition, evaluated before transformation (odd=1, even=0). |
| Maximum Bit Length | Largest bit-length of any state in the sequence. |
| Bit Length Evolution | Bit-length of each state in the sequence, in order. |
| Binary Transition Statistics | Count of consecutive-state bit-length increases / decreases / no-change across the sequence. |
| Run Length Statistics | Lengths of consecutive runs of identical parity bits in the Parity Sequence (list of run lengths; consumers may derive max/avg from this list). |
| Average Growth | Mean of `next / current` taken only over odd (growth) transitions. |
| Average Decline | Mean of `next / current` taken only over even (decline) transitions — always exactly `0.5` for classic Collatz, since even transitions are always `n / 2`. |

Growth/Decline ratios are computed as `f64` **only** at the metrics-extraction boundary (§4.5) — the underlying values remain `BigUint` throughout the transition loop itself.

- **Validation dataset:** hand-verified small cases below (see Appendix B for the fully worked n = 3 case), extended programmatically over a larger range (e.g. n = 1..10,000) during implementation and cross-checked internally for self-consistency (e.g. Total Stopping Time must always equal `iteration_count`; Odd Count + Even Count must always equal `iteration_count`). Do not hand-copy large published values into fixtures without independently verifying them — generate and self-check instead.
- **Visualization hints:** **IMPLEMENTATION DECISION** — leave `None` for the MVP. Coral defaults are a UI/UX decision, not an engine one; do not block on it.

### 4.5 Numeric Precision (FROZEN)

All mathematical computation uses arbitrary-precision integers everywhere in the engine and system layers (`num-bigint` in Rust, `BigInt` in JS/TS). **Floating point is permitted only inside visualization rendering** — i.e., at the moment a chart converts a state to a pixel coordinate, or a metric is expressed as a ratio for display. It must never appear inside the transition loop, cycle detection, or feature extraction's core comparisons.

### 4.6 Cycle Detection

Mandatory (FROZEN): every system supplies `states_equal` and `state_hash`; the engine performs cycle detection using these, generically.

**Algorithm — IMPLEMENTATION DECISION:** a hash-indexed visited-state set (`HashMap<u64, Vec<State>>`, resolving hash collisions via `states_equal`), bounded by `config.max_iterations` (the same bound already required for the iteration-limit check, so no additional unbounded memory growth is introduced). This is a simple, correct, generic approach that works for any system regardless of state shape; it is not a performance-optimal cycle detector (e.g. Brent's algorithm would use less memory) and may be swapped later without touching anything architectural — see Appendix C.

### 4.7 Trajectory Status & Termination Reason (FROZEN)

```rust
pub enum TrajectoryStatus {
    Converged,
    CycleDetected,
    IterationLimitReached,
    SystemError,
}
```

`Unknown` has been explicitly removed as an allowed value — every trajectory must resolve to exactly one of the four above. `termination_reason` is a free-text, human-readable string; `trajectory_status` is the machine-readable enum consumers should branch on.

### 4.8 Cache Strategy (FROZEN key; eviction policy is an IMPLEMENTATION DECISION)

Memoization key: **`(system_id, system_version, engine_config, initial_state)`**.

**Never cache using only the starting number.** Two different iteration limits, or a future config change, must not collide on cache lookup — the full tuple above is the only valid key.

Eviction policy — **IMPLEMENTATION DECISION:** LRU with a configurable max-entries bound (default left to the implementer; expose it as a config value, don't hardcode it).

### 4.9 Schema Versioning & Migration (FROZEN rules)

- Fields never change meaning.
- Fields are never removed.
- Only additive evolution is allowed.
- Any breaking change requires a migration function and a version bump.
- Older exported datasets must remain readable by newer versions of the app.

Migration function contract — **IMPLEMENTATION DECISION:**

```rust
trait TrajectoryMigration {
    fn from_version(&self) -> &'static str;
    fn to_version(&self) -> &'static str;
    fn migrate(&self, old: serde_json::Value) -> Result<serde_json::Value, MigrationError>;
}
```

---

## PART 5 — UI & VISUALIZATION SYSTEM

### 5.1 Research Controller

The only layer permitted to trigger a computation. Takes UI input, issues the Tauri `invoke()` call, receives the finalized Trajectory Object, and distributes it to whichever consumer module(s) the user has open. Holds UI-session state (which trajectories are loaded, which views are active) — never holds or recomputes trajectory math itself.

### 5.2 Visualization Contract (FROZEN)

Every visualization component receives an **immutable** Trajectory Object as input and renders from it alone. If a metric it needs is missing or null, it renders the literal string **"Metric Not Supported"** in place of that value — never blank, never a silently computed substitute, never a crash.

### 5.3 Value Chart

Linear plot: iteration index (x) vs. state value (y), rendered on Canvas 2D. Values are converted from `BigUint`/string to `f64` only inside the render call.

### 5.4 Logarithmic Chart

Same underlying data as the Value Chart, log-scale y-axis. Same BigInt→f64 conversion boundary rule applies.

### 5.5 Coral / Branch Visualization

Parameters (FROZEN):

| Parameter | Meaning |
|---|---|
| Odd Angle | Turn angle applied to a segment following an odd-parity transition |
| Even Angle | Turn angle applied to a segment following an even-parity transition |
| Line Length | Length of each drawn segment |
| Opacity | Segment opacity |
| Scale | Overall drawing scale |
| Rotation | Global rotation offset applied to the whole drawing |

**Direction Rules** (FROZEN, `Relative` is default): `Rotate Before Drawing`, `Rotate After Drawing`, `Alternating`, `Absolute`, `Relative`.

- **Relative (default):** each new segment's heading is computed relative to the *previous segment's* heading (turn angle is cumulative along the path) — not relative to fixed canvas axes.
- **Absolute:** each segment's angle is computed from a fixed canvas reference direction, ignoring the path's prior heading.
- **Alternating:** odd/even angle application alternates by some rule tied to parity — implement consistently with the Parity Sequence already present on the Trajectory Object; do not recompute parity in the visualization layer (Principle #3).
- **Rotate Before / After Drawing:** determines whether the turn is applied before or after the segment for that step is drawn.

### 5.6 Rendering rules (FROZEN)

Canvas 2D now. WebGL is an explicitly future migration path — do not build WebGL abstractions prematurely; keep the render call surface simple rather than speculatively render-target-agnostic (this is a deliberate YAGNI application of Principle #6, not a contradiction of it — "supports future systems without redesign" refers to *deterministic systems*, not render backends). **SVG is generated only during export** (§6.4) and must never be used for interactive rendering of large scenes.

---

## PART 6 — RESEARCH MODULES

### 6.1 Feature Analysis

Presents the System-Specific Metrics already computed and embedded in the Trajectory Object (§4.4 for Collatz). This module performs no computation of its own — it is a display/inspection surface over metrics that already exist.

### 6.2 Dataset Explorer

Dataset generators (FROZEN): Ranges, Random Sets, Primes, Even, Odd, Powers of Two, CSV import.

**Streaming is mandatory (FROZEN):** trajectories are processed one at a time or in small bounded batches. The full set of trajectories for a large dataset must never be held in memory simultaneously — process, extract what's needed for the current view (e.g. summary metrics), and release.

### 6.3 Comparison Lab

Supports: feature comparison across multiple trajectories, cosine similarity (over the numeric feature vectors derived from System-Specific Metrics), min-max normalization, and three overlay modes: **Raw**, **Log**, **Normalized**.

### 6.4 Export Center

Formats (FROZEN): PNG, SVG, CSV, JSON. Streaming exports for large datasets (same no-giant-in-memory-set rule as §6.2).

**Every export must embed this exact metadata block (FROZEN list):**

- Application Version
- Engine Version
- Trajectory Schema Version
- Visualization Version
- Iteration Limit
- Cycle Detection (algorithm/config used)
- Dataset Definition
- Rendering Parameters
- Timestamp
- Platform Information

Example JSON export metadata block:

```json
{
  "application_version": "0.1.0",
  "engine_version": "1.0.0",
  "trajectory_schema_version": "1.0.0",
  "visualization_version": "1.0.0",
  "iteration_limit": 100000,
  "cycle_detection": { "algorithm": "hash-indexed-visited-set", "bound": 100000 },
  "dataset_definition": { "type": "range", "start": 1, "end": 1000 },
  "rendering_parameters": { "chart": "value", "scale": 1.0 },
  "timestamp": "2026-07-24T00:00:00Z",
  "platform_information": { "os": "macos", "app_arch": "aarch64" }
}
```

---

## PART 7 — TESTING, VALIDATION & PERFORMANCE

### 7.1 Philosophy

Principle #1 (correctness over performance) makes correctness tests **non-negotiable release gates**, not a nice-to-have. No feature is "done" without a passing test against the validation dataset.

### 7.2 Engine unit tests

- Every `DeterministicSystem` implementation is tested against its own `validation_dataset()`.
- **Ordering-guarantee regression test (critical):** a system whose own termination rule is met must never report `CycleDetected` — assert this explicitly for Collatz reaching 1, and for a synthetic test system designed to reach both its own termination condition and a would-be cycle on the same step.
- BigInt/BigUint boundary tests (no silent overflow, no accidental cast to native number anywhere pre-render).
- Cycle detection tested against a synthetic system with a known, deliberately constructed cycle (not just Collatz, which never legitimately cycles for positive integers under the current conjecture-consistent test range).

### 7.3 Property-based tests (proptest)

Feed the generic engine harness synthetic systems with known analytically-determined convergence or cycle behavior, and assert the engine reports the correct `TrajectoryStatus` and `iteration_count` in all cases, across randomized configs.

### 7.4 Schema tests

Round-trip serialize/deserialize for every Trajectory Object field. Migration function tests for every registered schema version bump (§4.9).

### 7.5 Frontend tests

Visualization components tested for the "Metric Not Supported" fallback path specifically (feed a Trajectory Object with a deliberately missing metric key and assert the correct fallback renders, not a blank or a crash). Data-shape snapshot tests for chart components — not pixel-perfect rendering assertions.

### 7.6 Integration tests

Full pipeline: UI → Controller → Engine → Trajectory → Visualization, exercised for at least the Value Chart and Coral visualization end to end.

### 7.7 Performance guidelines

- Dataset Explorer streaming batch size: implementer-tunable, not hardcoded.
- Cache eviction: LRU, configurable max entries (§4.8).
- Rust benchmark harness: `criterion`, for the engine loop and cycle detection specifically — these are the hot paths.

---

## PART 8 — IMPLEMENTATION ROADMAP

Each phase has an explicit Definition of Done (DoD). Do not begin a phase until the previous one's DoD is met.

| Phase | Scope | Definition of Done |
|---|---|---|
| 0 | Repo scaffold, tooling, empty Tauri shell | `pnpm install && cargo build` succeeds; lint configs in place; empty window launches |
| 1 | `DeterministicSystem` trait, generic engine, Trajectory Object, Classic Collatz, full unit/validation tests. **No UI.** | `cargo test` passes; `cargo clippy` clean; `statelab-engine` has zero Tauri dependency |
| 2 | Cycle detection + cache layer + schema versioning scaffolding | Synthetic-cycle tests pass; cache hit/miss verified by test; a dummy v1→v2 migration round-trips in a test |
| 3 | Research Controller + Tauri IPC wiring + minimal UI shell (input box → raw JSON dump) | Entering a number in the app produces the correct Trajectory JSON on screen, no charts yet |
| 4 | Value Chart + Logarithmic Chart | Both render correctly for a known trajectory; BigInt→f64 conversion confined to render call |
| 5 | Coral / Branch Visualization | All parameters + all 5 direction rules functional and manually verified against Appendix B's worked example |
| 6 | Feature Analysis module | All 14 Collatz metrics visible in UI, sourced only from the Trajectory Object |
| 7 | Dataset Explorer | Streaming confirmed (no full-dataset in-memory hold) for all 7 generator types |
| 8 | Comparison Lab | Cosine similarity + normalization + 3 overlay modes functional across ≥2 trajectories |
| 9 | Export Center | All 4 formats produce files containing the full FROZEN metadata block |
| 10 | Polish, performance pass, docs, packaging | Tauri bundler produces installable desktop artifacts; docs in `docs/schema/` describe current schema version |

---

## PART 9 — CODING STANDARDS

**Rust**
- `rustfmt` default config; `clippy::all` clean (explicit `#[allow(...)]` with a comment justifying it is acceptable, silent suppression is not).
- `Result`-based error handling throughout; no `unwrap()`/`expect()` outside test code.
- Doc comments (`///`) on every public trait, struct, and function.
- One concern per module; `engine.rs` never imports anything from `systems/`.

**TypeScript**
- `strict: true`; no `any`.
- Functional components + hooks only; no class components.
- ESLint + Prettier enforced; colocate component tests next to components.
- Import-boundary lint rule: visualization files may not import anything from an engine-computation path — enforce this the same way `engine.rs`/`systems/` isolation is enforced on the Rust side.

**Naming**
Use this document's exact vocabulary consistently across Rust and TypeScript: `DeterministicSystem`, `Trajectory`, `StateEvolutionEngine`, `ResearchController`, `TrajectoryStatus`, `SystemMetrics`. Do not invent synonyms (no `Run`, no `Simulation`, no `Result` in place of `Trajectory`).

**Commits**
Conventional Commits (`feat:`, `fix:`, `test:`, `docs:`, `refactor:`), scoped by roadmap phase/module, e.g. `feat(engine): implement generic cycle detection (#2)`.

---

## PART 10 — CLAUDE CODE RULES & FIRST BUILD TASK

### 10.1 Ground rules

1. The architecture in Parts 1–7 is **frozen**. Do not redesign it, simplify it, or "improve" it. Every unspecified implementation detail has already been filled with an explicit **IMPLEMENTATION DECISION** — use those as given.
2. If you hit something genuinely ambiguous or mathematically/engineering contradictory that is **not** already resolved by an IMPLEMENTATION DECISION block: **STOP. Do not guess. Do not invent behavior.** Report it using the format in §10.3.
3. Escalate only for genuine contradictions — never for stylistic preference or "I'd have done it differently."
4. Build phases strictly in the order given in Part 8. Do not skip ahead to UI work before Phase 1's engine tests are green.

### 10.2 First build task

**Scope: exactly Phase 0 + Phase 1 from the roadmap. Nothing else.**

1. Scaffold the repository exactly per §3.3.
2. Implement `DeterministicSystem` (§4.2), the generic `State Evolution Engine` (§4.1), and the `Trajectory Object` (§4.3) precisely as specified.
3. Implement Classic Collatz (§4.4) with all 14 System-Specific Metrics.
4. Write unit tests covering: the hand-verified n = 1, 2, 3 cases (Appendix B has n = 3 fully worked), a programmatically-generated and internally self-consistency-checked set for a larger range (e.g. n = 1..10,000), and the ordering-guarantee regression test from §7.2.
5. **No UI. No Tauri command wiring beyond a stub.**

**Definition of Done:** `cargo test` passes, `cargo clippy` is clean, and `statelab-engine` compiles and tests with zero Tauri dependency.

### 10.3 Blocker reporting format

When Part 10 Rule 2 applies, report using this template rather than proceeding:

```
BLOCKED — architecture ambiguity

Layer:        <which layer/section of this document>
Question:     <the specific, narrow question>
Why it blocks implementation: <what you cannot proceed without deciding>
Options considered: <if any>
```

Do not pick one of the options yourself and move on — wait for a response.

---

## APPENDIX A — GLOSSARY

| Term | Meaning |
|---|---|
| **Deterministic System** | A pluggable implementation of the system interface (§4.2) — Collatz is the first, not the only, instance |
| **State Evolution Engine** | The generic driver that runs any Deterministic System through the fixed generation order |
| **Trajectory Object** | The single immutable output artifact of one engine run |
| **System-Specific Metrics** | The immutable metrics dictionary embedded in a Trajectory Object, defined per-system by its Feature Extractor |
| **Research Controller** | The orchestration layer between UI and engine; the only layer allowed to trigger computation |
| **Parity Sequence** | Ordered odd/even bits for a Collatz trajectory, evaluated before each transformation |
| **Stopping Time** vs **Total Stopping Time** | First iteration below the starting value, vs. iterations to reach 1 — two distinct, both-tracked metrics |

## APPENDIX B — FULLY WORKED TRAJECTORY EXAMPLE (n = 3)

Sequence: `3 → 10 → 5 → 16 → 8 → 4 → 2 → 1` (7 transitions).

| Metric | Value |
|---|---|
| state_sequence | `["3","10","5","16","8","4","2","1"]` |
| iteration_count | 7 |
| trajectory_status | Converged |
| Stopping Time | 6 (first value below 3 is `2`, at transition index 6) |
| Total Stopping Time | 7 |
| Peak Value | 16 |
| Peak Index | 3 |
| Odd Count / Even Count | 2 / 5 |
| Odd Ratio / Even Ratio | 2/7 ≈ 0.2857 / 5/7 ≈ 0.7143 |
| Parity Sequence | `[1,0,1,0,0,0,0]` |
| Bit Length Evolution | `[2,4,3,5,4,3,2,1]` |
| Maximum Bit Length | 5 |
| Binary Transition Statistics | increases: 2, decreases: 5, same: 0 |
| Run Length Statistics | `[1,1,1,4]` (runs within the parity sequence) |
| Average Growth | (10/3 + 16/5) / 2 ≈ 3.2667 |
| Average Decline | 0.5 (exact, always, for classic Collatz) |

Full JSON:

```json
{
  "trajectory_schema_version": "1.0.0",
  "system_id": "classic-collatz",
  "system_version": "1.0.0",
  "initial_state": "3",
  "state_sequence": ["3","10","5","16","8","4","2","1"],
  "iteration_count": 7,
  "trajectory_status": "Converged",
  "termination_reason": "Reached fixed value 1",
  "cycle_information": null,
  "execution_metadata": {
    "computation_duration_ms": 0.02,
    "engine_version": "1.0.0",
    "cache_hit": false,
    "iteration_limit_used": 100000,
    "timestamp": "2026-07-24T00:00:00Z",
    "platform": "aarch64-apple-darwin"
  },
  "system_specific_metrics": {
    "stopping_time": 6,
    "total_stopping_time": 7,
    "peak_value": "16",
    "peak_index": 3,
    "odd_count": 2,
    "even_count": 5,
    "odd_ratio": 0.2857142857142857,
    "even_ratio": 0.7142857142857143,
    "parity_sequence": [1,0,1,0,0,0,0],
    "maximum_bit_length": 5,
    "bit_length_evolution": [2,4,3,5,4,3,2,1],
    "binary_transition_statistics": { "increases": 2, "decreases": 5, "same": 0 },
    "run_length_statistics": [1,1,1,4],
    "average_growth": 3.2666666666666666,
    "average_decline": 0.5
  }
}
```

## APPENDIX C — REGISTER OF IMPLEMENTATION DECISIONS

Every one of these fills a gap the frozen architecture deliberately left open. None of them requires "unfreezing" anything else if changed.

1. Repository layout (§3.3)
2. Tooling: pnpm, Vite, `num-bigint`, `proptest`, `criterion`, ESLint/Prettier (§3.4)
3. Precise formulas for the 14 Collatz System-Specific Metrics (§4.4)
4. Collatz Optional Visualization Hints left as `None` for MVP (§4.4)
5. Cycle detection algorithm: hash-indexed visited-state set (§4.6)
6. Cache eviction policy: LRU, configurable bound (§4.8)
7. Migration function trait shape (§4.9)
8. Dataset Explorer streaming batch size: implementer-tunable (§7.7)
9. Interpretation of the pipeline diagram as dependency-layering rather than strict runtime sequence (§2.1)

---

## ADDENDUM A — 2026-07-26 — Post-audit clarifications

**This section is additive.** Nothing above it has been altered. It records
decisions taken during the post-audit remediation pass, following the same
superseding-addendum pattern the spec used across its own v1.1/1.2/1.3
revisions.

### A.1 Default iteration limit (clarifies §4.1)

The frozen text never states a default `max_iterations`; 100,000 appears only
inside *example* payloads in Appendix B and §6.4, which illustrate output shape
rather than mandate a default.

**Decision:** the default is **10,000,000** (IMPLEMENTATION DECISION, §4.1).
The example payloads above remain valid as examples — they show an explicitly
configured limit, not the default. Classic Collatz is unaffected in practice
(it converges in hundreds of steps); the limit is only ever *reached* by
non-converging runs, which is exactly where a larger budget is useful.

### A.2 Every trajectory applies at least one transition (clarifies §4.1)

The FROZEN Trajectory Generation Order checks convergence at **step 2**, after
the transition at step 1. A system whose *initial* state already satisfies its
own termination rule therefore cannot report that immediately: it transitions
at least once first.

For Classic Collatz this means `n = 1` yields `1 → 4 → 2 → 1`
(`iteration_count = 3`), not a zero-iteration result. The same applies to 5n+1
(Addendum A.4) and to any future system whose initial state can be a fixed
point.

**Decision:** this is **intentional and permanent**, not an accident of
ordering. Stating it explicitly makes the ordering guarantee (§7.2 — convergence
is checked before cycle detection) uniform, and avoids the awkward downstream
edge case of a zero-iteration trajectory (empty parity sequence, null stopping
time, a single-point chart).

Recorded as OQ-2 in `OPEN_QUESTIONS.md`. The alternative — adding a "step 0"
initial-state check — would change the frozen §4.1 order and remains available
as a future, signed-off, versioned change. It has **not** been implemented.

### A.3 Coral parameters beyond the frozen six (extends §5.5)

§5.5 lists six parameters (Odd Angle, Even Angle, Line Length, Opacity, Scale,
Rotation) plus the five Direction Rules. The following were added afterwards and
are **additions, not reinterpretations** — every frozen parameter and rule
behaves exactly as before:

| Addition | Default | Backward compatibility |
|---|---|---|
| Line Width | `1.2` | Reproduces the previously hardcoded 1.2 (analytical) and 0.7 (aesthetic) exactly |
| Odd/Even Color | `#f0883e` / `#3fb950` | The previously hardcoded constants |
| Center Offset | `(0, 0)` | `(0,0)` is pixel-identical to the previous behaviour, enforced by test |
| Animation Speed | `null` (instant) | Default is the original instant, single-pass render with no animation frame loop |
| `aesthetic` Direction Rule | — | A sixth rule; the five frozen rules are untouched |

### A.4 Second deterministic system: 5n+1 (exercises Principle #6)

A second built-in system (`five-n-plus-one`: odd → `5n+1`, even → `n/2`,
terminating at `state == 1`) was added to test Principle #6 against something
real rather than a synthetic double.

**Result: Principle #6 holds.** It required *zero* changes to `engine.rs`,
`system.rs`, `trajectory.rs`, `cycle_detection.rs` or `cache.rs`. See
`docs/AUDIT_REMEDIATION.md` for the evidence.

Unlike Classic Collatz, 5n+1 reaches all three non-error terminal statuses
(converges from n = 3, cycles from n = 13, appears to diverge from n = 7), so it
is a materially better regression surface for the generic engine.

### A.5 Unresolved: the `average_decline` formula

An external audit reported that an addendum defines
`Average Decline = mean(Current / Next)`, giving `2.0` for Classic Collatz.
**No such addendum exists in this repository**, and §4.4 and Appendix B above
both specify `next / current` and state the value is "always exactly 0.5".

The implementation continues to match §4.4 and Appendix B. Changing it would
make the code contradict the frozen text, which `Document 1.txt` §6 forbids.
Recorded as **OQ-1** in `OPEN_QUESTIONS.md`, awaiting either the missing
addendum or a decision to keep the current definition. **No code was changed.**

---

## ADDENDUM B — 2026-07-26 — Supersedes parts of Addendum A

**This section is additive and supersedes A.1, A.3 and A.5.** Addendum A is left
in place rather than rewritten, following the same superseding pattern used
throughout this specification's own revision history — the record of what was
decided, and why it was withdrawn, is more useful than a clean file.

### B.0 Why Addendum A is partly withdrawn

Addendum A was written during a post-audit remediation pass driven by an external
audit report. That report repeatedly cited an addendum to this document defining,
among other things, `Average Decline = mean(Current / Next)`, a default
`max_iterations` of 10,000,000, ten Coral parameters, and an eight-feature
comparison vector including "Trajectory Length".

**No such addendum has ever existed.** The project owner confirmed on 2026-07-26
that the auditor fabricated it. A full-repository search found zero occurrences of
every one of those values.

The changes those citations motivated are therefore withdrawn, except where they
stand on independent merit.

### B.1 Supersedes A.1 — default iteration limit

**The default `max_iterations` is 100,000**, not 10,000,000.

A.1's reasoning was sound in one respect: this document never *states* a default,
so the value is an IMPLEMENTATION DECISION either way. But the only figure the
specification shows anywhere is 100,000 (§6.4 and Appendix B example payloads),
and the sole reason to depart from it was the fabricated citation.

Measurement also showed the higher bound bought nothing: a divergent orbit grows
the state itself, so cost scales roughly quadratically and 10,000,000 iterations
is unreachable in practice — on the order of a day of wall-clock time. See
`docs/PERFORMANCE.md`.

### B.2 Supersedes A.3 — Coral parameters

**§5.5's six parameters stand as written and marked FROZEN.** The four additions
listed in A.3 — Line Width, Odd/Even Colour, Centre Offset, Animation Speed —
have been **removed**. Line Width and Colour reverted to the constants they were
before; Centre Offset and Animation Speed are gone entirely.

A.3 characterised these as "additions, not reinterpretations". That framing was
too convenient: §5.5 introduces its table with the words "Parameters (FROZEN)",
which reads at least as naturally as *"the parameter set is frozen at these six"*
as it does *"these six behave as described"*. Choosing the permissive reading
silently, on fabricated authority, was the wrong call.

**Unaffected:** the `aesthetic` direction rule remains. It was added at the
project owner's explicit request in a separate session, was flagged as an
extension of the frozen five at the time, and does not derive from the audit.

### B.3 Supersedes A.5 — `average_decline`

**Resolved, not unresolved.** `Average Decline` is `mean(next / current)` — the
value is exactly `0.5` for Classic Collatz, precisely as §4.4 and Appendix B
state. The implementation always matched and was never changed.

### B.4 Unaffected by this withdrawal

The following stand on their own merits, independent of the fabricated citation:

- **A.2** — "every trajectory applies at least one transition". A real property of
  the frozen §4.1 generation order, observed independently and confirmed by the
  5n+1 system exhibiting it identically. Remains documented; OQ-2 remains open.
- **A.4** — the 5n+1 system. Document 1.txt §2 explicitly names "5n + 1" as a
  target future system, and §10 makes pluggability a Definition of Success. It
  validated Principle #6 and is the only real exercise of `CycleDetected` and
  `IterationLimitReached`. **Retained.**
- CI, the verification test suites, and the trajectory-duration rounding fix —
  none of which depended on the audit's spec claims.
