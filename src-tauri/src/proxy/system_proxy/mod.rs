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
        Box<dyn std::error::Error + Send + Sync>,
    ),
    #[error(transparent)]
    SettingSystem(#[serde(skip_serializing)] Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Serialize)]
pub enum SystemProxyMode {
    Direct,
    Pac { url: String },
    Manual { host: String, port: u16, bypass: String },
}

pub struct SystemProxyManager;

pub trait SystemProxyReader {
    fn read_system_proxy() -> Result<SystemProxyMode, SystemProxyError>;
}

pub trait SystemProxySetter {
    fn set_system_proxy(mode: SystemProxyMode) -> Result<(), SystemProxyError>;
}
