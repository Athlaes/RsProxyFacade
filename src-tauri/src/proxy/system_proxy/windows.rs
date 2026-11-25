use tauri::Url;
use windows::{
    Win32::{
        Foundation::{GlobalFree, HGLOBAL},
        Networking::WinHttp::{WINHTTP_CURRENT_USER_IE_PROXY_CONFIG, WinHttpGetIEProxyConfigForCurrentUser},
    },
    core::PWSTR,
};

use crate::proxy::system_proxy::{SystemProxy, SystemProxyError};

#[derive(Default)]
struct NativeConfig(WINHTTP_CURRENT_USER_IE_PROXY_CONFIG);

impl Drop for NativeConfig {
    fn drop(&mut self) {
        for ptr in [self.0.lpszAutoConfigUrl, self.0.lpszProxy, self.0.lpszProxyBypass] {
            if !ptr.is_null() {
                // WinHTTP allocates these strings with GlobalAlloc.
                unsafe {
                    let _ = GlobalFree(Some(HGLOBAL(ptr.0.cast())));
                }
            }
        }
    }
}

// The non-null pointer must reference a live, zero-terminated WinHTTP string.
unsafe fn copy_string(ptr: PWSTR) -> Result<Option<String>, SystemProxyError> {
    if ptr.is_null() {
        return Ok(None);
    }
    let value = unsafe {
        ptr.to_string()
            .map_err(|error| SystemProxyError::ReadingSystem(Box::new(error)))?
    };
    Ok((!value.is_empty()).then_some(value))
}

fn parse_proxy(value: &str) -> Result<(String, u16), SystemProxyError> {
    let address = value.trim();
    if address.contains(['=', ';']) || address.chars().any(char::is_whitespace) {
        return Err(SystemProxyError::ReadingSystem(
            "Advanced proxy settings are unsupported; configure a single proxy address and port in Windows Settings"
                .into(),
        ));
    }
    if address.is_empty() || address.contains(['/', '\\', '@', '?', '#']) {
        return Err(SystemProxyError::ReadingSystem("Invalid system proxy address".into()));
    }
    let url =
        Url::parse(&format!("http://{address}")).map_err(|error| SystemProxyError::ReadingSystem(Box::new(error)))?;
    let host = url
        .host()
        .ok_or_else(|| SystemProxyError::ReadingSystem("Missing system proxy host".into()))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| SystemProxyError::ReadingSystem("Missing system proxy port".into()))?;
    if port == 0 {
        return Err(SystemProxyError::ReadingSystem("Invalid system proxy port".into()));
    }
    Ok((host.to_string(), port))
}

impl SystemProxy {
    pub fn new() -> Result<Self, SystemProxyError> {
        let mut native = NativeConfig::default();
        // The guard keeps allocations alive during conversion and frees them on every exit.
        unsafe {
            WinHttpGetIEProxyConfigForCurrentUser(&mut native.0)
                .map_err(|error| SystemProxyError::ReadingSystem(Box::new(error)))?;
            if let Some(url) = copy_string(native.0.lpszAutoConfigUrl)? {
                return Ok(Self::Pac { url });
            }
            if let Some(proxy) = copy_string(native.0.lpszProxy)? {
                let (host, port) = parse_proxy(&proxy)?;
                return Ok(Self::Manual {
                    host,
                    port,
                    bypass: copy_string(native.0.lpszProxyBypass)?.unwrap_or_default(),
                });
            }
        }
        // Automatic discovery (fAutoDetect / WPAD) is deliberately ignored for now.
        Ok(Self::Direct)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_native_strings_are_supported() {
        assert_eq!(unsafe { copy_string(PWSTR::null()) }.unwrap(), None);
    }

    #[test]
    fn manual_proxy_preserves_default_port() {
        assert_eq!(parse_proxy("proxy.example:80").unwrap(), ("proxy.example".into(), 80));
        assert_eq!(
            parse_proxy("proxy.example:8080").unwrap(),
            ("proxy.example".into(), 8080)
        );
    }

    #[test]
    fn advanced_proxy_settings_are_rejected() {
        for value in [
            "http=proxy-a:8080;https=proxy-b:80",
            "http=proxy:8080",
            "proxy-a:80 proxy-b:80",
        ] {
            assert!(parse_proxy(value).is_err());
        }
    }

    #[test]
    fn malformed_addresses_are_not_silently_truncated() {
        for value in [
            "http=",
            "proxy:8080/path",
            "user:secret@proxy:8080",
            "proxy:0",
            "proxy:8080?query",
        ] {
            assert!(parse_proxy(value).is_err());
        }
    }
}
