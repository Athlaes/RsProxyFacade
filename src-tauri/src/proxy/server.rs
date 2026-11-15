use std::sync::Arc;

use crate::ExecutionStatus;
use http_body_util::{BodyExt, Empty, Full, combinators::BoxBody};
use hyper::{Method, Request, Response, body::Bytes, client, server, service::service_fn, upgrade::Upgraded};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
use tracing::info;

use crate::commands::CommandError;

pub struct ProxyServer {
    pub config: Arc<ProxyConfig>,
    join_handle: JoinHandle<()>,
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
    pub async fn new(config: ProxyConfig) -> Result<Self, CommandError> {
        let config = Arc::new(config);
        let server_config = Arc::clone(&config);
        info!("Starting proxy server on http://{}:{}", &config.host, &config.port);
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
                    .unwrap_or_else(|err| panic!("Proxy tcp listener unable to accept any connection : {err}"));
                let io = TokioIo::new(stream);
                let server_config = Arc::clone(&server_config);

                tokio::task::spawn(async move {
                    server::conn::http1::Builder::new()
                        .preserve_header_case(true)
                        .title_case_headers(true)
                        .serve_connection(
                            io,
                            service_fn(move |req| Self::proxy_req(req, Arc::clone(&server_config))),
                        )
                        .with_upgrades()
                        .await
                        .unwrap_or_else(|err| panic!("Proxy server unable to process requests : {err}"));
                });
            }
        });
        Ok(ProxyServer {
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

    async fn proxy_req(
        req: Request<hyper::body::Incoming>,
        _: Arc<ProxyConfig>,
    ) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
        println!("req: {:?}", req);

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
                tokio::task::spawn(async move {
                    match hyper::upgrade::on(req).await {
                        Ok(upgraded) => {
                            if let Err(e) = Self::tunnel(upgraded, addr).await {
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
                eprintln!("CONNECT host is not socket addr: {:?}", req.uri());
                let mut resp = Response::new(
                    Full::<Bytes>::new("CONNECT must be to a socket address".into())
                        .map_err(|never| -> hyper::Error { match never {} })
                        .boxed(),
                );
                *resp.status_mut() = http::StatusCode::BAD_REQUEST;

                Ok(resp)
            }
        } else {
            let host = req.uri().host().expect("uri has no host");
            let port = req.uri().port_u16().unwrap_or(80);

            let stream = TcpStream::connect((host, port)).await.unwrap();
            let io = TokioIo::new(stream);

            let (mut sender, conn) = client::conn::http1::Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .handshake(io)
                .await?;
            tokio::task::spawn(async move {
                if let Err(err) = conn.await {
                    println!("Connection failed: {:?}", err);
                }
            });

            let resp = sender.send_request(req).await?;
            Ok(resp.map(|b| b.boxed()))
        }
    }

    fn host_addr(uri: &http::Uri) -> Option<String> {
        uri.authority().map(|auth| auth.to_string())
    }

    // Create a TCP connection to host:port, build a tunnel between the connection and
    // the upgraded connection
    async fn tunnel(upgraded: Upgraded, addr: String) -> std::io::Result<()> {
        // Connect to remote server
        let mut server = TcpStream::connect(addr).await?;
        let mut upgraded = TokioIo::new(upgraded);

        // Proxying data
        let (from_client, from_server) = tokio::io::copy_bidirectional(&mut upgraded, &mut server).await?;

        // Print message when done
        println!("client wrote {} bytes and received {} bytes", from_client, from_server);

        Ok(())
    }
}
