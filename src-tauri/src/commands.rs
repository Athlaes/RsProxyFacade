use serde::Serialize;
use tauri::State;
use thiserror::Error;
use tokio::{io, sync::Mutex};
use tracing::error;

use crate::ExecutionStatus;

use crate::pac::server::{ProxyPacServer, ProxyPacServerConfig};
use crate::{
    AppState,
    proxy::server::{ProxyConfig, ProxyServer},
};

#[derive(Error, Debug, Serialize)]
pub enum CommandError {
    #[error("Error starting server on address {host}:{port}: {cause}")]
    ServerStart {
        host: String,
        port: u16,
        #[source]
        #[serde(skip_serializing)]
        cause: io::Error,
    },
    #[error("{0}")]
    ServerStop(String),
    #[error("Couldn't load pac file for path {path}")]
    PacLoading {
        path: String,
        #[source]
        #[serde(skip_serializing)]
        cause: io::Error,
    },
    #[error("Couldn't load file with selector {selector}")]
    LoadingMethodSelection { selector: String },
}

#[tauri::command]
pub async fn start_proxy_server(
    state: State<'_, Mutex<AppState>>,
    config: ProxyConfig,
) -> Result<String, CommandError> {
    // Serialize start/stop, and release the old port before binding the new one.
    let mut state = state.lock().await;
    if let Some(mut server) = state.proxy_server.take() {
        server.shutdown().await;
    }
    let server = ProxyServer::new(config).await.map_err(|err| {
        error!("{err}");
        err
    })?;

    state.proxy_server = Some(server);

    Ok("Proxy server started sucessfully".to_string())
}

#[tauri::command]
pub async fn stop_proxy_server(state: State<'_, Mutex<AppState>>) -> Result<String, CommandError> {
    let mut state = state.lock().await;
    if let Some(mut server) = state.proxy_server.take() {
        server.shutdown().await;
    } else {
        let message = "Proxy server stopped command received but no proxy found started.";
        error!(message);
        return Err(CommandError::ServerStop(message.to_string()));
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
    config: ProxyPacServerConfig,
) -> Result<String, CommandError> {
    let mut state = state.lock().await;
    if let Some(mut server) = state.proxy_pac_server.take() {
        server.shutdown().await;
    }
    let server = ProxyPacServer::new(config).await.map_err(|err| {
        error!("{err}");
        err
    })?;

    state.proxy_pac_server = Some(server);

    Ok("Proxy pac server started sucessfully".to_string())
}

#[tauri::command]
pub async fn stop_proxy_pac_server(state: State<'_, Mutex<AppState>>) -> Result<String, CommandError> {
    let mut state = state.lock().await;
    if let Some(mut server) = state.proxy_pac_server.take() {
        server.shutdown().await;
    } else {
        let message = "Proxy pac server stopped command received but no proxy found started.";
        error!(message);
        return Err(CommandError::ServerStop(message.to_string()));
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
