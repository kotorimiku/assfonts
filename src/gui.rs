use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

use color_eyre::eyre::WrapErr;
use slint::ComponentHandle;

use crate::{cli::RunOptions, commands::run_process, error::Result};

slint::include_modules!();

pub fn run_gui() -> Result<()> {
    let app = AppWindow::new()?;

    let log_buffer: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let running_flag: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

    // Browse callbacks
    {
        let app_weak = app.as_weak();
        app.on_browse_input(move || {
            if let Some(app) = app_weak.upgrade() {
                if let Some(paths) = rfd::FileDialog::new()
                    .set_title("Select ASS Files")
                    .add_filter("ASS Subtitles", &["ass"])
                    .pick_files()
                {
                    let paths_str = paths
                        .iter()
                        .map(|p| p.to_string_lossy().to_string())
                        .collect::<Vec<_>>()
                        .join("|");
                    app.set_input_paths(paths_str.into());
                }
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_browse_input_folders(move || {
            if let Some(app) = app_weak.upgrade() {
                if let Some(paths) = rfd::FileDialog::new()
                    .set_title("Select Input Directories")
                    .pick_folders()
                {
                    let current = app.get_input_paths().to_string();
                    let new_paths = paths
                        .iter()
                        .map(|p| p.to_string_lossy().to_string())
                        .collect::<Vec<_>>()
                        .join("|");
                    let combined = if current.trim().is_empty() {
                        new_paths
                    } else {
                        format!("{}|{}", current, new_paths)
                    };
                    app.set_input_paths(combined.into());
                }
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_browse_fonts(move || {
            if let Some(app) = app_weak.upgrade() {
                if let Some(paths) = rfd::FileDialog::new()
                    .set_title("Select Font Directories")
                    .pick_folders()
                {
                    let paths_str = paths
                        .iter()
                        .map(|p| p.to_string_lossy().to_string())
                        .collect::<Vec<_>>()
                        .join("|");
                    app.set_font_paths(paths_str.into());
                }
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_browse_output(move || {
            if let Some(app) = app_weak.upgrade() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Select Output Directory")
                    .pick_folder()
                {
                    app.set_output_path(path.to_string_lossy().to_string().into());
                }
            }
        });
    }

    {
        let app_weak = app.as_weak();
        app.on_browse_db(move || {
            if let Some(app) = app_weak.upgrade() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Select Database Directory")
                    .pick_folder()
                {
                    app.set_db_path(path.to_string_lossy().to_string().into());
                }
            }
        });
    }

    // Run callback
    {
        let app_weak = app.as_weak();
        let log_buffer = log_buffer.clone();
        let running_flag = running_flag.clone();

        app.on_run_process(move || {
            let app = match app_weak.upgrade() {
                Some(app) => app,
                None => return,
            };

            // Parse inputs
            let inputs_str = app.get_input_paths().to_string();
            if inputs_str.trim().is_empty() {
                app.set_status_text("Error: No input files specified".into());
                return;
            }

            let inputs: Vec<PathBuf> = inputs_str
                .split('|')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .collect();

            if inputs.is_empty() {
                app.set_status_text("Error: No valid input paths".into());
                return;
            }

            // Parse options
            let fontpaths_str = app.get_font_paths().to_string();
            let fontpaths = if fontpaths_str.trim().is_empty() {
                None
            } else {
                Some(
                    fontpaths_str
                        .split('|')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .map(PathBuf::from)
                        .collect::<Vec<_>>(),
                )
            };

            let options = RunOptions {
                inputs,
                output: PathBuf::from(app.get_output_path().to_string()),
                fontpaths,
                dbpath: PathBuf::from(app.get_db_path().to_string()),
                strict: app.get_strict_mode(),
                allow_missing_sample: app.get_allow_missing_sample(),
                allow_error_fonts: app.get_allow_missing_fonts(),
                report: app.get_generate_report(),
                force: app.get_force_overwrite(),
            };

            // Update UI state
            app.set_is_running(true);
            app.set_status_text("Processing...".into());
            app.set_progress(0.0);

            {
                let mut log = log_buffer.lock().unwrap();
                log.clear();
                log.push_str("Starting process...\n");
                app.set_log_text(log.clone().into());
            }

            // Store running flag
            {
                let mut running = running_flag.lock().unwrap();
                *running = true;
            }

            // Spawn processing thread
            let app_weak_inner = app.as_weak();
            let log_buffer_inner = log_buffer.clone();
            let running_flag_inner = running_flag.clone();

            thread::spawn(move || {
                let result = run_process(options);

                // Update UI on completion
                if let Some(app) = app_weak_inner.upgrade() {
                    let mut log = log_buffer_inner.lock().unwrap();

                    match result {
                        Ok(()) => {
                            log.push_str("\nProcess completed successfully.\n");
                            app.set_status_text("Completed".into());
                            app.set_progress(1.0);
                        }
                        Err(e) => {
                            log.push_str(&format!("\nError: {}\n", e));
                            app.set_status_text("Error occurred".into());
                        }
                    }

                    {
                        let mut running = running_flag_inner.lock().unwrap();
                        *running = false;
                    }

                    app.set_is_running(false);
                    app.set_log_text(log.clone().into());
                }
            });
        });
    }

    // Stop callback
    {
        let app_weak = app.as_weak();
        let running_flag = running_flag.clone();

        app.on_stop_process(move || {
            {
                let mut running = running_flag.lock().unwrap();
                *running = false;
            }

            if let Some(app) = app_weak.upgrade() {
                app.set_is_running(false);
                app.set_status_text("Stopped".into());
            }
        });
    }

    app.run().wrap_err("GUI event loop error")?;
    Ok(())
}
