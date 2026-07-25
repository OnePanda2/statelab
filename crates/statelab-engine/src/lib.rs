//! # StateLab Engine
//!
//! The generic, system-agnostic core of StateLab. This crate contains **no
//! Collatz-specific logic in the engine driver** — Classic Collatz is merely the
//! first built-in [`DeterministicSystem`] (see [`systems::collatz`]).
//!
//! ## Architecture (FROZEN v1.3.1)
//!
//! ```text
//! Research Controller
//!   -> State Evolution Engine   (engine.rs — generic driver)
//!     -> Deterministic System   (system.rs — the pluggable interface)
//!       -> Trajectory Object    (trajectory.rs — immutable run record)
//!         -> Feature Extraction (system.extract_features -> SystemMetrics)
//! ```
//!
//! Guiding principles this crate enforces:
//! 1. Mathematical correctness over performance.
//! 2. Reproducibility: same (system, version, config, initial state) => byte-identical Trajectory.
//! 3. The engine is the single source of truth; nothing downstream recomputes.
//! 4. Every consumer receives an **immutable** [`Trajectory`].
//! 5. Adding a new deterministic system must require **no engine changes**.
//!
//! Arbitrary-precision integers (`num-bigint`) are used everywhere in the engine
//! and system layers. Floating point is confined to the metric-extraction boundary
//! and (downstream, in the frontend) to visualization rendering — never inside the
//! transition loop, cycle detection, or state comparisons (§4.5 FROZEN).

pub mod cache;
pub mod cycle_detection;
pub mod engine;
pub mod migration;
pub mod system;
pub mod systems;
pub mod trajectory;

/// Version of this engine implementation. Embedded into every Trajectory's
/// execution metadata and every export metadata block.
pub const ENGINE_VERSION: &str = "1.0.0";

// ---- Public prelude re-exports (the exact vocabulary from Part 9 — do not alias) ----
pub use cache::{CacheKey, LruCache, TrajectoryCache};
pub use cycle_detection::{CycleHit, CycleTracker};
pub use engine::{EngineConfig, StateEvolutionEngine};
pub use migration::{MigrationError, MigrationRegistry, TrajectoryMigration};
pub use system::{
    DeterministicSystem, InitialStateInput, MetricValue, RawTrajectory, SystemMetrics,
    SystemMetricsBuilder, TerminationReason, TrajectoryHistory, ValidationCase, ValidationError,
    VisualizationHints,
};
pub use systems::collatz::ClassicCollatz;
pub use trajectory::{
    CycleInfo, ExecutionMetadata, Trajectory, TrajectoryStatus, TRAJECTORY_SCHEMA_VERSION,
};
