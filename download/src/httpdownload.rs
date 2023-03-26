use async_trait::async_trait;
use std::future::Future;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::RANGE;
use reqwest::{
    header::{self, HeaderMap, HeaderValue},
    Client, Response, Url,
};
use thiserror;
use thiserror::Error;
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::oneshot::error::TryRecvError;
use tokio::sync::oneshot::{channel, Receiver, Sender};
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

use crate::api::{Download, Error, Result};
use crate::util::{file_size, parse_filename, supports_byte_ranges};
use crate::{constants::DEFAULT_USER_AGENT, download_config::DownloadConfig};

pub struct HttpDownload {
    /**
     * Download Link
     */
    pub url: Url,
    /**
     * Target file for the download
     */
    pub file_path: PathBuf,

    pub config: DownloadConfig,
    /**
     * Currently used HttpClient
     */
    client: Client,
    /** Size of the download in bytes
     */
    pub content_length: u64,
    /**
    If the server for the Download supports bytes
    * This value gets updated by the struct
     */
    supports_byte_ranges: bool,
}

#[async_trait]
impl Download for HttpDownload {
    /** Starts the Download from scratch */
    async fn start(&self) -> Result<(JoinHandle<Result<u64>>, Sender<()>)> {
        let resp = self
            .client
            .get(self.url.as_ref())
            .timeout(self.config.timeout)
            .headers(self.config.headers.clone())
            .send()
            .await?;
        let (tx, rx) = channel::<()>();
        let file_handler = File::create(&self.file_path).await?;
        let f = HttpDownload::progress(
            self.url.clone(),
            self.config.chunk_size,
            resp,
            file_handler,
            rx,
        );
        let handle: JoinHandle<Result<u64>> = tokio::spawn(f);
        Ok((handle, tx))
    }

    async fn resume(&self) -> Result<(JoinHandle<Result<u64>>, Sender<()>)> {
        let downloaded_bytes = self.get_bytes_on_disk().await;
        if downloaded_bytes == self.content_length {
            log::warn!(
                "Tried downloading a file that was already downloaded: {}",
                self.url
            );
            return Err(Error::DownloadComplete(downloaded_bytes));
        }
        let (tx, rx) = channel::<()>();
        if !self.supports_byte_ranges {
            log::warn!(
                "Tried resuming a download that doesn't support byte ranges: {}",
                self.url
            );
            log::info!("Starting from scratch: {}", self.url);
            return self.start().await;
        }
        let file_handler = OpenOptions::new()
            .write(true)
            .append(true)
            .open(&self.file_path)
            .await?;

        let resp = self
            .client
            .get(self.url.as_ref())
            .timeout(self.config.timeout)
            .headers(self.config.headers.clone())
            .header(RANGE, format!("bytes={}-", downloaded_bytes))
            .send()
            .await?;
        let chunk_size = self.config.chunk_size;
        let url = self.url.clone();
        let f = async move {
            Ok(
                HttpDownload::progress(url, chunk_size, resp, file_handler, rx).await?
                    + downloaded_bytes,
            )
        };
        let handle: JoinHandle<Result<u64>> = tokio::spawn(f);
        Ok((handle, tx))
    }
}

impl HttpDownload {
    /** Initializes a new HttpDownload.
     *  file_path: Path to the file, doesn't matter if it exists already.
     *  config: optional HttpDownloadConfig (to configure timeout, headers, retries, etc...)
     */
    pub async fn new(
        url: Url,
        file_path: PathBuf,
        client: Client,
        config: Option<DownloadConfig>,
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
        url: Url,
        chunk_size: usize,
        resp: Response,
        mut file_handler: File,
        mut stopper: Receiver<()>,
    ) -> Result<u64> {
        let mut downloaded_bytes = 0u64;
        // Await the response, raise error with String msg otherwise
        let mut stream = resp.bytes_stream().chunks(chunk_size);
        while let Some(buffered_chunks) = stream.next().await {
            for item in buffered_chunks {
                let chunk = item?;
                downloaded_bytes += file_handler.write(&chunk).await? as u64;
            }
            match stopper.try_recv() {
                Ok(_) => {
                    log::info!("Download stop signal received for: {}", url);
                    return Ok(downloaded_bytes);
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Closed) => {
                    log::error!("Download stop signal channel closed for: {}", url);
                    log::info!("Stopping download because of error: {}", url);
                    return Err(Error::ChannelDrop(downloaded_bytes, url.clone()));
                }
            }
        }
        Ok(downloaded_bytes)
    }

    /**
    Queries the server to update some Download data.
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
    use tokio::sync::Mutex;

    use super::*;

    type Test<T> = std::result::Result<T, Box<dyn Error>>;

    async fn setup_test_download() -> Test<(HttpDownload, TempDir)> {
        let tmp_dir = TempDir::new()?;
        let tmp_path = tmp_dir.path();
        let url_str = "https://github.com/yourkin/fileupload-fastapi/raw/a85a697cab2f887780b3278059a0dd52847d80f3/tests/data/test-10mb.bin";
        let url = Url::parse(url_str)?;
        let file_path = tmp_path.join(PathBuf::from(parse_filename(&url).unwrap()));
        let download = HttpDownload::new(url, file_path, Client::new(), None).await?;
        Ok((download, tmp_dir))
    }

    #[tokio::test]
    async fn concurrent_download_test() -> Test<()> {
        let mut handles = Vec::new();
        let mut downloads = Vec::new();
        // Needed because if the tmp dir is dropped it is actually deleted in the Drop impl
        let mut _tmp_dir_owner = Vec::new();
        let mut _stopper_owner = Vec::new();
        for _ in 0..60 {
            let (download, _tmp_dir) = setup_test_download().await.unwrap();
            _tmp_dir_owner.push(_tmp_dir);
            let (handle, _stopper) = download.start().await?;
            _stopper_owner.push(_stopper);
            downloads.push(download);
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
        let url_str = "https://speed.hetzner.de/10GB.bin";
        let url = Url::parse(url_str)?;
        let file_path = PathBuf::from(parse_filename(&url).unwrap());
        // when creating a download, server data is present in the download struct
        let download = HttpDownload::new(url, file_path, Client::new(), None).await?;
        // then
        assert!(
            download.supports_byte_ranges,
            "Server should support bytes!"
        );
        assert_eq!(
            download.content_length, 10485760000,
            "content-length should be exactly the same as always for the 10GB file"
        );
        Ok(())
    }

    #[tokio::test]
    async fn default_download_test() -> Test<()> {
        // given
        let (download, _tmp_dir) = setup_test_download().await?;
        // when
        let (download_handle, _stopper) = download.start().await?;
        let join_result = download_handle.await;
        let downloaded_bytes = join_result??;
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
        let mut config = DownloadConfig::default();
        config.chunk_size = 1024 * 1029;
        // and
        let (mut download, _tmp_dir) = setup_test_download().await?;
        download.config = config;
        // when
        let (download_handle, _stopper) = download.start().await?;
        let downloaded_bytes = download_handle.await??;
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
}
