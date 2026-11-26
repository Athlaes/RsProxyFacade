use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use http_body_util::{BodyExt, Empty, Limited};
use hyper::body::Bytes;
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use proxy_watch::pac::PacScript;
use serde::Serialize;
use thiserror::Error;
use tokio::{
    fs::File,
    io::{self, AsyncReadExt},
};
use tracing::debug;

#[derive(Error, Debug, Serialize)]
pub enum LoaderError {
    #[error("Couldn't select loader for selector {selector}")]
    LoadingMethodSelection { selector: String },
    #[error("Couldn't load proxy pac file at path {path}")]
    Loading {
        path: String,
        #[source]
        #[serde(skip_serializing)]
        source: io::Error,
    },
}

#[async_trait]
trait Loader: Send + Sync {
    async fn load(&self, path: &str) -> Result<String, LoaderError>;
}

struct FileLoader;

struct HttpLoader;

const PAC_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_PAC_BYTES: usize = 4 * 1024 * 1024;

#[async_trait::async_trait]
impl Loader for FileLoader {
    async fn load(&self, path: &str) -> Result<String, LoaderError> {
        let path = path.strip_prefix("file://").unwrap_or(path);
        let mut file = File::open(Path::new(path)).await.map_err(|err| LoaderError::Loading {
            path: path.to_string(),
            source: err,
        })?;
        let mut buf: String = String::new();
        file.read_to_string(&mut buf)
            .await
            .map_err(|err| LoaderError::Loading {
                path: path.to_string(),
                source: err,
            })?;
        Ok(buf)
    }
}

#[async_trait::async_trait]
impl Loader for HttpLoader {
    async fn load(&self, path: &str) -> Result<String, LoaderError> {
        let download = async {
            let uri: http::Uri = path.parse().map_err(io::Error::other)?;
            let connector = HttpsConnectorBuilder::new()
                .with_native_roots()?
                .https_or_http()
                .enable_http1()
                .build();
            let client = Client::builder(TokioExecutor::new()).build::<_, Empty<Bytes>>(connector);
            let response = client.get(uri).await.map_err(io::Error::other)?;
            if !response.status().is_success() {
                return Err(io::Error::other(format!(
                    "PAC server returned HTTP {}",
                    response.status()
                )));
            }
            let body = Limited::new(response.into_body(), MAX_PAC_BYTES)
                .collect()
                .await
                .map_err(io::Error::other)?
                .to_bytes();
            String::from_utf8(body.to_vec()).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
        };
        tokio::time::timeout(PAC_DOWNLOAD_TIMEOUT, download)
            .await
            .unwrap_or_else(|_| Err(io::Error::new(io::ErrorKind::TimedOut, "PAC download timed out")))
            .map_err(|source| LoaderError::Loading {
                path: path.to_owned(),
                source,
            })
    }
}

fn get_loader(path: &str) -> Result<Box<dyn Loader>, LoaderError> {
    let path = path.split_once("://");
    let Some((left, _)) = path else {
        return Err(LoaderError::LoadingMethodSelection {
            selector: "unknown".to_string(),
        });
    };
    match left {
        "file" => Ok(Box::new(FileLoader)),
        "http" | "https" => Ok(Box::new(HttpLoader)),
        _ => Err(LoaderError::LoadingMethodSelection {
            selector: left.to_string(),
        }),
    }
}

pub async fn load_str(path: &str) -> Result<String, LoaderError> {
    let loader = get_loader(path)?;
    debug!("Loading PAC script");
    loader.load(path).await
}

pub async fn load_script(path: &str) -> Result<PacScript, LoaderError> {
    let pac_str = load_str(path).await?;
    Ok(PacScript::new(pac_str))
}
