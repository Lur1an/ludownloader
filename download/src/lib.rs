extern crate core;

use std::sync::Arc;

use httpdownload::HttpDownload;
use reqwest::Url;
use thiserror::Error;
pub mod download_config;
pub mod httpdownload;
mod util;

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
    #[error("Download req did not yield 200, instead: '{0}'")]
    DownloadNotOk(reqwest::StatusCode),
}

pub type Result<T> = std::result::Result<T, Error>;

pub const DEFAULT_USER_AGENT: &str = "ludownloader";
pub const DEFAULT_CHUNK_SIZE: usize = 1024 * 1024;
