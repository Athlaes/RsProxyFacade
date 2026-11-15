use std::{future::Future, net::SocketAddr, sync::Arc};

use crate::ExecutionStatus;
use http_body_util::{BodyExt, Empty, Full, combinators::BoxBody};
use hyper::{Method, Request, Response, body::Bytes, client, server, service::service_fn, upgrade::Upgraded};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, TcpStream, ToSocketAddrs, lookup_host},
    task::JoinHandle,
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::{error, info, warn};

use crate::commands::CommandError;

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod lifecycle_tests;

pub struct ProxyServer {
    pub config: Arc<ProxyConfig>,
    cancel: CancellationToken,
    join_handle: Option<JoinHandle<()>>,
}

// Shared by the listener, HTTP connections and CONNECT tunnels.
#[derive(Clone)]
struct ProxyRuntime {
    listener: SocketAddr,
    cancel: CancellationToken,
    tasks: TaskTracker,
}

impl ProxyRuntime {
    fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        let cancel = self.cancel.clone();
        self.tasks.spawn(async move {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {},
                _ = future => {},
            }
        });
    }
}

impl Drop for ProxyServer {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyConfig {
    pub host: String,
    pub port: u16,
    pub proxy_mode: ProxyMode,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProxyMode {
    Direct,
    System,
    Manual {
        host: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
    },
    Pac {
        url: String,
    },
}

impl ProxyServer {
    pub async fn new(mut config: ProxyConfig) -> Result<Self, CommandError> {
        info!("Starting proxy server on http://{}:{}", &config.host, &config.port);
        let listener = TcpListener::bind((config.host.clone(), config.port))
            .await
            .map_err(|err| CommandError::ServerStart {
                host: config.host.to_string(),
                port: config.port,
                cause: err,
            })?;

        let address = listener.local_addr().map_err(|cause| CommandError::ServerStart {
            host: config.host.clone(),
            port: config.port,
            cause,
        })?;
        config.port = address.port();
        let cancel = CancellationToken::new();
        let runtime = ProxyRuntime {
            listener: address,
            cancel: cancel.clone(),
            tasks: TaskTracker::new(),
        };

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
                        error!(%err, "Proxy listener stopped after an accept error");
                        runtime.cancel.cancel();
                        break;
                    }
                };
                let io = TokioIo::new(stream);
                let connection_runtime = runtime.clone();

