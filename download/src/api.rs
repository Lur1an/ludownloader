use reqwest::Url;
use thiserror::Error;

pub trait Download {}

pub trait Publisher<T> {
    fn publish(value: T);
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
