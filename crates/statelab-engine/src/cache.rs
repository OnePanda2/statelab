//! Memoization cache (§4.8).
//!
//! **Key (FROZEN):** a trajectory may only be memoized under the full tuple
//! `(system_id, system_version, engine_config, initial_state)` — never under the
//! starting number alone, so two different iteration limits (or any future config
//! change) can never collide on lookup.
//!
//! **Eviction (IMPLEMENTATION DECISION):** LRU with a configurable
//! `cache_max_entries` bound (see [`crate::engine::EngineConfig`]). The LRU here is
//! a simple `HashMap` + recency-ordered `Vec` — correct and generic, not the
//! lowest-overhead structure possible; it may be swapped for an intrusive
//! linked-list LRU later without touching the cache's public contract (Appendix C).
//!
//! The pure [`crate::engine::StateEvolutionEngine`] stays stateless and
//! cache-unaware; caching is layered on top by [`TrajectoryCache`], which the
//! Research Controller owns. This keeps the driver reproducible in isolation.

use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;

use crate::engine::{EngineConfig, StateEvolutionEngine};
use crate::system::{DeterministicSystem, InitialStateInput};
use crate::trajectory::Trajectory;

/// The one valid memoization key (§4.8 FROZEN).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CacheKey {
    /// Identifier of the system, e.g. `"classic-collatz"`.
    pub system_id: String,
    /// Version of that system implementation.
    pub system_version: String,
    /// Full engine configuration in effect.
    pub engine_config: EngineConfig,
    /// The validated initial state, in canonical string form.
    pub initial_state: String,
}

impl CacheKey {
    /// Builds a cache key from its FROZEN components.
    pub fn new(
        system_id: impl Into<String>,
        system_version: impl Into<String>,
        engine_config: EngineConfig,
        initial_state: impl Into<String>,
    ) -> Self {
        Self {
            system_id: system_id.into(),
            system_version: system_version.into(),
            engine_config,
            initial_state: initial_state.into(),
        }
    }
}

/// A small, generic LRU cache. `capacity == 0` disables caching entirely.
#[derive(Clone, Debug)]
pub struct LruCache<K, V> {
    capacity: usize,
    map: HashMap<K, V>,
    /// Recency order: front = least-recently-used, back = most-recently-used.
    order: Vec<K>,
}

impl<K: Clone + Eq + Hash, V> LruCache<K, V> {
    /// Creates a cache holding at most `capacity` entries.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            map: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// Looks up a key, marking it most-recently-used on a hit.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        if self.map.contains_key(key) {
            self.touch(key);
            self.map.get(key)
        } else {
            None
        }
    }

    /// Inserts or updates a value, evicting the least-recently-used entry if the
    /// capacity would be exceeded.
    pub fn put(&mut self, key: K, value: V) {
        if self.capacity == 0 {
            return;
        }
        if self.map.contains_key(&key) {
            self.map.insert(key.clone(), value);
            self.touch(&key);
            return;
        }
        if self.map.len() >= self.capacity && !self.order.is_empty() {
            let lru = self.order.remove(0);
            self.map.remove(&lru);
        }
        self.map.insert(key.clone(), value);
        self.order.push(key);
    }

    /// Whether the key is currently cached (does not affect recency).
    pub fn contains(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Configured maximum number of entries.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    fn touch(&mut self, key: &K) {
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            let k = self.order.remove(pos);
            self.order.push(k);
        }
    }
}

/// Memoizes finalized trajectories under the FROZEN [`CacheKey`]. Wraps the pure
/// engine: a miss computes via [`StateEvolutionEngine::run`] and stores the result;
/// a hit returns the stored trajectory with `execution_metadata.cache_hit = true`.
#[derive(Clone, Debug)]
pub struct TrajectoryCache {
    inner: LruCache<CacheKey, Trajectory>,
}

impl TrajectoryCache {
    /// Creates a cache bounded to `capacity` trajectories.
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: LruCache::new(capacity),
        }
    }

    /// Creates a cache using the bound from an [`EngineConfig`].
    pub fn from_config(config: &EngineConfig) -> Self {
        Self::new(config.cache_max_entries)
    }

    /// Returns the trajectory for `raw` under `config`, computing and storing it on
    /// a miss. Invalid input (which produces a `SystemError` trajectory) is **not**
    /// cached — the canonical key cannot be formed and the computation is trivial.
    pub fn get_or_compute<S>(
        &mut self,
        system: &S,
        raw: &InitialStateInput,
        config: &EngineConfig,
    ) -> Trajectory
    where
        S: DeterministicSystem,
        S::State: Display,
    {
        // Form the FROZEN key from the *canonical* validated state, so equivalent
        // inputs (e.g. "27" and " 27 ") share a cache entry.
        let canonical = match system.validate_initial_state(raw) {
            Ok(state) => state.to_string(),
            Err(_) => return StateEvolutionEngine::run(system, raw, config),
        };

        let key = CacheKey::new(
            system.system_id(),
            system.system_version(),
            config.clone(),
            canonical,
        );

        if let Some(hit) = self.inner.get(&key) {
            return hit.clone().mark_cache_hit();
        }

        let trajectory = StateEvolutionEngine::run(system, raw, config);
        self.inner.put(key, trajectory.clone());
        trajectory
    }

    /// Number of cached trajectories.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Whether a given key is currently cached (does not affect recency).
    pub fn contains(&self, key: &CacheKey) -> bool {
        self.inner.contains(key)
    }
}