                runtime.spawn(async move {
                    if let Err(err) = server::conn::http1::Builder::new()
                        .preserve_header_case(true)
                        .title_case_headers(true)
                        .serve_connection(
                            io,
                            service_fn(move |req| Self::proxy_req(req, connection_runtime.clone())),
                        )
                        .with_upgrades()
                        .await
                    {
                        warn!(%err, "Proxy client connection failed");
                    }
                });
            }
            drop(listener);
            runtime.tasks.close();
            runtime.tasks.wait().await;
        });
        Ok(ProxyServer {
            config: Arc::new(config),
            cancel,
            join_handle: Some(handle),
        })
    }

    pub async fn shutdown(&mut self) {
        self.cancel.cancel();
        if let Some(handle) = self.join_handle.take() {
            if let Err(err) = handle.await {
                error!(%err, "Proxy listener task failed during shutdown");
            }
        }
        info!(
            "Proxy server {}:{} shutdown successfully.",
            self.config.host, self.config.port
        )
    }

    pub fn proxy_status(&self) -> ExecutionStatus {
        if self.cancel.is_cancelled() || self.join_handle.as_ref().is_none_or(|handle| handle.is_finished()) {
            return ExecutionStatus::Fail;
        } else {
            return ExecutionStatus::Success;
        }
    }

    async fn proxy_req(
        mut req: Request<hyper::body::Incoming>,
        runtime: ProxyRuntime,
    ) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
        // Client proxy credentials must never be forwarded to the destination.
        req.headers_mut().remove(http::header::PROXY_AUTHORIZATION);

        if Method::CONNECT == req.method() {
            // Received an HTTP request like:
            // ```
            // CONNECT www.domain.com:443 HTTP/1.1
            // Host: www.domain.com:443
            // Proxy-Connection: Keep-Alive
            // ```
            //
            // When HTTP method is CONNECT we should return an empty body
            // then we can eventually upgrade the connection and talk a new protocol.
            //
            // Note: only after client received an empty body with STATUS_OK can the
            // connection be upgraded, so we can't return a response inside
            // `on_upgrade` future.
            if let Some(addr) = Self::host_addr(req.uri()) {
                // Establish the destination connection before acknowledging CONNECT.
                let stream = match Self::connect_destination(addr, runtime.listener).await {
                    Ok(stream) => stream,
                    Err(status) => return Ok(Self::error_response(status)),
                };
                runtime.spawn(async move {
                    match hyper::upgrade::on(req).await {
                        Ok(upgraded) => {
                            if let Err(e) = Self::tunnel(upgraded, stream).await {
                                eprintln!("server io error: {}", e);
                            };
                        }
                        Err(e) => eprintln!("upgrade error: {}", e),
                    }
                });

                Ok(Response::new(
                    Empty::<Bytes>::new()
                        .map_err(|never| -> hyper::Error { match never {} })
                        .boxed(),
                ))
            } else {
                warn!("CONNECT request has no destination authority");
                let mut resp = Response::new(
                    Full::<Bytes>::new("CONNECT must be to a socket address".into())
                        .map_err(|never| -> hyper::Error { match never {} })
                        .boxed(),
                );
                *resp.status_mut() = http::StatusCode::BAD_REQUEST;

                Ok(resp)
            }
        } else {
            let Some(host) = req.uri().host() else {
                return Ok(Self::error_response(http::StatusCode::BAD_REQUEST));
            };
            let port = req.uri().port_u16().unwrap_or(80);

            let host = host.trim_start_matches('[').trim_end_matches(']');
            let stream = match Self::connect_destination((host, port), runtime.listener).await {
                Ok(stream) => stream,
                Err(status) => return Ok(Self::error_response(status)),
            };
            let io = TokioIo::new(stream);

            let (mut sender, conn) = match client::conn::http1::Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .handshake(io)
                .await
            {
                Ok(connection) => connection,
                Err(err) => return Ok(Self::upstream_error_response(err)),
            };
            runtime.spawn(async move {
                if let Err(err) = conn.await {
                    println!("Connection failed: {:?}", err);
                }
            });

            let resp = match sender.send_request(req).await {
                Ok(response) => response,
                Err(err) => return Ok(Self::upstream_error_response(err)),
            };
            Ok(resp.map(|b| b.boxed()))
        }
    }

    async fn connect_destination(
        address: impl ToSocketAddrs,
        listener: SocketAddr,
    ) -> Result<TcpStream, http::StatusCode> {
        let addresses: Vec<_> = lookup_host(address)
            .await
            .map_err(Self::connection_error_status)?
            .collect();
        // Check all candidates before connecting. Reuse these IPs to avoid a
        // second DNS lookup returning an unchecked listener address.
        if addresses.iter().any(|address| Self::is_listener(*address, listener)) {
            warn!("Blocked a destination pointing back to the proxy listener");
            return Err(http::StatusCode::FORBIDDEN);
        }
        TcpStream::connect(addresses.as_slice())
            .await
            .map_err(Self::connection_error_status)
    }

    fn is_listener(address: SocketAddr, listener: SocketAddr) -> bool {
        if address.port() != listener.port() {
            return false;
        }
        let destination = address.ip().to_canonical();
        let local = listener.ip().to_canonical();
        if destination == local || destination.is_unspecified() {
            return true;
        }
        // A wildcard listener also accepts connections on local interface IPs.
        // Binding an ephemeral UDP socket checks local ownership without traffic.
        local.is_unspecified()
            && (destination.is_loopback() || std::net::UdpSocket::bind(SocketAddr::new(destination, 0)).is_ok())
    }

    fn connection_error_status(err: std::io::Error) -> http::StatusCode {
        warn!(%err, "Destination resolution or connection failed");
        if err.kind() == std::io::ErrorKind::TimedOut {
            http::StatusCode::GATEWAY_TIMEOUT
        } else {
            http::StatusCode::BAD_GATEWAY
        }
    }

    fn upstream_error_response(err: hyper::Error) -> Response<BoxBody<Bytes, hyper::Error>> {
        warn!(%err, "Destination HTTP exchange failed");
        Self::error_response(if err.is_timeout() {
            http::StatusCode::GATEWAY_TIMEOUT
        } else {
            http::StatusCode::BAD_GATEWAY
        })
    }

    fn error_response(status: http::StatusCode) -> Response<BoxBody<Bytes, hyper::Error>> {
        let message = match status {
            http::StatusCode::BAD_REQUEST => "Invalid proxy request",
            http::StatusCode::FORBIDDEN => "Destination is the local proxy listener",
            http::StatusCode::GATEWAY_TIMEOUT => "Destination timed out",
            _ => "Destination unavailable",
        };
        let mut response = Response::new(
            Full::new(Bytes::from_static(message.as_bytes()))
                .map_err(|never| match never {})
                .boxed(),
        );
        *response.status_mut() = status;
        // An upload may still have unread bytes when its destination fails.
        response
            .headers_mut()
            .insert(http::header::CONNECTION, http::HeaderValue::from_static("close"));
        response
    }

    fn host_addr(uri: &http::Uri) -> Option<String> {
        uri.authority().map(|auth| auth.to_string())
    }

    // Forward traffic through the destination connection established before the 200.
    async fn tunnel(upgraded: Upgraded, mut server: TcpStream) -> std::io::Result<()> {
        let mut upgraded = TokioIo::new(upgraded);

        // Proxying data
        let (from_client, from_server) = tokio::io::copy_bidirectional(&mut upgraded, &mut server).await?;

        // Print message when done
        println!("client wrote {} bytes and received {} bytes", from_client, from_server);

        Ok(())
    }
}
