use http::{Method, Request, Uri};
use hyper::body::Incoming;
use ipnet::IpNet;
use proxy_watch::{
    ProxyStep,
    pac::{self, PacPolicy, PacScript},
};
use std::net::IpAddr;
use thiserror::Error;

use crate::proxy::{
    server::{ProxyMode, ProxyServerConfig},
    system_proxy::{self, SystemProxyError},
};

#[derive(Error, Debug)]
pub enum TargetResolveError {
    #[error("Unable to resolve system proxy settings")]
    SystemProxySettingsResolution(#[from] SystemProxyError),
    #[error("Invalid or unsupported proxy bypass rule: {rule}")]
    InvalidBypassRule { rule: String },
    #[error("Invalid proxy settings {host}:{port}")]
    InvalidServerProxy { host: String, port: u16 },
    #[error("Authority not found in {uri}")]
    MissingAuthority { uri: String },
    #[error("Missing PAC file data")]
    MissingPacFile,
    #[error("Invalid request destination for PAC evaluation")]
    InvalidPacDestination,
    #[error("PAC returned an invalid HTTP proxy")]
    InvalidPacProxy,
    #[error("PAC returned a proxy protocol unsupported by this transport")]
    UnsupportedPacProxy,
    #[error("PAC evaluation failed")]
    PacEvaluation(#[from] proxy_watch::Error),
    #[error("PAC evaluation task failed")]
    PacTask(#[from] tokio::task::JoinError),
}

pub enum ResolvedDestination {
    Direct,
    HttpProxy { host: String, port: u16 },
}

pub async fn resolve_destination(
    req: &Request<Incoming>,
    config: &ProxyServerConfig,
) -> Result<Vec<ResolvedDestination>, TargetResolveError> {
    match &config.proxy_mode {
        ProxyMode::Direct => resolve_direct(req.uri()),
        ProxyMode::Manual { host, port, bypass } => resolve_manual(req.uri(), host, port, bypass),
        ProxyMode::Pac { pac_file, .. } => resolve_pac(&pac_file, req.uri(), req.method()).await,
    }
}

fn resolve_direct(uri: &Uri) -> Result<Vec<ResolvedDestination>, TargetResolveError> {
    uri.authority()
        .map(|_| vec![ResolvedDestination::Direct])
        .ok_or(TargetResolveError::MissingAuthority { uri: uri.to_string() })
}

fn resolve_manual(
    uri: &Uri,
    host: &str,
    port: &u16,
    bypass: &str,
) -> Result<Vec<ResolvedDestination>, TargetResolveError> {
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
        return Ok(vec![ResolvedDestination::Direct]);
    }

    Ok(vec![ResolvedDestination::HttpProxy {
        host: host.to_owned(),
        port: port.to_owned(),
    }])
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

async fn resolve_pac(
    pac_content: &PacScript,
    uri: &Uri,
    method: &Method,
) -> Result<Vec<ResolvedDestination>, TargetResolveError> {
    let authority = uri.authority().ok_or(TargetResolveError::InvalidPacDestination)?;

    if authority.as_str().contains('@') {
        return Err(TargetResolveError::InvalidPacDestination);
    }

    // CONNECT fournit une autorité, par exemple "intranet.example:443".
    let destination = if method == Method::CONNECT {
        if authority.port_u16().is_none() {
            return Err(TargetResolveError::InvalidPacDestination);
        }
        format!("https://{authority}/")
    } else {
        if uri.scheme_str() != Some("http") {
            return Err(TargetResolveError::InvalidPacDestination);
        }
        uri.to_string()
    };

    let url = proxy_watch::Url::parse(&destination).map_err(|_| TargetResolveError::InvalidPacDestination)?;

    // L'évaluation JavaScript est synchrone. Libérer le worker Tokio
    // pendant son exécution permet aux autres connexions de progresser.
    let script = pac_content.clone();
    let steps = tokio::task::spawn_blocking(move || pac::evaluate(&script, &url, &PacPolicy::new())).await??;

    steps
        .into_iter()
        .map(|step| match step {
            ProxyStep::Direct => Ok(ResolvedDestination::Direct),
            ProxyStep::Http(endpoint) => {
                let authority = endpoint
                    .authority()
                    .parse::<http::uri::Authority>()
                    .map_err(|_| TargetResolveError::InvalidPacProxy)?;
                let port = authority
                    .port_u16()
                    .filter(|port| *port != 0)
                    .ok_or(TargetResolveError::InvalidPacProxy)?;

                Ok(ResolvedDestination::HttpProxy {
                    host: authority.host().to_owned(),
                    port,
                })
            }
            _ => Err(TargetResolveError::UnsupportedPacProxy),
        })
        .collect()
}

#[cfg(test)]
#[path = "target_resolver.test.rs"]
mod tests;
