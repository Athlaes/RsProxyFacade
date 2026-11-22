use serde::Serialize;
use thiserror::Error;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Error, Debug, Serialize)]
pub enum SystemProxyError {}

#[derive(Serialize)]
pub enum SystemProxy {
    Direct,
    Auto { url: String },
    Manual { host: String, port: u16, bypass: String },
}
