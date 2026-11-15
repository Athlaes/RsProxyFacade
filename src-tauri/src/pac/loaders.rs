use std::path::Path;

use async_trait::async_trait;
use tokio::{
    fs::File,
    io::{self, AsyncReadExt},
};

use crate::commands::CommandError;

#[async_trait]
pub trait Loader: Send + Sync {
    async fn load(&self, path: &str) -> Result<String, io::Error>;
}

struct FileLoader;

struct HttpLoader;

#[async_trait::async_trait]
impl Loader for FileLoader {
    async fn load(&self, path: &str) -> Result<String, io::Error> {
        let path = path.strip_prefix("file://").unwrap_or(path);
        let mut file = File::open(Path::new(path)).await?;
        let mut buf: String = String::new();
        file.read_to_string(&mut buf).await?;
        Ok(buf)
    }
}

pub fn get_loader(path: &str) -> Result<Box<dyn Loader>, CommandError> {
    let path = path.split_once("://");
    let Some((left, _)) = path else {
        return Err(CommandError::LoadingMethodSelection {
            selector: "unknown".to_string(),
        });
    };
    if left == "file" {
        return Ok(Box::new(FileLoader));
    }
    Err(CommandError::LoadingMethodSelection {
        selector: left.to_string(),
    })
}
