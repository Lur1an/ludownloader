use async_trait::async_trait;
use reqwest::Url;
use thiserror::Error;

#[async_trait]
pub trait Download {
    async fn start(&self) -> Result<u64>;
    async fn stop(&self) -> Result<()>;
    async fn resume(&self) -> Result<u64>;
}

#[async_trait]
pub trait Subscriber<T> {
    async fn subscribe(&self, value: T);
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("File IO operation failed, error: '{0}'")]
    Io(#[from] tokio::io::Error),
    #[error("Request error: '{0}'")]
    Request(#[from] reqwest::Error),
    #[error("Content length not provided for url: '{0}'")]
    MissingContentLength(Url),
}

pub type Result<T> = std::result::Result<T, Error>;
