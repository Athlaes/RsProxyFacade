use tauri::Url;
use windows::Win32::Networking::WinHttp::{
    WINHTTP_CURRENT_USER_IE_PROXY_CONFIG, WinHttpGetIEProxyConfigForCurrentUser,
};

use crate::proxy::system_proxy::{SystemProxy, SystemProxyError};

impl SystemProxy {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut proxy_config = WINHTTP_CURRENT_USER_IE_PROXY_CONFIG::default();
        unsafe {
            WinHttpGetIEProxyConfigForCurrentUser(&mut proxy_config).map_err(Box::new)?;
            if !proxy_config.lpszAutoConfigUrl.is_null() && !proxy_config.lpszAutoConfigUrl.is_empty() {
                return Ok(SystemProxy::Auto {
                    url: proxy_config.lpszAutoConfigUrl.to_string()?,
                });
            }
            if !proxy_config.lpszProxy.is_null() && !proxy_config.lpszProxy.is_empty() {
                let sys_url = proxy_config.lpszProxy.to_string()?;
                let url = Url::parse(&format!("http://{}", &sys_url))?;
                let host = url.host().ok_or(SystemProxyError::ReadingSystem(
                    format!("Couldn't read host of {}", sys_url).into(),
                ))?;
                let port = url.port().ok_or(SystemProxyError::ReadingSystem(
                    format!("Couldn't read port of {}", sys_url).into(),
                ))?;
                let bypass = proxy_config.lpszProxyBypass.to_string()?;
                return Ok(SystemProxy::Manual {
                    host: host.to_string(),
                    port: port,
                    bypass: bypass,
                });
            }
            Ok(Self::Direct)
        }
    }

    pub fn set_system_proxy(&self) {
        todo!()
    }
}
