pub mod ass;
pub mod cff_fix;
#[cfg(feature = "cli")]
pub mod cli;
#[cfg(feature = "gui")]
pub mod commands;
pub mod embed;
pub mod error;
pub mod font;
pub mod processing;
pub mod subset;

#[cfg(feature = "gui")]
use specta_typescript::{BigIntExportBehavior, Typescript};
#[cfg(feature = "gui")]
use tauri_specta::{Builder, collect_commands};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[cfg(feature = "gui")]
pub fn run() {
    let builder = Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::run_build,
            commands::run_process,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Throw);

    #[cfg(debug_assertions)] // <- Only export on non-release builds
    builder
        .export(
            Typescript::default().bigint(BigIntExportBehavior::Number),
            "../src/bindings.ts",
        )
        .expect("Failed to export typescript bindings");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
