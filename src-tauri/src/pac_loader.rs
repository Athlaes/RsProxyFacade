use std::path::Path;

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
    #[error("PAC file at {path} exceeds the size limit of {limit} bytes")]
    FileTooLarge { path: String, limit: usize },
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

const MAX_PAC_BYTES: usize = 4 * 1024 * 1024;

#[async_trait::async_trait]
impl Loader for FileLoader {
    async fn load(&self, path: &str) -> Result<String, LoaderError> {
        let loading_error = |source| LoaderError::Loading {
            path: path.to_string(),
            source,
        };
        // Older settings stored "file://" followed by a literal native path.
        let legacy = Path::new(path.strip_prefix("file://").unwrap_or(path));
        let decoded = tauri::Url::parse(path).ok().and_then(|url| url.to_file_path().ok());
        let file = match decoded {
            Some(decoded) if decoded != legacy => match File::open(decoded).await {
                Err(err) if err.kind() == io::ErrorKind::NotFound => File::open(legacy).await,
                result => result,
            },
            _ => File::open(legacy).await,
        }
        .map_err(loading_error)?;
        let mut bytes = Vec::new();
        file.take(MAX_PAC_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .await
            .map_err(loading_error)?;
        if bytes.len() > MAX_PAC_BYTES {
            return Err(LoaderError::FileTooLarge {
                path: path.to_string(),
                limit: MAX_PAC_BYTES,
            });
        }
        String::from_utf8(bytes).map_err(|err| loading_error(io::Error::new(io::ErrorKind::InvalidData, err)))
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
        download.await.map_err(|source| LoaderError::Loading {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn loads_encoded_urls_and_legacy_literal_paths() {
        let path = std::env::temp_dir().join(format!("rs-proxy-{} espace %20 #.pac", std::process::id()));
        std::fs::write(&path, "PAC content").unwrap();
        let encoded = tauri::Url::from_file_path(&path).unwrap();
        let from_url = load_str(encoded.as_str()).await;
        let from_legacy = load_str(&format!("file://{}", path.display())).await;
        std::fs::remove_file(path).unwrap();
        assert_eq!(from_url.unwrap(), "PAC content");
        assert_eq!(from_legacy.unwrap(), "PAC content");
    }

    #[tokio::test]
    async fn local_pac_size_limit_is_inclusive() {
        let path = std::env::temp_dir().join(format!("rs-proxy-pac-size-{}.pac", std::process::id()));
        let url = format!("file://{}", path.display());
        let file = std::fs::File::create_new(&path).unwrap();
        file.set_len(MAX_PAC_BYTES as u64).unwrap();
        let at_limit = load_str(&url).await;
        file.set_len(MAX_PAC_BYTES as u64 + 1).unwrap();
        let over_limit = load_str(&url).await;
        drop(file);
        std::fs::remove_file(path).unwrap();

        assert_eq!(at_limit.unwrap().len(), MAX_PAC_BYTES);
        assert!(matches!(
            over_limit,
            Err(LoaderError::FileTooLarge {
                limit: MAX_PAC_BYTES,
                ..
            })
        ));
    }
}
