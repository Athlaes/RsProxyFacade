use std::{future::Future, net::SocketAddr, sync::Arc, time::Duration};

use crate::proxy::target_resolver::ResolvedDestination;
use crate::{ExecutionStatus, commands::ProxyConfig, proxy::target_resolver};
use display_error_chain::ErrorChainExt;
use http_body_util::{BodyExt, Empty, Full, combinators::BoxBody};
use hyper::{Method, Request, Response, body::Bytes, client, server, service::service_fn, upgrade::Upgraded};
use hyper_util::rt::TokioIo;
use serde::Serialize;
use tokio::{
    io,
    net::{TcpListener, TcpStream, ToSocketAddrs, lookup_host},
    task::{JoinError, JoinHandle},
    time::timeout,
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::{error, info, warn};

const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(thiserror::Error, Debug, Serialize)]
pub enum ProxyServerError {
    #[error("Couldn't start proxy server on {host}:{port}")]
    Binding {
        host: String,
        port: u16,
        #[source]
        #[serde(skip_serializing)]
        source: io::Error,
    },
    #[error("Couldn't stop proxy server")]
    Stopping(
        #[from]
        #[serde(skip_serializing)]
        JoinError,
    ),
}

pub struct ProxyServer {
    config: Arc<ProxyConfig>,
    runtime: Arc<ProxyRuntime>,
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
        self.runtime.cancel.cancel();
    }
}

