use std::path::Path;

use async_trait::async_trait;
use serde::Serialize;
use thiserror::Error;
use tokio::{
    fs::File,
    io::{self, AsyncReadExt},
};

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
pub trait Loader: Send + Sync {
    async fn load(&self, path: &str) -> Result<String, LoaderError>;
}

struct FileLoader;

struct HttpLoader;

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

pub fn get_loader(path: &str) -> Result<Box<dyn Loader>, LoaderError> {
    let path = path.split_once("://");
    let Some((left, _)) = path else {
        return Err(LoaderError::LoadingMethodSelection {
            selector: "unknown".to_string(),
        });
    };
    if left == "file" {
        return Ok(Box::new(FileLoader));
    }
    Err(LoaderError::LoadingMethodSelection {
        selector: left.to_string(),
    })
}
