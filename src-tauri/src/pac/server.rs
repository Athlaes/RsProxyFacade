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
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::{error, info, warn};

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
    runtime: PacRuntime,
    join_handle: Option<JoinHandle<()>>,
}

#[derive(Clone)]
struct PacRuntime {
    cancel: CancellationToken,
    tasks: TaskTracker,
}

impl Drop for ProxyPacServer {
    fn drop(&mut self) {
        self.runtime.cancel.cancel();
    }
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
        let runtime = PacRuntime {
            cancel: CancellationToken::new(),
            tasks: TaskTracker::new(),
        };
        let server_runtime = runtime.clone();
        let handle = tokio::task::spawn(async move {
            loop {
                let accepted = tokio::select! {
                    biased;
                    _ = runtime.cancel.cancelled() => break,
                    accepted = listener.accept() => accepted,
                };
                let (stream, _) = match accepted {
                    Ok(connection) => connection,
                    Err(err) => {
                        error!(%err, "PAC listener failed");
                        runtime.cancel.cancel();
                        break;
                    }
                };
                let io = TokioIo::new(stream);
                let server_config = Arc::clone(&server_config);
                let pac_content = Arc::clone(&pac_content_shared);

                let cancel = runtime.cancel.clone();
                runtime.tasks.spawn(async move {
                    let connection = hyper::server::conn::http1::Builder::new()
                        .preserve_header_case(true)
                        .title_case_headers(true)
                        .serve_connection(
                            io,
                            service_fn(move |req| {
                                Self::serve_pac(req, Arc::clone(&server_config), Arc::clone(&pac_content))
                            }),
                        )
                        .with_upgrades();
                    tokio::select! {
                        biased;
                        _ = cancel.cancelled() => {},
                        result = connection => {
                            if let Err(err) = result {
                                warn!(%err, "PAC client connection failed");
                            }
                        },
                    }
                });
            }
            drop(listener);
            runtime.tasks.close();
            runtime.tasks.wait().await;
        });
        Ok(ProxyPacServer {
            config,
            runtime: server_runtime,
            join_handle: Some(handle),
        })
    }

    pub async fn shutdown(&mut self) {
        self.runtime.cancel.cancel();
        if let Some(handle) = self.join_handle.take()
            && let Err(err) = handle.await
        {
            error!(%err, "PAC listener task failed during shutdown");
        }
        info!(
            "Proxy server {}:{} shutdown successfully.",
            self.config.host, self.config.port
        )
    }

    pub fn proxy_status(&self) -> ExecutionStatus {
        if self.runtime.cancel.is_cancelled() || self.join_handle.as_ref().is_none_or(|handle| handle.is_finished()) {
            ExecutionStatus::Fail
        } else {
            ExecutionStatus::Success
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
