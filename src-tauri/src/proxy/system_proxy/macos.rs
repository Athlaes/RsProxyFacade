use core_foundation::{
    array::CFArray,
    base::{CFType, TCFType},
    dictionary::CFDictionary,
    number::CFNumber,
    string::CFString,
};
use system_configuration::dynamic_store::SCDynamicStoreBuilder;

use super::{SystemProxy, SystemProxyError};

type ProxySettings = CFDictionary<CFString, CFType>;

fn error(message: impl Into<String>) -> SystemProxyError {
    SystemProxyError::ReadingSystem(message.into().into())
}

fn number(settings: &ProxySettings, key: &str) -> Result<Option<i64>, SystemProxyError> {
    settings
        .find(CFString::new(key))
        .map(|value| {
            value
                .downcast::<CFNumber>()
                .and_then(|number| number.to_i64())
                .ok_or_else(|| error(format!("Invalid macOS proxy setting: {key}")))
        })
        .transpose()
}

fn enabled(settings: &ProxySettings, key: &str) -> Result<bool, SystemProxyError> {
    Ok(number(settings, key)?.unwrap_or(0) != 0)
}

fn string(settings: &ProxySettings, key: &str) -> Result<String, SystemProxyError> {
    settings
        .find(CFString::new(key))
        .and_then(|value| value.downcast::<CFString>())
        .map(|value| value.to_string())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| error(format!("Missing or invalid macOS proxy setting: {key}")))
}

fn endpoint(settings: &ProxySettings, protocol: &str) -> Result<(String, u16), SystemProxyError> {
    let host = string(settings, &format!("{protocol}Proxy"))?;
    let port = number(settings, &format!("{protocol}Port"))?
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port != 0)
        .ok_or_else(|| error(format!("Missing or invalid macOS {protocol} proxy port")))?;
    Ok((host.trim().to_owned(), port))
}

fn read_settings(settings: &ProxySettings) -> Result<SystemProxy, SystemProxyError> {
    // An explicit PAC takes precedence; ProxyAutoDiscoveryEnable (WPAD) is ignored.
    if enabled(settings, "ProxyAutoConfigEnable")? {
        return Ok(SystemProxy::Auto {
            url: string(settings, "ProxyAutoConfigURLString")?,
        });
    }
    if enabled(settings, "SOCKSEnable")? {
        return Err(error("SOCKS system proxies are not supported"));
    }
    let http = enabled(settings, "HTTPEnable")?;
    let https = enabled(settings, "HTTPSEnable")?;
    if !http && !https {
        return Ok(SystemProxy::Direct);
    }
    if !http || !https {
        return Err(error(
            "Configure the same proxy for both HTTP and HTTPS in macOS settings",
        ));
    }
    let (host, port) = endpoint(settings, "HTTP")?;
    let (https_host, https_port) = endpoint(settings, "HTTPS")?;
    if !host.eq_ignore_ascii_case(&https_host) || port != https_port {
        return Err(error("Separate HTTP and HTTPS system proxies are not supported"));
    }
    let mut bypass = Vec::new();
    if let Some(value) = settings.find(CFString::new("ExceptionsList")) {
        let array = value
            .downcast::<CFArray>()
            .ok_or_else(|| error("Invalid macOS proxy exclusions"))?;
        for item in array.iter() {
            // SystemConfiguration returns an array of CF objects. Check each object's type.
            let value = unsafe { CFType::wrap_under_get_rule(*item) };
            let exception = value
                .downcast::<CFString>()
                .ok_or_else(|| error("Invalid macOS proxy exclusion"))?;
            bypass.push(exception.to_string());
        }
    }
    if enabled(settings, "ExcludeSimpleHostnames")? {
        bypass.push("<local>".into());
    }
    Ok(SystemProxy::Manual {
        host,
        port,
        bypass: bypass.join(";"),
    })
}

impl SystemProxy {
    pub fn new() -> Result<Self, SystemProxyError> {
        let store = SCDynamicStoreBuilder::new("rs-proxy-facade")
            .build()
            .ok_or_else(|| error("Could not open the macOS system configuration store"))?;
        // None means either unavailable settings or an API failure, not a confirmed DIRECT route.
        let settings = store
            .get_proxies()
            .ok_or_else(|| error("Could not read macOS proxy settings"))?;
        read_settings(&settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dictionary(entries: &[(&str, CFType)]) -> ProxySettings {
        CFDictionary::from_CFType_pairs(
            &entries
                .iter()
                .map(|(key, value)| (CFString::new(key), value.clone()))
                .collect::<Vec<_>>(),
        )
    }

    fn manual(http_port: i64, https_port: i64) -> Vec<(&'static str, CFType)> {
        vec![
            ("HTTPEnable", CFNumber::from(1).as_CFType()),
            ("HTTPSEnable", CFNumber::from(1).as_CFType()),
            ("HTTPProxy", CFString::new("proxy.example").as_CFType()),
            ("HTTPSProxy", CFString::new("proxy.example").as_CFType()),
            ("HTTPPort", CFNumber::from(http_port).as_CFType()),
            ("HTTPSPort", CFNumber::from(https_port).as_CFType()),
        ]
    }

    #[test]
    fn direct_and_wpad_only_do_not_enable_a_proxy() {
        assert!(matches!(read_settings(&dictionary(&[])).unwrap(), SystemProxy::Direct));
        let settings = dictionary(&[("ProxyAutoDiscoveryEnable", CFNumber::from(1).as_CFType())]);
        assert!(matches!(read_settings(&settings).unwrap(), SystemProxy::Direct));
    }

    #[test]
    fn explicit_pac_has_priority_and_requires_a_url() {
        let mut entries = manual(80, 8080);
        entries.push(("ProxyAutoConfigEnable", CFNumber::from(1).as_CFType()));
        assert!(read_settings(&dictionary(&entries)).is_err());
        entries.push((
            "ProxyAutoConfigURLString",
            CFString::new("https://config.example/proxy.pac").as_CFType(),
        ));
        assert!(
            matches!(read_settings(&dictionary(&entries)).unwrap(), SystemProxy::Auto { url } if url == "https://config.example/proxy.pac")
        );
    }

    #[test]
    fn common_proxy_preserves_port_80_and_exclusions() {
        let mut entries = manual(80, 80);
        entries.push(("ExcludeSimpleHostnames", CFNumber::from(1).as_CFType()));
        entries.push((
            "ExceptionsList",
            CFArray::from_CFTypes(&[CFString::new("*.example"), CFString::new("10.0.0.0/8")]).as_CFType(),
        ));
        let SystemProxy::Manual { host, port, bypass } = read_settings(&dictionary(&entries)).unwrap() else {
            panic!("Expected manual proxy")
        };
        assert_eq!(host, "proxy.example");
        assert_eq!(port, 80);
        assert_eq!(bypass, "*.example;10.0.0.0/8;<local>");
    }

    #[test]
    fn unsupported_or_malformed_settings_fail() {
        assert!(read_settings(&dictionary(&manual(80, 8080))).is_err());
        assert!(read_settings(&dictionary(&manual(0, 0))).is_err());
        assert!(read_settings(&dictionary(&manual(65536, 65536))).is_err());
        assert!(read_settings(&dictionary(&[("HTTPEnable", CFNumber::from(1).as_CFType())])).is_err());
        assert!(read_settings(&dictionary(&[("SOCKSEnable", CFNumber::from(1).as_CFType())])).is_err());
        assert!(read_settings(&dictionary(&[("HTTPEnable", CFString::new("invalid").as_CFType())])).is_err());
    }
}
