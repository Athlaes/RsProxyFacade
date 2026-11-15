use std::{convert::Infallible, sync::Arc};

use http::{
    Method, Request, Response, StatusCode,
    header::{ALLOW, CACHE_CONTROL, CONTENT_TYPE},
};
use http_body_util::Full;
use hyper::{
    body::{Bytes, Incoming},
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use serde::Deserialize;
use tokio::{net::TcpListener, task::JoinHandle};
use tracing::info;

use crate::{ExecutionStatus, commands::CommandError, pac::loaders};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyPacServerConfig {
    host: String,
    port: u16,
    pac_url: String,
}

pub struct ProxyPacServer {
    config: Arc<ProxyPacServerConfig>,
    join_handle: JoinHandle<()>,
}

impl ProxyPacServer {
    pub async fn new(config: ProxyPacServerConfig) -> Result<ProxyPacServer, CommandError> {
        let config = Arc::new(config);
        let loader = loaders::get_loader(&config.pac_url)?;
        let pac_content = loader
            .load(&config.pac_url)
            .await
            .map_err(|err| CommandError::PacLoading {
                path: config.pac_url.clone(),
                cause: err,
            })?;

        let pac_content_shared = Arc::new(pac_content);
        let server_config = Arc::clone(&config);
        info!("Starting proxy_pac_server on http://{}:{}", &config.host, &config.port);
        let listener = TcpListener::bind((config.host.clone(), config.port))
            .await
            .map_err(|err| CommandError::ServerStart {
                host: config.host.to_string(),
                port: config.port,
                cause: err,
            })?;
        let handle = tokio::task::spawn(async move {
            loop {
                let (stream, _) = listener
                    .accept()
                    .await
                    .unwrap_or_else(|err| panic!("Proxy pac tcp listener unable to accept any connection : {err}"));
                let io = TokioIo::new(stream);
                let server_config = Arc::clone(&server_config);
                let pac_content = Arc::clone(&pac_content_shared);

                tokio::task::spawn(async move {
                    hyper::server::conn::http1::Builder::new()
                        .preserve_header_case(true)
                        .title_case_headers(true)
                        .serve_connection(
                            io,
                            service_fn(move |req| {
                                Self::serve_pac(req, Arc::clone(&server_config), Arc::clone(&pac_content))
                            }),
                        )
                        .with_upgrades()
                        .await
                        .unwrap_or_else(|err| panic!("Proxy server unable to process requests : {err}"));
                });
            }
        });
        Ok(ProxyPacServer {
            config: config,
            join_handle: handle,
        })
    }

    pub fn shutdown(&self) {
        self.join_handle.abort();
        info!(
            "Proxy server {}:{} shutdown successfully.",
            self.config.host, self.config.port
        )
    }

    pub fn proxy_status(&self) -> ExecutionStatus {
        if self.join_handle.is_finished() {
            return ExecutionStatus::Fail;
        } else {
            return ExecutionStatus::Success;
        }
    }

    async fn serve_pac(
        request: Request<Incoming>,
        _: Arc<ProxyPacServerConfig>,
        pac_content: Arc<String>,
    ) -> Result<Response<Full<Bytes>>, Infallible> {
        let mut response = Response::new(Full::new(Bytes::new()));

        if request.uri().path() != "/proxy.pac" {
            *response.status_mut() = StatusCode::NOT_FOUND;
            return Ok(response);
        }

        if request.method() != Method::GET && request.method() != Method::HEAD {
            *response.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
            response.headers_mut().insert(ALLOW, "GET, HEAD".parse().unwrap());
            return Ok(response);
        }

        response
            .headers_mut()
            .insert(CONTENT_TYPE, "application/x-ns-proxy-autoconfig".parse().unwrap());
        response
            .headers_mut()
            .insert(CACHE_CONTROL, "no-store".parse().unwrap());

        if request.method() == Method::GET {
            *response.body_mut() = Full::new(Bytes::copy_from_slice(pac_content.as_bytes()));
        }

        Ok(response)
    }
}
