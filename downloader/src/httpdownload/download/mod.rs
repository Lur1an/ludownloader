pub mod config;

use thiserror::Error;
use futures_util::StreamExt;
use reqwest::header::RANGE;
use reqwest::{Client, Response, Url};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::oneshot::error::TryRecvError;
use tokio::sync::{mpsc, oneshot};

use crate::util::{file_size, supports_byte_ranges, mb};

use self::config::HttpDownloadConfig;
    
#[derive(Debug, Error)]
pub enum Error {
    #[error("File IO operation failed, error: '{0}'")]
    Io(#[from] tokio::io::Error),
    #[error("Request error: '{0}'")]
    Request(#[from] reqwest::Error),
    #[error("Content length not provided for url: '{0}'")]
    MissingContentLength(Url),
    #[error("Prematurely dropped channel for download with url: '{0}', downloaded bytes before drop: '{1}'")]
    ChannelDrop(u64, Url),
    #[error("Download was already finished, downloaded bytes: '{0}'")]
    DownloadComplete(u64),
    #[error("Download req did not yield 200, instead: '{0}', body: '{1}'")]
    DownloadNotOk(reqwest::StatusCode, String),
    #[error("Download ended before completion, downloaded bytes: '{0}'")]
    StreamEndedBeforeCompletion(u64)
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct DownloadUpdate {
    pub id: uuid::Uuid,
    pub update_type: UpdateType,
}

#[derive(Debug)]
pub enum UpdateType {
    Complete,
    Paused,
    Running {
        bytes_downloaded: u64,
        bytes_per_second: u64,
    },
    Error(Error)
}

#[derive(Debug, Clone)]
pub struct HttpDownload {
    pub url: Url,
    pub id: uuid::Uuid,
    pub file_path: PathBuf,
    pub config: HttpDownloadConfig,
    pub content_length: u64,
    pub supports_byte_ranges: bool,
    pub client: Client,
}

impl HttpDownload {
    pub async fn start(
        &self,
        stop_ch: oneshot::Receiver<()>,
        update_ch: mpsc::Sender<DownloadUpdate>,
    ) -> Result<u64> {
        let resp = self
            .client
            .get(self.url.as_ref())
            .headers(self.config.headers.clone())
            .send()
            .await?;
        let file_handler = File::create(&self.file_path).await?;
        self.progress(resp, file_handler, stop_ch, update_ch, 0).await
    }


    pub async fn resume(
        &self,
        stop_ch: oneshot::Receiver<()>,
        update_ch: mpsc::Sender<DownloadUpdate>,
    ) -> Result<u64> {
        let bytes_on_disk = self.get_bytes_on_disk().await;
        if bytes_on_disk == self.content_length {
            log::warn!(
                "Tried downloading a file that was already completely downloaded: {}",
                self.url
            );
            return Err(Error::DownloadComplete(bytes_on_disk));
        }
        if !self.supports_byte_ranges {
            log::warn!(
                "Tried resuming a download that doesn't support byte ranges: {}",
                self.url
            );
            log::info!("Starting from scratch: {}", self.url);
            return self.start(stop_ch, update_ch).await;
        }
        let file_handler = OpenOptions::new()
            .write(true)
            .append(true)
            .open(&self.file_path)
            .await?;

        let resp = self
            .client
            .get(self.url.as_ref())
            .headers(self.config.headers.clone())
            .header(RANGE, format!("bytes={}-", bytes_on_disk))
            .send()
            .await?;
        Ok(self.progress(resp, file_handler, stop_ch, update_ch, bytes_on_disk).await?)
    }

    pub async fn new(
        url: Url,
        file_path: PathBuf,
        client: Client,
        config: Option<HttpDownloadConfig>,
    ) -> Result<Self> {
        // If no configuration is passed the default one is copied
        let config = config.unwrap_or_default();
        let id = uuid::Uuid::new_v4();
        let mut download = HttpDownload {
            id,
            url,
            file_path,
            config,
            client,
            supports_byte_ranges: false,
            content_length: 0u64,
        };
        download.update_server_data().await?;
        Ok(download)
    }

    async fn progress(
        &self,
        resp: Response,
        mut file_handler: File,
        mut stop_ch: oneshot::Receiver<()>,
        update_ch: mpsc::Sender<DownloadUpdate>,
        mut downloaded_bytes: u64,
    ) -> Result<u64> {
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let item = chunk?;
            let bytes_written = file_handler.write(&item).await? as u64;
            downloaded_bytes += bytes_written;
            let _ = update_ch.try_send(
                DownloadUpdate { 
                    id: self.id,
                    update_type: UpdateType::Running {
                        bytes_downloaded: downloaded_bytes,
                        bytes_per_second: 0u64, // TODO: measure download speed
                    }
                }
            );
            match stop_ch.try_recv() {
                Ok(_) => {
                    log::info!("Download stop signal received for: {}", self.url);
                    return Ok(downloaded_bytes);
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Closed) => {
                    log::error!("Download stop signal channel closed for: {}, this shouldn't happen!", self.url);
                    log::info!("Stopping download because of channel error: {}", self.url);
                    return Err(Error::ChannelDrop(downloaded_bytes, self.url.clone()));
                }
            }
        }
        if downloaded_bytes < self.content_length {
            log::error!(
                "Download stream ended before completion, downloaded bytes: {}, content length: {}",
                downloaded_bytes,
                self.content_length
            );
            return Err(Error::StreamEndedBeforeCompletion(downloaded_bytes));
        }
        log::info!(
            "Download completed successfully: {}, {}MB",
            self.url,
            mb(downloaded_bytes)
        );
        Ok(downloaded_bytes)
    }

    /**
    Queries the server to update Download metadata.
    * updates content_length
    * updates accepts_bytes
     */
    async fn update_server_data(&mut self) -> Result<()> {
        let resp = self
            .client
            .get(self.url.as_ref())
            .timeout(self.config.timeout)
            .headers(self.config.headers.clone())
            .send()
            .await?;

        let status = resp.status();
        match status {
            reqwest::StatusCode::OK => {}
            _ => {
                let body = resp.text().await.unwrap_or_default();
                return Err(Error::DownloadNotOk(status, body))
            },
        };

        match resp.content_length() {
            Some(val) => self.content_length = val,
            None => Err(Error::MissingContentLength(self.url.clone()))?,
        }
        self.supports_byte_ranges = supports_byte_ranges(resp.headers());
        Ok(())
    }

    pub async fn get_bytes_on_disk(&self) -> u64 {
        file_size(&self.file_path).await
    }
}

#[cfg(test)]
mod test {
    use std::error::Error;
    use std::iter::zip;
    use std::sync::Arc;
    use test_log::test;

    use pretty_assertions::assert_eq;

    use crate::util::{parse_filename, setup_test_download};

    use super::*;

    type Test<T> = std::result::Result<T, Box<dyn Error>>;
    const TEST_DOWNLOAD_URL: &str =
        "https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb";


    #[test(tokio::test)]
    async fn concurrent_download_test() -> Test<()> {
        let mut handles = Vec::new();
        let mut downloads = Vec::new();
        // Needed because if the tmp dir is dropped it is actually deleted in the Drop impl
        let mut anti_drop = Vec::new();
        for _ in 0..3 {
            let (download, _tmp_dir) = setup_test_download(TEST_DOWNLOAD_URL).await.unwrap();
            let download = Arc::new(download);
            let (tx, rx) = tokio::sync::oneshot::channel();
            let (update_sender, _) = mpsc::channel::<DownloadUpdate>(1000);
            downloads.push(download.clone());
            let fut = async move { download.start(rx, update_sender).await };
            let handle = tokio::spawn(fut);
            anti_drop.push((_tmp_dir, tx));
            handles.push(handle);
        }
        let results = futures::future::join_all(handles).await;
        for (download, result) in zip(downloads, results) {
            let downloaded_bytes = result??;
            assert_eq!(
                download.content_length,
                download.get_bytes_on_disk().await,
                "File size should be equal to content_length"
            );
            assert_eq!(
                downloaded_bytes,
                download.content_length,
                "The downloaded bytes need to be equal to the content_length when the download is finished"
            );
        }
        Ok(())
    }

    #[test(tokio::test)]
    async fn server_data_is_requested_on_create_test() -> Test<()> {
        // given
        let url_str = TEST_DOWNLOAD_URL;
        let url = Url::parse(url_str)?;
        let file_path = PathBuf::from(parse_filename(&url).unwrap());
        // when creating a download, server data is present in the download struct
        let download = HttpDownload::new(url, file_path, Client::new(), None).await?;
        // then
        assert!(
            download.supports_byte_ranges,
            "Server should support bytes!"
        );
        Ok(())
    }

    #[test(tokio::test)]
    async fn default_download_test() -> Test<()> {
        // given
        let (download, _tmp_dir) = setup_test_download(TEST_DOWNLOAD_URL).await?;
        // when
        let (_tx, rx) = tokio::sync::oneshot::channel();
        let (update_sender, _) = mpsc::channel::<DownloadUpdate>(1000);
        let downloaded_bytes = download.start(rx, update_sender).await?;
        // then
        assert_eq!(
            download.content_length,
            file_size(&download.file_path).await,
            "File size should be equal to content_length"
        );
        assert_eq!(
            downloaded_bytes,
            download.content_length,
            "The downloaded bytes need to be equal to the content_length when the download is finished"
        );
        Ok(())
    }

    #[test(tokio::test)]
    async fn download_with_custom_chunksize_test() -> Test<()> {
        // given
        let mut config = HttpDownloadConfig::default();
        config.chunk_size = 1024 * 1029;
        // and
        let (mut download, _tmp_dir) = setup_test_download(TEST_DOWNLOAD_URL).await?;
        download.config = config;
        // when
        let (_tx, rx) = tokio::sync::oneshot::channel();
        let (update_sender, _) = mpsc::channel::<DownloadUpdate>(1000);
        let downloaded_bytes = download.start(rx, update_sender).await?;
        // then
        assert_eq!(
            download.content_length,
            file_size(&download.file_path).await,
            "File size should be equal to content_length"
        );
        assert_eq!(
            downloaded_bytes,
            download.content_length,
            "The downloaded bytes need to be equal to the content_length when the download is finished"
        );
        Ok(())
    }

    #[test(tokio::test)]
    async fn download_can_be_stopped_and_resumed_test() -> Test<()> {
        // setup
        let (download, _tmp_dir) = setup_test_download(TEST_DOWNLOAD_URL).await?;
        let (tx, rx) = tokio::sync::oneshot::channel();
        let content_length = download.content_length;
        let (update_sender, _) = mpsc::channel::<DownloadUpdate>(1000);
        let download = Arc::new(download);
        let download_clone = download.clone();
        let sender_clone = update_sender.clone();
        let handle = tokio::spawn(async move { download_clone.start(rx, sender_clone).await });
        tx.send(()).expect("Message needs to be sent");
        let join_result = handle.await;
        let downloaded_bytes = join_result??;
        assert!(
            downloaded_bytes < content_length,
            "The downloaded bytes need to be less than the content_length when the download is stopped prematurely"
        );
        // Start the download again
        let (_tx, rx) = tokio::sync::oneshot::channel();
        let downloaded_bytes = download.resume(rx, update_sender).await?;
        let bytes_on_disk = download.get_bytes_on_disk().await;
        assert_eq!(
            downloaded_bytes, 
            content_length,
            "The downloaded bytes need to be equal to the content_length when the download is finished"
        );
        assert_eq!(
            bytes_on_disk, 
            content_length,
            "The bytes on disk need to be equal to the content_length when the download is finished"
        );
        Ok(())
    }
}
