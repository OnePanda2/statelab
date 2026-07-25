//! Schema versioning & migration (§4.9).
//!
//! FROZEN rules this module enforces by construction:
//!   - Fields never change meaning, and are never removed.
//!   - Only **additive** evolution is allowed.
//!   - Any breaking change requires a migration function **and** a version bump.
//!   - Older exported datasets must remain readable by newer app versions —
//!     [`MigrationRegistry::migrate`] walks a chain of single-step migrations to
//!     bring an old JSON value forward to a target schema version.
//!
//! Migrations operate on `serde_json::Value` (not the typed [`crate::Trajectory`])
//! so that a document written against an *older* struct — which may lack fields the
//! current struct requires — can still be upgraded before it is deserialized.

use serde_json::Value;

/// Error raised while migrating a trajectory document between schema versions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrationError {
    /// Human-readable explanation.
    pub message: String,
}

impl MigrationError {
    /// Builds a migration error.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for MigrationError {}

/// A single-step, additive migration from one schema version to the next
/// (IMPLEMENTATION DECISION §4.9 — this is the trait shape).
// `from_version(&self)` trips `wrong_self_convention` (a `from_*` method usually
// takes no `self`), but this name + signature are fixed by the §4.9 migration
// trait contract, so the convention lint is deliberately allowed here.
#[allow(clippy::wrong_self_convention)]
pub trait TrajectoryMigration {
    /// The schema version this migration upgrades **from**.
    fn from_version(&self) -> &'static str;
    /// The schema version this migration upgrades **to**.
    fn to_version(&self) -> &'static str;
    /// Transforms an old document into the `to_version` shape. Must be additive:
    /// preserve every existing field and bump `trajectory_schema_version`.
    fn migrate(&self, old: Value) -> Result<Value, MigrationError>;
}

/// An ordered registry of single-step migrations. Bring a document forward with
/// [`migrate`](Self::migrate).
#[derive(Default)]
pub struct MigrationRegistry {
    migrations: Vec<Box<dyn TrajectoryMigration>>,
}

impl MigrationRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            migrations: Vec::new(),
        }
    }

    /// Registers a single-step migration.
    pub fn register(&mut self, migration: Box<dyn TrajectoryMigration>) {
        self.migrations.push(migration);
    }

    /// Number of registered migrations.
    pub fn len(&self) -> usize {
        self.migrations.len()
    }

    /// Whether the registry has no migrations.
    pub fn is_empty(&self) -> bool {
        self.migrations.is_empty()
    }

    /// Reads the `trajectory_schema_version` field from a document.
    pub fn version_of(value: &Value) -> Option<&str> {
        value.get("trajectory_schema_version")?.as_str()
    }

    /// Migrates `value` forward until its schema version equals `target`, applying
    /// registered single-step migrations in sequence. Fails if no migration exists
    /// for an intermediate version, or if the version field is missing, or if a
    /// migration fails to advance the version (loop guard).
    pub fn migrate(&self, mut value: Value, target: &str) -> Result<Value, MigrationError> {
        // At most one step per registered migration, plus a margin, bounds the loop.
        let max_steps = self.migrations.len() + 1;
        for _ in 0..max_steps {
            let current = Self::version_of(&value)
                .ok_or_else(|| {
                    MigrationError::new("document is missing `trajectory_schema_version`")
                })?
                .to_string();

            if current == target {
                return Ok(value);
            }

            let migration = self
                .migrations
                .iter()
                .find(|m| m.from_version() == current)
                .ok_or_else(|| {
                    MigrationError::new(format!(
                        "no migration registered from schema version {current}"
                    ))
                })?;

            value = migration.migrate(value)?;

            let advanced = Self::version_of(&value).ok_or_else(|| {
                MigrationError::new("migration output is missing `trajectory_schema_version`")
            })?;
            if advanced == current {
                return Err(MigrationError::new(format!(
                    "migration from {current} did not advance the schema version"
                )));
            }
        }

        Err(MigrationError::new(format!(
            "could not reach schema version {target} within the migration chain"
        )))
    }
}
