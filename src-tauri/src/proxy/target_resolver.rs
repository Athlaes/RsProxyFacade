use http::Request;
use hyper::body::Incoming;
use thiserror::Error;

use crate::{
    commands::{ProxyConfig, ProxyMode},
    proxy::system_proxy::{self, SystemProxyError},
};

#[derive(Error, Debug)]
pub enum TargetResolveError {
    #[error("System proxy settings couldn't be resolved")]
    SystemProxyConfig(#[from] SystemProxyError),
    #[error("Authority not found in {uri}")]
    MissingAuthority { uri: String },
}

pub fn resolve_destination(req: &Request<Incoming>, config: &ProxyConfig) -> Result<String, TargetResolveError> {
    match &config.proxy_mode {
        ProxyMode::Direct => resolve_direct(req),
        ProxyMode::System => {
            let sys_proxy = system_proxy::SystemProxy::new()?;
            match sys_proxy {
                system_proxy::SystemProxy::Direct => resolve_direct(req),
                system_proxy::SystemProxy::Pac { url } => resolve_pac_url(url),
                system_proxy::SystemProxy::Manual { host, port, bypass } => resolve_manual(host, port, bypass),
            }
        }
        ProxyMode::Manual { host, port, bypass } => resolve_manual(host, port, bypass),
        ProxyMode::Pac { url } => resolve_pac_url(url),
    }
}

fn resolve_direct(req: &Request<Incoming>) -> Result<String, TargetResolveError> {
    req.uri()
        .authority()
        .map(|auth| auth.to_string())
        .ok_or(TargetResolveError::MissingAuthority {
            uri: req.uri().to_string(),
        })
}
