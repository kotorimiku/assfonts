use std::sync::{Arc, atomic::AtomicBool};

use tauri::Emitter;

use crate::{
    cli::{BuildOptions, RunOptions},
    processing,
};

#[tauri::command]
#[specta::specta]
pub async fn run_build(options: BuildOptions) -> Result<(), String> {
    processing::run_build(options).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn run_process(app_handle: tauri::AppHandle, options: RunOptions) -> Result<(), String> {
    let running = Arc::new(AtomicBool::new(true));
    let once = |processes| {
        app_handle
            .emit("process-updated", processes)
            .expect("Failed to emit process update");
    };
    processing::run_process(options, running, once).map_err(|e| e.to_string())
}
