//! Phase 2 tests: memoization cache (§4.8) and schema migration (§4.9).

use serde_json::{json, Value};

use statelab_engine::{
    ClassicCollatz, EngineConfig, InitialStateInput, MigrationError, MigrationRegistry,
    TrajectoryCache, TrajectoryMigration,
};

fn input(n: u64) -> InitialStateInput {
    InitialStateInput::new(n.to_string())
}

#[test]
fn cache_miss_then_hit() {
    let system = ClassicCollatz;
    let config = EngineConfig::with_max_iterations(1_000_000);
    let mut cache = TrajectoryCache::from_config(&config);

    // First call: a miss — computed fresh.
    let first = cache.get_or_compute(&system, &input(27), &config);
    assert!(
        !first.execution_metadata.cache_hit,
        "first call must be a miss"
    );
    assert_eq!(cache.len(), 1);

    // Second identical call: a hit — same maths, cache_hit flag flipped.
    let second = cache.get_or_compute(&system, &input(27), &config);
    assert!(
        second.execution_metadata.cache_hit,
        "second call must be a hit"
    );

    // Mathematical content is byte-identical between miss and hit.
    assert_eq!(first.state_sequence, second.state_sequence);
    assert_eq!(first.iteration_count, second.iteration_count);
    assert_eq!(
        first.system_specific_metrics,
        second.system_specific_metrics
    );
    assert_eq!(cache.len(), 1, "a hit must not add a new entry");
}

#[test]
fn equivalent_inputs_share_one_entry() {
    let system = ClassicCollatz;
    let config = EngineConfig::default();
    let mut cache = TrajectoryCache::new(16);

    let a = cache.get_or_compute(&system, &InitialStateInput::new("27"), &config);
    let b = cache.get_or_compute(&system, &InitialStateInput::new("  27 "), &config);
    assert!(!a.execution_metadata.cache_hit);
    assert!(
        b.execution_metadata.cache_hit,
        "canonicalized input must hit the same entry"
    );
    assert_eq!(cache.len(), 1);
}

#[test]
fn cache_key_includes_config() {
    // The FROZEN key includes engine_config, so a different iteration limit must
    // NOT collide with an existing entry (§4.8 — never key on the number alone).
    let system = ClassicCollatz;
    let mut cache = TrajectoryCache::new(16);

    let config_a = EngineConfig::with_max_iterations(1_000_000);
    let config_b = EngineConfig::with_max_iterations(500); // different limit -> different key

    let a = cache.get_or_compute(&system, &input(27), &config_a);
    assert!(!a.execution_metadata.cache_hit);

    let b = cache.get_or_compute(&system, &input(27), &config_b);
    assert!(
        !b.execution_metadata.cache_hit,
        "different engine_config must be a distinct cache key, not a collision"
    );
    assert_eq!(cache.len(), 2, "the two configs must occupy two entries");
}

#[test]
fn lru_evicts_least_recently_used() {
    let system = ClassicCollatz;
    let config = EngineConfig::with_max_iterations(1_000_000);
    let mut cache = TrajectoryCache::new(2); // capacity 2

    cache.get_or_compute(&system, &input(3), &config); // [3]
    cache.get_or_compute(&system, &input(6), &config); // [3, 6]
                                                       // Touch 3 so 6 becomes the LRU.
    let three_hit = cache.get_or_compute(&system, &input(3), &config);
    assert!(three_hit.execution_metadata.cache_hit);

    // Inserting 7 exceeds capacity -> evicts 6 (the LRU).
    cache.get_or_compute(&system, &input(7), &config); // [3, 7]
    assert_eq!(cache.len(), 2);

    // 6 was the LRU and was evicted -> it now recomputes as a miss, proving
    // eviction happened. (Re-inserting 6 in turn evicts 3, the new LRU.)
    let six = cache.get_or_compute(&system, &input(6), &config);
    assert!(
        !six.execution_metadata.cache_hit,
        "6 should have been evicted"
    );
    let three = cache.get_or_compute(&system, &input(3), &config);
    assert!(
        !three.execution_metadata.cache_hit,
        "3 became the LRU and should have been evicted by re-inserting 6"
    );
    assert_eq!(cache.len(), 2);
}

#[test]
fn invalid_input_is_not_cached() {
    let system = ClassicCollatz;
    let config = EngineConfig::default();
    let mut cache = TrajectoryCache::new(16);

    let bad = cache.get_or_compute(&system, &InitialStateInput::new("-4"), &config);
    assert_eq!(
        bad.trajectory_status,
        statelab_engine::TrajectoryStatus::SystemError
    );
    assert_eq!(cache.len(), 0, "SystemError results must not be cached");
}

