#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod adapters;
pub mod app;
pub mod core;
pub mod error;
pub mod ipc;
pub mod state;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init()
        .ok();

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        // commands wired in by feature modules
        .run(tauri::generate_context!());

    if let Err(err) = result {
        tracing::error!("tauri runtime error: {err}");
    }
}
