use std::sync::{Arc, atomic::AtomicBool};

use color_eyre::eyre;
use tauri::Emitter;

use crate::{
    cli::{BuildOptions, RunOptions},
    processing,
};

#[derive(Debug, serde::Serialize, specta::Type)]
pub struct CommandError(pub String);

impl From<eyre::Error> for CommandError {
    fn from(err: eyre::Error) -> Self {
        Self(
            err.chain()
                .map(|cause| cause.to_string())
                .collect::<Vec<_>>()
                .join(" -> "),
        )
    }
}

pub type Result<T> = std::result::Result<T, CommandError>;

#[tauri::command]
#[specta::specta]
pub async fn run_build(options: BuildOptions) -> Result<()> {
    Ok(processing::run_build(options)?)
}

#[tauri::command]
#[specta::specta]
pub async fn run_process(app_handle: tauri::AppHandle, options: RunOptions) -> Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let progress = |progress| {
        app_handle
            .emit("process-updated", progress)
            .expect("Failed to emit process update");
    };
    Ok(processing::run_process(options, running, progress)?)
}
