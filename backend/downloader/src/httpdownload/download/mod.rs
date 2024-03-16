pub mod config;

use futures_util::StreamExt;
use reqwest::header::RANGE;
use reqwest::{Client, Response, Url};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::Sender;

use crate::util::{file_size, supports_byte_ranges, HALF_SECOND};

use self::config::HttpDownloadConfig;

use super::DownloadMetadata;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("File IO operation failed, error: '{0}'")]
    Io(#[from] tokio::io::Error),
    #[error("Request error: '{0}'")]
    Request(#[from] reqwest::Error),
    #[error("Content length not provided for url: '{0}'")]
    MissingContentLength(Url),
    #[error("Download was already finished, downloaded bytes: '{0}'")]
    DownloadComplete(u64),
    #[error("Download ended before completion, downloaded bytes: '{0}'")]
    StreamEndedBeforeCompletion(u64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum State {
    Complete,
    Paused(u64),
    Running {
        bytes_downloaded: u64,
        bytes_per_second: u64,
    },
    Error(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct DownloadUpdate {
    pub id: uuid::Uuid,
    pub state: State,
}

#[derive(Debug, Clone)]
pub struct HttpDownload {
    pub id: uuid::Uuid,
    pub url: Url,
    pub directory: PathBuf,
    pub filename: String,
    pub config: HttpDownloadConfig,
    pub client: Client,
    content_length: u64,
    supports_byte_ranges: bool,
}

impl HttpDownload {
    pub async fn start(&self, update_ch: Sender<DownloadUpdate>) -> Result<u64> {
        let resp = self
            .client
            .get(self.url.as_ref())
            .headers(self.config.headers.clone())
            .send()
            .await?;
        tracing::info!(
            url = ?self.url,
            file = ?self.file_path(),
            "Starting new download",
        );
        let file_handler = File::create(self.file_path()).await?;
        self.progress(resp, file_handler, update_ch, 0).await
    }

    pub fn file_path(&self) -> PathBuf {
        self.directory.join(&self.filename)
    }

    pub async fn resume(&self, update_ch: Sender<DownloadUpdate>) -> Result<u64> {
        let bytes_on_disk = self.get_bytes_on_disk().await;
        if bytes_on_disk == self.content_length {
            tracing::warn!(
                ?self,
                "Tried downloading a file that was already completely downloaded",
            );
            return Err(Error::DownloadComplete(bytes_on_disk));
        }
        if !self.supports_byte_ranges {
            tracing::warn!(
                ?self,
                "Tried resuming a download that doesn't support byte ranges",
            );
            tracing::info!(?self, "Starting from scratch");
            return self.start(update_ch).await;
        }

        let file_handler = OpenOptions::new()
            .write(true)
            .append(true)
            .open(self.file_path())
            .await?;

        let resp = self
            .client
            .get(self.url.as_ref())
            .headers(self.config.headers.clone())
            .header(RANGE, format!("bytes={}-", bytes_on_disk))
            .send()
            .await?;
        self.progress(resp, file_handler, update_ch, bytes_on_disk)
            .await
    }

    pub async fn create(
        url: Url,
        directory: PathBuf,
        filename: String,
        client: Client,
        config: Option<HttpDownloadConfig>,
    ) -> Result<Self> {
        // If no configuration is passed the default one is copied
        let config = config.unwrap_or_default();
        let id = uuid::Uuid::new_v4();
        let resp = client
            .get(url.as_ref())
            .timeout(config.timeout)
            .headers(config.headers.clone())
            .send()
            .await?;
        resp.error_for_status_ref()?;
        let content_length = match resp.content_length() {
            Some(val) => Ok(val),
            None => Err(Error::MissingContentLength(url.clone())),
        }?;
        let supports_byte_ranges = supports_byte_ranges(resp.headers());
        let download = HttpDownload {
            id,
            url,
            directory,
            filename: filename.to_string(),
            config,
            client,
            supports_byte_ranges,
            content_length,
        };
        Ok(download)
    }

    async fn progress(
        &self,
        resp: Response,
        mut file_handler: File,
        update_ch: Sender<DownloadUpdate>,
        mut downloaded_bytes: u64,
    ) -> Result<u64> {
        resp.error_for_status_ref()?;
        let mut stream = resp.bytes_stream();
        let mut last_update = std::time::Instant::now();
        let mut previous_bytes = 0u64;
        while let Some(chunk) = stream.next().await {
            let item = chunk?;
            let bytes_written = file_handler.write(&item).await? as u64;
            downloaded_bytes += bytes_written;
            previous_bytes += bytes_written;
            let elapsed = last_update.elapsed();
            if elapsed > HALF_SECOND {
                let _ = update_ch.try_send(DownloadUpdate {
                    id: self.id,
                    state: State::Running {
                        bytes_downloaded: downloaded_bytes,
                        bytes_per_second: previous_bytes / last_update.elapsed().as_millis() as u64
                            * 1000,
                    },
                });
                last_update = std::time::Instant::now();
                previous_bytes = 0u64;
            }
        }
        if downloaded_bytes < self.content_length {
            tracing::error!(
                ?self,
                ?downloaded_bytes,
                "Download stream ended before completiont",
            );
            return Err(Error::StreamEndedBeforeCompletion(downloaded_bytes));
        }
        tracing::info!(?self, "Download completed successfully",);
        Ok(downloaded_bytes)
    }

    pub fn get_metadata(&self) -> DownloadMetadata {
        DownloadMetadata {
            id: self.id,
            url: self.url.to_string(),
            file_path: self.file_path(),
            download_size: self.content_length,
        }
    }

    pub async fn get_bytes_on_disk(&self) -> u64 {
        file_size(&self.file_path()).await
    }
}

#[cfg(test)]
mod test {
    use std::error::Error;
    use test_log::test;

    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc;

    use crate::util::{parse_filename, test::setup_test_download};

    use super::*;

    type Test<T> = std::result::Result<T, Box<dyn Error>>;
    const TEST_DOWNLOAD_URL: &str =
        "https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb";

    #[test(tokio::test)]
    async fn server_data_is_requested_on_create_test() -> Test<()> {
        // given
        let url_str = TEST_DOWNLOAD_URL;
        let url = Url::parse(url_str)?;
        let filename = parse_filename(&url).unwrap().to_string();
        let directory = PathBuf::new();
        // when creating a download, server data is present in the download struct
        let download = HttpDownload::create(url, directory, filename, Client::new(), None).await?;
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
        let (update_sender, _) = mpsc::channel::<DownloadUpdate>(1000);
        let downloaded_bytes = download.start(update_sender).await?;
        // then
        assert_eq!(
            download.content_length,
            file_size(&download.file_path()).await,
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
        let (update_sender, _) = mpsc::channel::<DownloadUpdate>(1000);
        let downloaded_bytes = download.start(update_sender).await?;
        // then
        assert_eq!(
            download.content_length,
            file_size(&download.file_path()).await,
            "File size should be equal to content_length"
        );
        assert_eq!(
            downloaded_bytes,
            download.content_length,
            "The downloaded bytes need to be equal to the content_length when the download is finished"
        );
        Ok(())
    }
}