// ---- Schema migration (§4.9) ----

/// Dummy additive migration `1.0.0 -> 2.0.0`: bumps the version and adds one new
/// optional field, preserving everything else (a stand-in for a real future bump).
struct DummyV1ToV2;

impl TrajectoryMigration for DummyV1ToV2 {
    fn from_version(&self) -> &'static str {
        "1.0.0"
    }
    fn to_version(&self) -> &'static str {
        "2.0.0"
    }
    fn migrate(&self, mut old: Value) -> Result<Value, MigrationError> {
        let obj = old
            .as_object_mut()
            .ok_or_else(|| MigrationError::new("expected a JSON object"))?;
        obj.insert("trajectory_schema_version".into(), json!("2.0.0"));
        // Additive: a new field, defaulted, without touching existing ones.
        obj.insert("example_added_field".into(), Value::Null);
        Ok(old)
    }
}

#[test]
fn dummy_v1_to_v2_round_trips() {
    // Produce a real v1.0.0 document from the engine.
    let system = ClassicCollatz;
    let config = EngineConfig::with_max_iterations(1_000_000);
    let mut cache = TrajectoryCache::from_config(&config);
    let trajectory = cache.get_or_compute(&system, &input(3), &config);
    let v1: Value = serde_json::to_value(&trajectory).expect("serialize v1");
    assert_eq!(MigrationRegistry::version_of(&v1), Some("1.0.0"));

    // Register the migration and bring the old document forward.
    let mut registry = MigrationRegistry::new();
    registry.register(Box::new(DummyV1ToV2));
    let v2 = registry
        .migrate(v1.clone(), "2.0.0")
        .expect("migrate to 2.0.0");

    // Version bumped, new field present.
    assert_eq!(MigrationRegistry::version_of(&v2), Some("2.0.0"));
    assert_eq!(v2.get("example_added_field"), Some(&Value::Null));

    // Additive guarantee: every original field survived unchanged (§4.9 — older
    // exported datasets remain readable).
    assert_eq!(v2.get("state_sequence"), v1.get("state_sequence"));
    assert_eq!(v2.get("system_id"), v1.get("system_id"));
    assert_eq!(
        v2.get("system_specific_metrics"),
        v1.get("system_specific_metrics")
    );

    // A no-op migration to the current version returns the document unchanged.
    let same = registry
        .migrate(v1.clone(), "1.0.0")
        .expect("no-op migrate");
    assert_eq!(same, v1);
}

#[test]
fn migration_fails_without_a_path() {
    let registry = MigrationRegistry::new(); // no migrations registered
    let doc = json!({ "trajectory_schema_version": "1.0.0" });
    let err = registry.migrate(doc, "2.0.0").unwrap_err();
    assert!(err
        .message
        .contains("no migration registered from schema version 1.0.0"));
}

#[test]
fn migration_requires_a_version_field() {
    let mut registry = MigrationRegistry::new();
    registry.register(Box::new(DummyV1ToV2));
    let doc = json!({ "not_a_version": true });
    let err = registry.migrate(doc, "2.0.0").unwrap_err();
    assert!(err.message.contains("missing"));
}

// ---- Engine defaults ----

/// Pins the default iteration limit as an explicit contract: the frontend mirrors
/// the same number in `DEFAULT_ENGINE_CONFIG`, and a silent drift between the two
/// would make the UI request a different bound than the engine's own default.
///
/// The value was briefly 10,000,000 during the post-audit pass, on the strength of
/// a cited spec addendum that did not exist (see docs/AUDIT_REMEDIATION.md).
/// Reverted to 100,000 — the only figure the specification actually shows.
#[test]
fn default_iteration_limit_is_one_hundred_thousand() {
    assert_eq!(EngineConfig::default().max_iterations, 100_000);
}

/// The default limit must not change what a *converging* run produces — only how
/// far a non-converging one is allowed to go. n = 27 converges in 111 steps under
/// any limit above that, so this holds regardless of which default is in force.
#[test]
fn the_default_limit_does_not_change_converging_runs() {
    let system = ClassicCollatz;
    let tight = statelab_engine::StateEvolutionEngine::run(
        &system,
        &input(27),
        &EngineConfig::with_max_iterations(100_000),
    );
    let generous =
        statelab_engine::StateEvolutionEngine::run(&system, &input(27), &EngineConfig::default());
    assert_eq!(tight.state_sequence, generous.state_sequence);
    assert_eq!(tight.iteration_count, generous.iteration_count);
    assert_eq!(tight.trajectory_status, generous.trajectory_status);
    assert_eq!(
        tight.system_specific_metrics,
        generous.system_specific_metrics
    );
}
