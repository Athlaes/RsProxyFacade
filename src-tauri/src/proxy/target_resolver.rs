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
        ProxyMode::Direct => {
            Ok(req
                .uri()
                .authority()
                .map(|auth| auth.to_string())
                .ok_or(TargetResolveError::MissingAuthority {
                    uri: req.uri().to_string(),
                })?)
        }
        ProxyMode::System => {
            let sys_proxy = system_proxy::SystemProxy::new();
            sys_proxy;
            Ok("".to_string())
        }
        ProxyMode::Manual { host, port, bypass } => {
            todo!()
        }
        ProxyMode::Pac { url } => todo!(),
    }
}
