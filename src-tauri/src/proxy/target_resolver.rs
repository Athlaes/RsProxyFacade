use http::{Request, Uri};
use hyper::body::Incoming;
use ipnet::IpNet;
use std::net::IpAddr;
use thiserror::Error;

use crate::{
    commands::{ProxyConfig, ProxyMode},
    proxy::system_proxy::{self, SystemProxyError},
};

#[derive(Error, Debug)]
pub enum TargetResolveError {
    #[error("Invalid or unsupported proxy bypass rule: {rule}")]
    InvalidBypassRule { rule: String },
    #[error("System proxy settings couldn't be resolved")]
    SystemProxyConfig(#[from] SystemProxyError),
    #[error("Invalid proxy settings {host}:{port}")]
    InvalidServerProxy { host: String, port: u16 },
    #[error("Authority not found in {uri}")]
    MissingAuthority { uri: String },
}

pub enum ResolvedDestination {
    Direct,
    HttpProxy { host: String, port: u16 },
}

pub fn resolve_destination(
    req: &Request<Incoming>,
    config: &ProxyConfig,
) -> Result<ResolvedDestination, TargetResolveError> {
    match &config.proxy_mode {
        ProxyMode::Direct => resolve_direct(req.uri()),
        ProxyMode::System => {
            let sys_proxy = system_proxy::SystemProxy::new()?;
            match &sys_proxy {
                system_proxy::SystemProxy::Direct => resolve_direct(req.uri()),
                system_proxy::SystemProxy::Pac { url } => resolve_pac_url(&url),
                system_proxy::SystemProxy::Manual { host, port, bypass } => {
                    resolve_manual(req.uri(), host, port, bypass)
                }
            }
        }
        ProxyMode::Manual { host, port, bypass } => resolve_manual(req.uri(), host, port, bypass),
        ProxyMode::Pac { url } => resolve_pac_url(url),
    }
}

fn resolve_direct(uri: &Uri) -> Result<ResolvedDestination, TargetResolveError> {
    uri.authority()
        .map(|_| ResolvedDestination::Direct)
        .ok_or(TargetResolveError::MissingAuthority { uri: uri.to_string() })
}

fn resolve_manual(uri: &Uri, host: &str, port: &u16, bypass: &str) -> Result<ResolvedDestination, TargetResolveError> {
    let destination = uri
        .host()
        .ok_or_else(|| TargetResolveError::MissingAuthority { uri: uri.to_string() })?;

    if host.trim().is_empty() || *port == 0 {
        return Err(TargetResolveError::InvalidServerProxy {
            host: host.to_owned(),
            port: port.to_owned(),
        });
    }

    if matches_bypass(destination, bypass)? {
        return Ok(ResolvedDestination::Direct);
    }

    Ok(ResolvedDestination::HttpProxy {
        host: host.to_owned(),
        port: port.to_owned(),
    })
}

/// Match a host (without a port) against semicolon/comma-separated exclusions.
/// Supports exact hosts/IPs, `*` globs, `.example.com` (domain and subdomains),
/// `<local>` (non-IP names without dots), and IPv4/IPv6 CIDRs.
/// CIDRs match literal IP destinations only: this function does not resolve DNS.
fn matches_bypass(destination: &str, bypass: &str) -> Result<bool, TargetResolveError> {
    let host = unbracket(destination).trim_end_matches('.').to_ascii_lowercase();
    let address = host.parse::<IpAddr>().ok();
    let mut matched = false;

    // Validate every rule even after a match, so malformed configuration cannot
    // accidentally authorize DIRECT depending on the ordering of exclusions.
    for rule in bypass.split([';', ',']).map(str::trim).filter(|rule| !rule.is_empty()) {
        let invalid = || TargetResolveError::InvalidBypassRule { rule: rule.to_owned() };
        let pattern = rule.to_ascii_lowercase();
        let matches = if pattern == "<local>" {
            !host.is_empty() && address.is_none() && !host.contains('.')
        } else if pattern.contains('/') {
            let network = pattern.parse::<IpNet>().map_err(|_| invalid())?;
            address.is_some_and(|ip| network.contains(&ip))
        } else if let Ok(ip) = unbracket(&pattern).parse::<IpAddr>() {
            address == Some(ip)
        } else {
            let pattern = pattern.trim_end_matches('.');
            let domain = pattern.strip_prefix('.').unwrap_or(pattern);
            if domain.is_empty()
                || domain.split('.').any(|label| label.is_empty())
                || !domain
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_*.".contains(&byte))
            {
                return Err(invalid());
            }
            if pattern.starts_with('.') {
                if domain.contains('*') {
                    return Err(invalid());
                }
                host == domain || host.ends_with(&format!(".{domain}"))
            } else {
                wildcard_matches(&host, pattern)
            }
        };
        matched |= matches;
    }
    Ok(matched)
}

fn unbracket(host: &str) -> &str {
    host.strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host)
}

