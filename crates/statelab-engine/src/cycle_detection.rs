//! Generic cycle detection (§4.6).
//!
//! The engine is the only place cycle detection happens, and it is fully generic:
//! it works for **any** system via that system's `states_equal` / `state_hash`,
//! knowing nothing about the state's shape.
//!
//! **Algorithm (IMPLEMENTATION DECISION §4.6):** a hash-indexed visited-state set
//! (`HashMap<u64, Vec<(index, State)>>`, resolving hash collisions via
//! `states_equal`). Memory is bounded by `config.max_iterations` — the same bound
//! that already governs the iteration-limit check — so no new unbounded growth is
//! introduced. This is correct and generic, not memory-optimal; a lower-memory
//! detector (e.g. Brent's) may replace it later without touching any architecture
//! (Appendix C, item 5).

use std::collections::HashMap;

/// A detected revisit: the run returned to a state first seen at
/// `start_index`, and the cycle spans `length` states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CycleHit {
    /// Index (into the state sequence) where the repeated state first appeared.
    pub start_index: usize,
    /// Number of states in the detected cycle (`current_index - start_index`).
    pub length: usize,
}

/// Tracks every state the run has visited so a revisit can be detected generically.
#[derive(Clone, Debug, Default)]
pub struct CycleTracker<S> {
    // hash -> list of (sequence index, state) sharing that hash.
    buckets: HashMap<u64, Vec<(usize, S)>>,
}

impl<S: Clone> CycleTracker<S> {
    /// Creates a tracker already containing the initial state at index 0.
    pub fn new(initial: &S, hash: impl Fn(&S) -> u64) -> Self {
        let mut buckets: HashMap<u64, Vec<(usize, S)>> = HashMap::new();
        buckets
            .entry(hash(initial))
            .or_default()
            .push((0, initial.clone()));
        Self { buckets }
    }

    /// Records `state` at `index` and reports a [`CycleHit`] if this exact state
    /// (per `eq`) was seen at an earlier index. Hash collisions are resolved by
    /// scanning the bucket with `eq`, so correctness never depends on `hash` being
    /// collision-free.
    pub fn check(
        &mut self,
        state: &S,
        index: usize,
        eq: impl Fn(&S, &S) -> bool,
        hash: impl Fn(&S) -> u64,
    ) -> Option<CycleHit> {
        let bucket = self.buckets.entry(hash(state)).or_default();
        for (seen_index, seen_state) in bucket.iter() {
            if eq(seen_state, state) {
                return Some(CycleHit {
                    start_index: *seen_index,
                    length: index - *seen_index,
                });
            }
        }
        bucket.push((index, state.clone()));
        None
    }
}