impl ProxyServer {
    pub async fn new(mut config: ProxyConfig) -> Result<Self, ProxyServerError> {
        info!("Starting proxy server on http://{}:{}", &config.host, &config.port);
        let listener = TcpListener::bind((config.host.clone(), config.port))
            .await
            .map_err(|err| ProxyServerError::Binding {
                host: config.host.to_string(),
                port: config.port,
                source: err,
            })?;

        let address = listener.local_addr().map_err(|err| ProxyServerError::Binding {
            host: config.host.clone(),
            port: config.port,
            source: err,
        })?;
        config.port = address.port();
        let cancel = CancellationToken::new();
        let runtime = Arc::new(ProxyRuntime {
            listener: address,
            cancel: cancel.clone(),
            tasks: TaskTracker::new(),
        });
        let config = Arc::new(config);

        let server_config = config.clone();
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
                        error!("{}", err.chain());
                        runtime.cancel.cancel();
                        break;
                    }
                };
                let io = TokioIo::new(stream);
                let connection_runtime = runtime.clone();
                let config = config.clone();

                runtime.spawn(async move {
                    if let Err(err) = server::conn::http1::Builder::new()
                        .preserve_header_case(true)
                        .title_case_headers(true)
                        .serve_connection(
                            io,
                            service_fn(move |req| Self::proxy_req(req, config.clone(), connection_runtime.clone())),
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
            config: server_config,
            runtime: server_runtime,
            join_handle: Some(handle),
        })
    }

    pub async fn shutdown(&mut self) -> Result<(), ProxyServerError> {
        self.runtime.cancel.cancel();
        if let Some(handle) = self.join_handle.take()
            && let Err(err) = handle.await
        {
            return Err(ProxyServerError::from(err));
        }
        info!(
            "Proxy server {}:{} shutdown successfully.",
            self.config.host, self.config.port
        );
        Ok(())
    }

    pub fn proxy_status(&self) -> ExecutionStatus {
        if self.runtime.cancel.is_cancelled() || self.join_handle.as_ref().is_none_or(|handle| handle.is_finished()) {
            ExecutionStatus::Fail
        } else {
            ExecutionStatus::Success
        }
    }

    async fn proxy_req(
        mut req: Request<hyper::body::Incoming>,
        config: Arc<ProxyConfig>,
        runtime: Arc<ProxyRuntime>,
    ) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
        req.headers_mut().remove(http::header::PROXY_AUTHORIZATION);
        let route = match target_resolver::resolve_destination(&req, &config) {
            Ok(route) => route,
            Err(_) => return Ok(Self::error_response(http::StatusCode::BAD_GATEWAY)),
        };
        let connect = req.method() == Method::CONNECT;
        let Some(authority) = req.uri().authority().cloned() else {
            return Ok(Self::error_response(http::StatusCode::BAD_REQUEST));
        };
        if authority.as_str().contains('@')
            || (connect && authority.port_u16().is_none())
            || (!connect && req.uri().scheme_str() != Some("http"))
        {
            return Ok(Self::error_response(http::StatusCode::BAD_REQUEST));
        }
        let destination_host = authority.host().trim_start_matches('[').trim_end_matches(']');
        let destination_port = authority.port_u16().unwrap_or(80);
        let (peer_host, peer_port) = match &route {
            ResolvedDestination::Direct => (destination_host, destination_port),
            ResolvedDestination::HttpProxy { host, port } => {
                (host.trim_start_matches('[').trim_end_matches(']'), *port)
            }
        };
        let stream = match timeout(
            UPSTREAM_TIMEOUT,
            Self::connect_destination((peer_host, peer_port), runtime.listener),
        )
        .await
        {
            Ok(Ok(stream)) => stream,
            Ok(Err(status)) => return Ok(Self::error_response(status)),
            Err(_) => return Ok(Self::error_response(http::StatusCode::GATEWAY_TIMEOUT)),
        };

        if connect {
            match &route {
                ResolvedDestination::Direct => Self::spawn_tunnel(req, stream, &runtime),
                ResolvedDestination::HttpProxy { .. } => {
                    let tunnel = timeout(
                        UPSTREAM_TIMEOUT,
                        Self::connect_upstream(stream, authority.as_str(), &runtime),
                    )
                    .await;
                    match tunnel {
                        Ok(Ok(upgraded)) => Self::spawn_tunnel(req, TokioIo::new(upgraded), &runtime),
                        Ok(Err(status)) => return Ok(Self::error_response(status)),
                        Err(_) => return Ok(Self::error_response(http::StatusCode::GATEWAY_TIMEOUT)),
                    }
                }
            }
            return Ok(Response::new(
                Empty::<Bytes>::new()
                    .map_err(|never| -> hyper::Error { match never {} })
                    .boxed(),
            ));
        }

        // Origins expect origin-form; forward proxies expect absolute-form.
        if matches!(route, ResolvedDestination::Direct) {
            let path = req.uri().path_and_query().map(|path| path.as_str()).unwrap_or("/");
            let uri = match path.parse() {
                Ok(uri) => uri,
                Err(_) => return Ok(Self::error_response(http::StatusCode::BAD_REQUEST)),
            };
            *req.uri_mut() = uri;
        }
        Self::strip_hop_headers(req.headers_mut());
        let host_header = match http::HeaderValue::from_str(authority.as_str()) {
            Ok(value) => value,
            Err(_) => return Ok(Self::error_response(http::StatusCode::BAD_REQUEST)),
        };
        req.headers_mut().insert(http::header::HOST, host_header);
        let (mut sender, conn) = match client::conn::http1::handshake(TokioIo::new(stream)).await {
            Ok(connection) => connection,
            Err(err) => return Ok(Self::upstream_error_response(err)),
        };
        let connection_cancel = CancellationToken::new();
        let connection_guard = connection_cancel.clone().drop_guard();
        runtime.spawn(async move {
            tokio::select! {
                _ = connection_cancel.cancelled() => {},
                result = conn => if let Err(err) = result { warn!(%err, "HTTP connection failed"); },
            }
        });
        let mut response = match timeout(UPSTREAM_TIMEOUT, sender.send_request(req)).await {
            Ok(Ok(response)) => response,
            Ok(Err(err)) => return Ok(Self::upstream_error_response(err)),
            Err(_) => return Ok(Self::error_response(http::StatusCode::GATEWAY_TIMEOUT)),
        };
        if response.status() == http::StatusCode::PROXY_AUTHENTICATION_REQUIRED {
            // TODO: implement upstream authentication.
            warn!("Proxy authentication required; returning 502");
            return Ok(Self::error_response(http::StatusCode::BAD_GATEWAY));
        }
        // Upgrades other than CONNECT are not supported by this forwarding path.
        if response.status() == http::StatusCode::SWITCHING_PROTOCOLS {
            return Ok(Self::error_response(http::StatusCode::BAD_GATEWAY));
        }
        Self::strip_hop_headers(response.headers_mut());
        response.headers_mut().remove(http::header::PROXY_AUTHENTICATE);
        // Keep the driver alive for the streaming response; all earlier error paths
        // cancel it, including an authentication rejection during an upload.
        connection_guard.disarm();
        Ok(response.map(|body| body.boxed()))
    }

    async fn connect_upstream(
        stream: TcpStream,
        authority: &str,
        runtime: &ProxyRuntime,
    ) -> Result<Upgraded, http::StatusCode> {
        let (mut sender, conn) = client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .map_err(|_| http::StatusCode::BAD_GATEWAY)?;
        let connection_cancel = CancellationToken::new();
        let _connection_guard = connection_cancel.clone().drop_guard();
        runtime.spawn(async move {
            tokio::select! {
                _ = connection_cancel.cancelled() => {},
                result = conn.with_upgrades() => if let Err(err) = result { warn!(%err, "Upstream CONNECT failed"); },
            }
        });
        let request = Request::builder()
            .method(Method::CONNECT)
            .uri(authority)
            .header(http::header::HOST, authority)
            .body(Empty::<Bytes>::new())
            .map_err(|_| http::StatusCode::BAD_REQUEST)?;
        let response = sender
            .send_request(request)
            .await
            .map_err(|_| http::StatusCode::BAD_GATEWAY)?;
        if response.status() == http::StatusCode::PROXY_AUTHENTICATION_REQUIRED {
            // TODO: implement upstream authentication.
            warn!("Upstream CONNECT requires authentication; returning 502");
            return Err(http::StatusCode::BAD_GATEWAY);
        }
        if !response.status().is_success() {
            return Err(if response.status() == http::StatusCode::GATEWAY_TIMEOUT {
                http::StatusCode::GATEWAY_TIMEOUT
            } else {
                http::StatusCode::BAD_GATEWAY
            });
        }
        // Keep Hyper's buffered tunnel bytes instead of extracting the raw TCP stream.
        hyper::upgrade::on(response)
            .await
            .map_err(|_| http::StatusCode::BAD_GATEWAY)
    }

    fn spawn_tunnel<T>(req: Request<hyper::body::Incoming>, stream: T, runtime: &ProxyRuntime)
    where
        T: io::AsyncRead + io::AsyncWrite + Unpin + Send + 'static,
    {
        runtime.spawn(async move {
            match hyper::upgrade::on(req).await {
                Ok(upgraded) => {
                    if let Err(err) = Self::tunnel(upgraded, stream).await {
                        warn!(%err, "Tunnel failed");
                    }
                }
                Err(err) => warn!(%err, "Client upgrade failed"),
            }
        });
    }

    fn strip_hop_headers(headers: &mut http::HeaderMap) {
        let nominated: Vec<_> = headers
            .get_all(http::header::CONNECTION)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(','))
            .filter_map(|name| http::HeaderName::from_bytes(name.trim().as_bytes()).ok())
            .collect();
        for name in nominated {
            headers.remove(name);
        }
        for name in [
            "connection",
            "proxy-connection",
            "keep-alive",
            "te",
            "trailer",
            "transfer-encoding",
            "upgrade",
            "proxy-authorization",
        ] {
            headers.remove(name);
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

    // Forward traffic through the destination connection established before the 200.
    async fn tunnel<T: io::AsyncRead + io::AsyncWrite + Unpin>(
        upgraded: Upgraded,
        mut server: T,
    ) -> std::io::Result<()> {
        let mut upgraded = TokioIo::new(upgraded);

        // Proxying data
        let (from_client, from_server) = tokio::io::copy_bidirectional(&mut upgraded, &mut server).await?;

        // Print message when done
        println!("client wrote {} bytes and received {} bytes", from_client, from_server);

        Ok(())
    }
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
