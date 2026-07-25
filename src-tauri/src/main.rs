//! StateLab desktop shell (Tauri).
//!
//! The shell's entire job is to host the UI and expose the engine over IPC
//! (§3.2). It holds no trajectory mathematics: every number the user sees is
//! computed by `statelab-engine`, the single source of truth (Principle #4).
//!
//! Commands live in [`commands`]; the frontend reaches them through the Research
//! Controller's single IPC seam (`src/lib/invoke.ts`).

// Release builds attach no console window — this is a GUI application.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

mod commands;

use commands::AppState;

fn main() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::run_trajectory,
            commands::run_dataset
        ])
        .run(tauri::generate_context!())
        .expect("error while running the StateLab application");
}