// Only '*' is special; it matches zero or more characters. No regex execution.
fn wildcard_matches(host: &str, pattern: &str) -> bool {
    let (host, pattern) = (host.as_bytes(), pattern.as_bytes());
    let (mut h, mut p) = (0, 0);
    let mut star = None;
    let mut retry = 0;
    while h < host.len() {
        if p < pattern.len() && pattern[p] == b'*' {
            star = Some(p);
            p += 1;
            retry = h;
        } else if p < pattern.len() && pattern[p] == host[h] {
            h += 1;
            p += 1;
        } else if let Some(index) = star {
            retry += 1;
            h = retry;
            p = index + 1;
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|byte| *byte == b'*')
}

fn resolve_pac_url(_: &str) -> Result<ResolvedDestination, TargetResolveError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_hosts_and_globs_without_crossing_domain_boundaries() {
        for (host, rules, expected) in [
            ("example.com", "", false),
            ("EXAMPLE.com.", "localhost; example.COM", true),
            ("evil-example.com", "example.com", false),
            ("a.example.com", "*.example.com", true),
            ("example.com", "*.example.com", false),
            ("example.com", ".example.com", true),
            ("a.example.com", ".example.com", true),
            ("notexample.com", ".example.com", false),
            ("example.com.evil", "*.example.com", false),
            ("api.dev.internal", "api.*.internal", true),
            ("anything.example", "*", true),
            ("127.0.0.1", "localhost, 127.*", true),
        ] {
            assert_eq!(matches_bypass(host, rules).unwrap(), expected, "{host}: {rules}");
        }
    }

    #[test]
    fn local_means_simple_hostname_not_private_ip() {
        for (host, expected) in [
            ("printer", true),
            ("printer.", true),
            ("printer.example", false),
            ("127.0.0.1", false),
            ("[::1]", false),
        ] {
            assert_eq!(matches_bypass(host, "<LOCAL>").unwrap(), expected);
        }
    }

    #[test]
    fn matches_literal_ips_and_cidr_without_dns() {
        for (host, rule, expected) in [
            ("10.1.2.3", "10.0.0.0/8", true),
            ("11.0.0.1", "10.0.0.0/8", false),
            ("10.1.2.3", "10.1.2.3/32", true),
            ("localhost", "127.0.0.0/8", false),
            ("[2001:db8::1]", "2001:db8::/32", true),
            ("[::1]", "0:0:0:0:0:0:0:1", true),
            ("::1", "[::1]", true),
            ("::1", "0.0.0.0/0", false),
        ] {
            assert_eq!(matches_bypass(host, rule).unwrap(), expected, "{host}: {rule}");
        }
    }

    #[test]
    fn invalid_rules_fail_even_after_a_match() {
        for rule in [
            "10.0.0.0/33",
            "::/129",
            "https://example.com",
            "example.com:80",
            "<invalid>",
            "a?b",
            "a..b",
            "bad rule",
        ] {
            assert!(matches!(
                matches_bypass("example.com", &format!("*;{rule}")),
                Err(TargetResolveError::InvalidBypassRule { .. })
            ));
        }
    }

    #[test]
    fn manual_resolution_applies_bypass_to_http_and_connect() {
        for uri in ["http://api.example/path", "api.example:443"] {
            let uri: Uri = uri.parse().unwrap();
            assert!(matches!(
                resolve_manual(&uri, "proxy", &8080, "*.example").unwrap(),
                ResolvedDestination::Direct
            ));
            assert!(
                matches!(resolve_manual(&uri, "proxy", &8080, "elsewhere.example").unwrap(), ResolvedDestination::HttpProxy { host, port: 8080 } if host == "proxy")
            );
            assert!(resolve_manual(&uri, "proxy", &8080, "invalid/99").is_err());
        }
    }
}
