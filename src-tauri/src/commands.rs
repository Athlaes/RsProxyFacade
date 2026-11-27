use display_error_chain::ErrorChainExt;
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::Mutex;
use tracing::{error, warn};

use crate::ExecutionStatus;

use crate::pac::server::{PacServerError, ProxyPacServer};
use crate::proxy::server::ProxyServerError;
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

#[derive(thiserror::Error, Debug, Serialize)]
pub enum CommandError {
    #[error(transparent)]
    ProxyServer(#[from] ProxyServerError),
    #[error(transparent)]
    PacServer(#[from] PacServerError),
}

#[tauri::command]
pub async fn start_proxy_server(
    state: State<'_, Mutex<AppState>>,
    config: ProxyServerCommand,
) -> Result<String, CommandError> {
    // Serialize start/stop, and release the old port before binding the new one.
    let mut state = state.lock().await;
    if let Some(mut server) = state.proxy_server.take() {
        server.shutdown().await.map_err(|err| {
            error!("{}", err.chain());
            err
        })?;
    }
    let server = ProxyServer::new(config)
        .await
        .inspect_err(|err| error!("{}", err.chain()))?;

    state.proxy_server = Some(server);

    Ok("Proxy server started sucessfully".to_string())
}

#[tauri::command]
pub async fn stop_proxy_server(state: State<'_, Mutex<AppState>>) -> Result<String, CommandError> {
    let mut state = state.lock().await;
    if let Some(mut server) = state.proxy_server.take() {
        server.shutdown().await.inspect_err(|err| error!("{}", err.chain()))?;
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
) -> Result<String, CommandError> {
    let mut state = state.lock().await;
    if let Some(mut server) = state.proxy_pac_server.take() {
        server.shutdown().await.inspect_err(|err| error!("{}", err.chain()))?;
    }
    let server = ProxyPacServer::new(config)
        .await
        .inspect_err(|err| error!("{}", err.chain()))?;

    state.proxy_pac_server = Some(server);

    Ok("Proxy pac server started sucessfully".to_string())
}

#[tauri::command]
pub async fn stop_proxy_pac_server(state: State<'_, Mutex<AppState>>) -> Result<String, CommandError> {
    let mut state = state.lock().await;
    if let Some(mut server) = state.proxy_pac_server.take() {
        server.shutdown().await.inspect_err(|err| error!("{}", err.chain()))?;
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
