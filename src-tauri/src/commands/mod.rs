//! Tauri IPC command handlers (§3.2).
//!
//! These are **thin wrappers** around `statelab-engine`: they marshal arguments
//! in, call the engine, and hand back a finalized Trajectory Object. They contain
//! **no trajectory mathematics** — per §2.2 the shell may never compute. The
//! Research Controller on the frontend is the only caller.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use statelab_dataset::{for_each_summary, DatasetSpec};
use statelab_engine::{
    ClassicCollatz, EngineConfig, InitialStateInput, Trajectory, TrajectoryCache,
};
use tauri::ipc::Channel;

/// Process-wide memoization cache (§4.8), owned by the shell. The engine itself
/// stays stateless so runs remain reproducible in isolation.
pub struct AppState {
    pub cache: Mutex<TrajectoryCache>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(TrajectoryCache::from_config(&EngineConfig::default())),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Mirror of the frontend's `EngineConfig` (§4.1 / §4.8).
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineConfigArgs {
    pub max_iterations: u64,
    pub cache_max_entries: usize,
}

impl From<EngineConfigArgs> for EngineConfig {
    fn from(args: EngineConfigArgs) -> Self {
        EngineConfig {
            max_iterations: args.max_iterations,
            cache_max_entries: args.cache_max_entries,
        }
    }
}

/// Runs one trajectory through the engine, via the memoization cache.
///
/// Invalid input is **not** an error: the engine returns a well-formed
/// `SystemError` trajectory, which the UI renders like any other result. An `Err`
/// here means the IPC call itself failed, not that the mathematics did.
#[tauri::command]
pub fn run_trajectory(
    system_id: String,
    initial_state: String,
    config: EngineConfigArgs,
    state: tauri::State<'_, AppState>,
) -> Result<Trajectory, String> {
    // Classic Collatz is the only registered system today. Reject anything else
    // explicitly rather than silently substituting it (§2.3 — never invent behaviour).
    if system_id != "classic-collatz" {
        return Err(format!("unknown system_id: {system_id}"));
    }

    let system = ClassicCollatz;
    let engine_config: EngineConfig = config.into();
    let input = InitialStateInput::new(initial_state);

    let mut cache = state
        .cache
        .lock()
        .map_err(|_| "trajectory cache lock was poisoned".to_string())?;
    Ok(cache.get_or_compute(&system, &input, &engine_config))
}

/// Streams a generated dataset (§6.2) back over an IPC channel, one compact
/// summary row per trajectory.
///
/// **Streaming is mandatory (FROZEN):** each trajectory is summarized, emitted,
/// and dropped before the next is generated, so the full set is never held in
/// memory. Returns the number of items processed.
#[tauri::command]
pub fn run_dataset(
    spec: DatasetSpec,
    max_iterations: Option<u64>,
    on_row: Channel<serde_json::Value>,
) -> Result<u64, String> {
    let processed = for_each_summary(spec, max_iterations, |row| {
        // A send failure means the frontend hung up (navigated away, cancelled) —
        // stop generating rather than computing rows nobody will read.
        on_row.send(row).is_ok()
    });
    Ok(processed)
}
