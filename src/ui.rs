mod app;
mod inputs;

use std::path::PathBuf;

use app::App;

pub fn run_gui() {
    let data_dir = webview_data_directory();
    dioxus::LaunchBuilder::desktop()
        .with_cfg(dioxus::desktop::Config::new().with_data_directory(data_dir))
        .launch(App);
}

fn webview_data_directory() -> PathBuf {
    let base_dir = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));

    base_dir.join("assfonts.webview")
}
