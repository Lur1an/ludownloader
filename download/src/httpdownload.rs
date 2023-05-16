use futures_util::StreamExt;
use reqwest::header::RANGE;
use reqwest::{Client, Response, Url};
use std::path::PathBuf;
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::oneshot::error::TryRecvError;
use tokio::sync::oneshot::{channel, Receiver, Sender};

use crate::util::{file_size, parse_filename, supports_byte_ranges};
use crate::{download_config::HttpDownloadConfig, Error, Result, DEFAULT_USER_AGENT};

#[derive(Debug, Clone)]
pub struct HttpDownload {
    /**
     * Download Link
     */
    pub url: Url,
    /**
     * Target file for the download
     */
    pub file_path: PathBuf,

    pub config: HttpDownloadConfig,
    /** Size of the download in bytes
     */
    pub content_length: u64,
    /**
    If the server for the Download supports bytes
    * This value gets updated by the struct
     */
    pub supports_byte_ranges: bool,
    /**
     * Currently used HttpClient
     */
    client: Client,
}

impl HttpDownload {
    /** Starts the Download from scratch */
    async fn start(&self, rx: Receiver<()>) -> Result<u64> {
        let resp = self
            .client
            .get(self.url.as_ref())
            .headers(self.config.headers.clone())
            .send()
            .await?;
        let file_handler = File::create(&self.file_path).await?;
        self.progress(resp, file_handler, rx).await
    }

    async fn resume(&self, rx: Receiver<()>) -> Result<u64> {
        let downloaded_bytes = self.get_bytes_on_disk().await;
        if downloaded_bytes == self.content_length {
            log::warn!(
                "Tried downloading a file that was already downloaded: {}",
                self.url
            );
            return Err(Error::DownloadComplete(downloaded_bytes));
        }
        if !self.supports_byte_ranges {
            log::warn!(
                "Tried resuming a download that doesn't support byte ranges: {}",
                self.url
            );
            log::info!("Starting from scratch: {}", self.url);
            return self.start(rx).await;
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
            .header(RANGE, format!("bytes={}-", downloaded_bytes))
            .send()
            .await?;
        self.progress(resp, file_handler, rx).await
    }

    /** Initializes a new HttpDownload.
     *  file_path: Path to the file, doesn't matter if it exists already.
     *  config: optional HttpDownloadConfig (to configure timeout, headers, retries, etc...)
     */
    pub async fn new(
        url: Url,
        file_path: PathBuf,
        client: Client,
        config: Option<HttpDownloadConfig>,
    ) -> Result<Self> {
        // If no configuration is passed the default one is copied
        let config = config.unwrap_or_default();
        let mut download = HttpDownload {
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
        mut stopper: Receiver<()>,
    ) -> Result<u64> {
        let mut downloaded_bytes = 0u64;
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let item = chunk?;
            let bytes_written = file_handler.write(&item).await? as u64;
            downloaded_bytes += bytes_written;
            match stopper.try_recv() {
                Ok(_) => {
                    log::info!("Download stop signal received for: {}", self.url);
                    return Ok(downloaded_bytes);
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Closed) => {
                    log::error!("Download stop signal channel closed for: {}", self.url);
                    log::info!("Stopping download because of error: {}", self.url);
                    return Err(Error::ChannelDrop(downloaded_bytes, self.url.clone()));
                }
            }
        }
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
            _ => return Err(Error::DownloadNotOk(status)),
        };

        match resp.content_length() {
            Some(val) => self.content_length = val,
            None => Err(Error::MissingContentLength(self.url.clone()))?,
        }
        self.supports_byte_ranges = supports_byte_ranges(resp.headers());
        Ok(())
    }

    async fn get_bytes_on_disk(&self) -> u64 {
        file_size(&self.file_path).await
    }
}

#[cfg(test)]
mod test {
    use std::error::Error;
    use std::iter::zip;
    use std::sync::Arc;

    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::*;

    type Test<T> = std::result::Result<T, Box<dyn Error>>;
    const TEST_DOWNLOAD_URL: &str =
        "https://dl.discordapp.net/apps/linux/0.0.26/discord-0.0.26.deb";

    async fn setup_test_download(url_str: &str) -> Test<(HttpDownload, TempDir)> {
        let tmp_dir = TempDir::new()?;
        let tmp_path = tmp_dir.path();
        let url = Url::parse(url_str)?;
        let file_path = tmp_path.join(PathBuf::from(parse_filename(&url).unwrap()));
        let client = Client::new();
        let download = HttpDownload::new(url, file_path, client, None).await?;
        Ok((download, tmp_dir))
    }

    #[tokio::test]
    async fn concurrent_download_test() -> Test<()> {
        let mut handles = Vec::new();
        let mut downloads = Vec::new();
        // Needed because if the tmp dir is dropped it is actually deleted in the Drop impl
        let mut anti_drop = Vec::new();
        for _ in 0..60 {
            let (download, _tmp_dir) = setup_test_download(TEST_DOWNLOAD_URL).await.unwrap();
            let download = Arc::new(download);
            let (tx, rx) = tokio::sync::oneshot::channel();
            downloads.push(download.clone());
            let fut = async move { download.start(rx).await };
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

    #[tokio::test]
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

    #[tokio::test]
    async fn default_download_test() -> Test<()> {
        // given
        let (download, _tmp_dir) = setup_test_download(TEST_DOWNLOAD_URL).await?;
        // when
        let (_tx, rx) = tokio::sync::oneshot::channel();
        let downloaded_bytes = download.start(rx).await?;
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

    #[tokio::test]
    async fn download_with_custom_chunksize_test() -> Test<()> {
        // given
        let mut config = HttpDownloadConfig::default();
        config.chunk_size = 1024 * 1029;
        // and
        let (mut download, _tmp_dir) = setup_test_download(TEST_DOWNLOAD_URL).await?;
        download.config = config;
        // when
        let (_tx, rx) = tokio::sync::oneshot::channel();
        let downloaded_bytes = download.start(rx).await?;
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

    #[tokio::test]
    async fn download_can_be_stopped_test() -> Test<()> {
        let (download, _tmp_dir) = setup_test_download(TEST_DOWNLOAD_URL).await?;
        let (tx, rx) = tokio::sync::oneshot::channel();
        let content_length = download.content_length;
        let handle = tokio::spawn(async move { download.start(rx).await });
        tx.send(()).expect("Message needs to be sent");
        let join_result = handle.await;
        let downloaded_bytes = join_result??;
        assert!(
            downloaded_bytes < content_length,
            "The downloaded bytes need to be less than the content_length when the download is stopped prematurely"
        );
        Ok(())
    }
}
