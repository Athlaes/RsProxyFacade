use gio::{Settings, SettingsBackend, SettingsSchemaSource, prelude::*};

use super::SystemProxyError;
use crate::commands::ProxyMode;

fn error(message: impl Into<String>) -> SystemProxyError {
    SystemProxyError::ReadingSystem(message.into().into())
}

fn supported_desktop(desktop: &str) -> bool {
    desktop
        .split(':')
        .any(|name| name.eq_ignore_ascii_case("GNOME") || name.eq_ignore_ascii_case("Unity"))
}

// Look up schemas before constructing Settings: Settings::new aborts if a schema is missing.
fn settings(
    source: &SettingsSchemaSource,
    backend: Option<&SettingsBackend>,
    id: &str,
    keys: &[&str],
) -> Result<Settings, SystemProxyError> {
    let schema = source
        .lookup(id, true)
        .ok_or_else(|| error(format!("Missing GSettings schema: {id}")))?;
    for key in keys {
        if !schema.has_key(key) {
            return Err(error(format!("Missing GSettings key: {id}.{key}")));
        }
    }
    Ok(Settings::new_full(&schema, backend, None))
}

fn read_settings(
    source: &SettingsSchemaSource,
    backend: Option<&SettingsBackend>,
) -> Result<ProxyMode, SystemProxyError> {
    let root = settings(
        source,
        backend,
        "org.gnome.system.proxy",
        &["mode", "autoconfig-url", "ignore-hosts"],
    )?;
    match root.string("mode").as_str() {
        "none" => Ok(ProxyMode::Direct),
        "auto" => {
            let url = root.string("autoconfig-url").trim().to_owned();
            if url.is_empty() {
                return Err(error(
                    "Automatic proxy configuration has no explicit PAC URL; WPAD is disabled",
                ));
            }
            Ok(ProxyMode::Pac { url })
        }
        "manual" => {
            let mut http_proxy = None;
            for protocol in ["http", "https", "socks"] {
                let child = settings(
                    source,
                    backend,
                    &format!("org.gnome.system.proxy.{protocol}"),
                    &["host", "port"],
                )?;
                let host = child.string("host").trim().to_owned();
                let port = child.int("port");
                if host.is_empty() || port == 0 {
                    continue;
                }
                let port = u16::try_from(port).map_err(|_| error(format!("Invalid {protocol} proxy port")))?;
                let endpoint = (host, port);
                match protocol {
                    "http" => http_proxy = Some(endpoint),
                    "https" if http_proxy.as_ref() == Some(&endpoint) => {}
                    _ => return Err(error("Only a single HTTP proxy shared by HTTP and HTTPS is supported")),
                }
            }
            // GNOME uses HTTP for HTTPS when the HTTPS entry is unset.
            let Some((host, port)) = http_proxy else {
                return Ok(ProxyMode::Direct);
            };
            let bypass = root
                .strv("ignore-hosts")
                .iter()
                .map(|value| value.as_str())
                .collect::<Vec<_>>()
                .join(";");
            Ok(ProxyMode::Manual { host, port, bypass })
        }
        _ => Err(error("Unsupported GNOME proxy mode")),
    }
}

impl ProxyMode {
    /// Read GNOME/Unity settings. KDE, environment variables and WPAD are not supported.
    pub fn from_system() -> Result<Self, SystemProxyError> {
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        if !supported_desktop(&desktop) {
            return Err(error(
                "System proxy detection currently supports only GNOME/Unity on Linux",
            ));
        }
        let source = SettingsSchemaSource::default().ok_or_else(|| error("GSettings schemas are unavailable"))?;
        read_settings(&source, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_selection_does_not_mistake_kde_for_gnome() {
        assert!(supported_desktop("ubuntu:GNOME"));
        assert!(supported_desktop("Unity"));
        assert!(!supported_desktop("KDE"));
        assert!(!supported_desktop(""));
    }

    // Uses real GSettings with a private memory backend; never writes desktop preferences.
    // Requires the gsettings-desktop-schemas package on the test host.
    #[test]
    fn reads_gnome_modes_and_exclusions() {
        let source = SettingsSchemaSource::default().expect("Install gsettings-desktop-schemas to run this test");
        let backend = gio::memory_settings_backend_new();
        let root = settings(&source, Some(&backend), "org.gnome.system.proxy", &["mode"]).unwrap();
        root.set_string("mode", "none").unwrap();
        assert!(matches!(
            read_settings(&source, Some(&backend)).unwrap(),
            ProxyMode::Direct
        ));
        root.set_string("mode", "auto").unwrap();
        root.set_string("autoconfig-url", "https://config.example/proxy.pac")
            .unwrap();
        assert!(
            matches!(read_settings(&source, Some(&backend)).unwrap(), ProxyMode::Pac { url } if url == "https://config.example/proxy.pac")
        );
        root.set_string("autoconfig-url", "").unwrap();
        assert!(read_settings(&source, Some(&backend)).is_err());
        root.set_string("mode", "manual").unwrap();
        root.set_strv("ignore-hosts", ["localhost", "10.0.0.0/8"]).unwrap();
        let http = settings(
            &source,
            Some(&backend),
            "org.gnome.system.proxy.http",
            &["host", "port"],
        )
        .unwrap();
        http.set_string("host", "proxy.example").unwrap();
        http.set_int("port", 8080).unwrap();
        let ProxyMode::Manual { host, port, bypass } = read_settings(&source, Some(&backend)).unwrap() else {
            panic!("Expected manual settings")
        };
        assert_eq!(host, "proxy.example");
        assert_eq!(port, 8080);
        assert_eq!(bypass, "localhost;10.0.0.0/8");
        let https = settings(
            &source,
            Some(&backend),
            "org.gnome.system.proxy.https",
            &["host", "port"],
        )
        .unwrap();
        https.set_string("host", "other-proxy.example").unwrap();
        https.set_int("port", 8080).unwrap();
        assert!(read_settings(&source, Some(&backend)).is_err());
        https.set_string("host", "proxy.example").unwrap();
        assert!(matches!(
            read_settings(&source, Some(&backend)).unwrap(),
            ProxyMode::Manual { .. }
        ));
        let socks = settings(
            &source,
            Some(&backend),
            "org.gnome.system.proxy.socks",
            &["host", "port"],
        )
        .unwrap();
        socks.set_string("host", "socks.example").unwrap();
        socks.set_int("port", 1080).unwrap();
        assert!(read_settings(&source, Some(&backend)).is_err());
        assert!(settings(&source, Some(&backend), "org.example.missing", &[]).is_err());
    }
}
