use display_error_chain::ErrorChainExt;
use serde::Deserialize;
use tauri::State;
use tokio::sync::Mutex;
use tracing::{error, warn};

use crate::ExecutionStatus;

use crate::pac::server::ProxyPacServer;
use crate::proxy::server::ProxyServerConfig;
use crate::{AppState, proxy::server::ProxyServer};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PacServerCommand {
    pub host: String,
    pub port: u16,
    pub pac_url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyServerCommand {
    pub host: String,
    pub port: u16,
    pub proxy_mode: ProxyModeCommand,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProxyModeCommand {
    Direct,
    System,
    Manual { host: String, port: u16, bypass: String },
    Pac { url: String },
}

#[tauri::command]
pub async fn start_proxy_server(
    state: State<'_, Mutex<AppState>>,
    config: ProxyServerCommand,
) -> Result<ProxyServerConfig, String> {
    // Serialize start/stop, and release the old port before binding the new one.
    let mut state = state.lock().await;
    if let Some(mut server) = state.proxy_server.take() {
        server.shutdown().await.map_err(|err| {
            error!("{}", err.chain());
            err.to_string()
        })?;
    }
    let server = ProxyServer::new(config)
        .await
        .inspect_err(|err| error!("{}", err.chain()))
        .map_err(|err| err.to_string())?;

    let config_res = server.config.as_ref().clone();
    state.proxy_server = Some(server);

    Ok(config_res)
}

#[tauri::command]
pub async fn stop_proxy_server(state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let mut state = state.lock().await;
    if let Some(mut server) = state.proxy_server.take() {
        server
            .shutdown()
            .await
            .inspect_err(|err| error!("{}", err.chain()))
            .map_err(|err| err.to_string())?;
    } else {
        let message = "Proxy server stopped command received but no proxy found started.";
        warn!(message);
        return Ok(message.to_string());
    }

    Ok("Proxy server stopped sucessfully".to_string())
}

#[tauri::command]
pub async fn proxy_server_status(state: State<'_, Mutex<AppState>>) -> Result<ExecutionStatus, ()> {
    let state = state.lock().await;
    Ok(state
        .proxy_server
        .as_ref()
        .map(|s| s.proxy_status())
        .unwrap_or(ExecutionStatus::Fail))
}

#[tauri::command]
pub async fn start_proxy_pac_server(
    state: State<'_, Mutex<AppState>>,
    config: PacServerCommand,
) -> Result<String, String> {
    let mut state = state.lock().await;
    if let Some(mut server) = state.proxy_pac_server.take() {
        server
            .shutdown()
            .await
            .inspect_err(|err| error!("{}", err.chain()))
            .map_err(|err| err.to_string())?;
    }
    let server = ProxyPacServer::new(config)
        .await
        .inspect_err(|err| error!("{}", err.chain()))
        .map_err(|err| err.to_string())?;
    state.proxy_pac_server = Some(server);

    Ok("Pac server started successfully".to_string())
}

#[tauri::command]
pub async fn stop_proxy_pac_server(state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let mut state = state.lock().await;
    if let Some(mut server) = state.proxy_pac_server.take() {
        server
            .shutdown()
            .await
            .inspect_err(|err| error!("{}", err.chain()))
            .map_err(|err| err.to_string())?;
    } else {
        let message = "Proxy pac server stopped command received but no proxy found started.";
        warn!(message);
        return Ok(message.to_string());
    }

    Ok("Proxy server stopped sucessfully".to_string())
}

#[tauri::command]
pub async fn proxy_pac_server_status(state: State<'_, Mutex<AppState>>) -> Result<ExecutionStatus, ()> {
    let state = state.lock().await;
    Ok(state
        .proxy_pac_server
        .as_ref()
        .map(|s| s.proxy_status())
        .unwrap_or(ExecutionStatus::Fail))
}
