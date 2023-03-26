use async_trait::async_trait;
use reqwest::Url;
use thiserror::Error;
use tokio::sync::oneshot::Sender;
use tokio::task::JoinHandle;

#[async_trait]
pub trait Download {
    async fn start(&self) -> Result<(JoinHandle<Result<u64>>, Sender<()>)>;
    async fn resume(&self) -> Result<(JoinHandle<Result<u64>>, Sender<()>)>;
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
    #[error("Failed sending stop signal to download: '{0}'")]
    StopFailure(Url),
    #[error("Prematurely dropped channel for download with url: '{0}', downloaded bytes before drop: '{1}'")]
    ChannelDrop(u64, Url),
    #[error("Download was already finished, downloaded bytes: '{0}'")]
    DownloadComplete(u64),
}

pub type Result<T> = std::result::Result<T, Error>;
