use serde::Serialize;
use thiserror::Error;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Error, Debug, Serialize)]
pub enum SystemProxyError {
    #[error(transparent)]
    ReadingSystem(
        #[from]
        #[serde(skip_serializing)]
        Box<dyn std::error::Error>,
    ),
}

#[derive(Serialize)]
pub enum SystemProxy {
    Direct,
    Auto { url: String },
    Manual { host: String, port: u16, bypass: String },
}
