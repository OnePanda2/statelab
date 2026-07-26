//! Tauri IPC command handlers (§3.2).
//!
//! These are **thin wrappers** around `statelab-engine`: they marshal arguments
//! in, call the engine, and hand back a finalized Trajectory Object. They contain
//! **no trajectory mathematics** — per §2.2 the shell may never compute. The
//! Research Controller on the frontend is the only caller.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use statelab_dataset::{for_each_summary, DatasetSpec};
use statelab_engine::{EngineConfig, InitialStateInput, Trajectory, TrajectoryCache};
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
    let engine_config: EngineConfig = config.into();
    let input = InitialStateInput::new(initial_state);

    let mut cache = state
        .cache
        .lock()
        .map_err(|_| "trajectory cache lock was poisoned".to_string())?;

    // Dispatch through the systems registry. An unknown id is still *rejected*,
    // never silently substituted (Principle #4) — it just is no longer the case
    // that everything except Classic Collatz is unknown.
    statelab_engine::run_by_id_cached(&system_id, &input, &engine_config, &mut cache)
        .ok_or_else(|| format!("unknown system_id: {system_id}"))
}

/// Lists the systems this build can run, so the UI can offer them without
/// hardcoding a list that could drift from the engine's registry.
#[tauri::command]
pub fn list_systems() -> Vec<SystemDescriptor> {
    statelab_engine::AVAILABLE_SYSTEMS
        .iter()
        .map(|s| SystemDescriptor {
            id: s.id.to_string(),
            label: s.label.to_string(),
        })
        .collect()
}

/// A system offered to the UI.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemDescriptor {
    pub id: String,
    pub label: String,
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
