use serde::{Deserialize, Serialize};
use tauri::Manager;
use tokio::sync::Mutex;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::{pac::server::ProxyPacServer, proxy::server::ProxyServer};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod commands;
mod pac;
mod proxy;

struct AppState {
    proxy_server: Option<ProxyServer>,
    proxy_pac_server: Option<ProxyPacServer>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ExecutionStatus {
    Success,
    Fail,
    Warning,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    tauri::Builder::default()
        .setup(|app| {
            app.manage(Mutex::new(AppState {
                proxy_server: None,
                proxy_pac_server: None,
            }));
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::start_proxy_server,
            commands::stop_proxy_server,
            commands::proxy_server_status,
            commands::start_proxy_pac_server,
            commands::stop_proxy_pac_server,
            commands::proxy_pac_server_status,
        ])
        .run(tauri::generate_context!())
        .expect("Error while running tauri application");
    info!("Application started successfully.")
}
