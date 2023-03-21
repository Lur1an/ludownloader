use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::{
    header::{self, HeaderMap, HeaderValue},
    Client, Url,
};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use crate::util::{file_size, parse_filename, supports_byte_ranges};
use crate::{constants::DEFAULT_USER_AGENT, download_config::DownloadConfig};

pub struct Download {
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
     Amount of bytes downloaded, same as size of the file found at file_path.
    * This value is calculated in case the download is started with resume() and is then updated for every downloaded chunk.
     */
    pub downloaded_bytes: Mutex<u64>,
    /**
    If the server for the Download supports bytes
    * This value gets updated by the struct
     */
    supports_byte_ranges: bool,
}

impl Download {
    /** Sends a request to the download server using encapsulated configuration and URL */
    pub async fn get(&self) -> Result<reqwest::Response, reqwest::Error> {
        self.client
            .get(self.url.as_ref())
            .timeout(self.config.timeout)
            .headers(self.config.headers.clone())
            .send()
            .await
    }
    /** Initializes a new HttpDownload.
     *  file_path: Path to the file, doesn't matter if it exists already.
     *  config: optional HttpDownloadConfig (to configure timeout, headers, retries, etc...)
     */
    pub async fn new(
        url: Url,
        file_path: PathBuf,
        client: Client,
        config: Option<DownloadConfig>,
    ) -> Result<Self, String> {
        // If no configuration is passed the default one is copied
        let config = config.unwrap_or_else(DownloadConfig::default);
        let downloaded_bytes = file_size(&file_path).await;
        let mut download = Download {
            url,
            file_path,
            config,
            client,
            downloaded_bytes: Mutex::new(downloaded_bytes),
            supports_byte_ranges: false,
            content_length: 0u64,
        };
        download.update_server_data().await?;
        Ok(download)
    }

    /** Starts the Download from scratch */
    pub async fn start(&self) -> Result<(), String> {
        // Send the frigging request
        let resp = self
            .get()
            .await
            .map_err(|_| format!("Failed to GET: '{}'", self.url.as_str()))?;
        // Open the file
        let mut file_handler = File::create(&self.file_path).await.map_err(|e| {
            format!(
                "Failed creating/opening File for HttpDownload, path: {:?}, error: {:?}",
                self.file_path, e
            )
        })?;
        let mut downloaded_bytes = self.get_downloaded_bytes().await;
        // Await the response, raise error with String msg otherwise
        let mut stream = resp.bytes_stream().chunks(self.config.chunk_size);
        while let Some(buffered_chunks) = stream.next().await {
            for item in buffered_chunks {
                let chunk = item.map_err(|e| {
                    format!("Error while chunking download response. Error: {:?}", e)
                })?;
                downloaded_bytes += file_handler.write(&chunk).await.map_err(|e| {
                    format!(
                        "Error while writing to file at {:?}. Error: {:#?}",
                        self.file_path, e
                    )
                })? as u64;
            }
            self.set_downloaded_bytes(downloaded_bytes).await;
        }
        Ok(())
    }
    /**
    Queries the server to update some Download data.
    * updates content_length
    * updates accepts_bytes
     */
    async fn update_server_data(&mut self) -> Result<(), String> {
        let response = self.get().await.map_err(|err| {
            format!(
                "Couldn't execute Head request! url: {:?}, error: {:#?}",
                self.url.as_str(),
                err
            )
        })?;
        match response.content_length() {
            Some(val) => self.content_length = val,
            None => {
                return Err(format!(
                    "Couldn't get content_length from: {:?}",
                    self.url.as_str()
                ));
            }
        }
        self.supports_byte_ranges = supports_byte_ranges(response.headers());
        Ok(())
    }

    async fn get_bytes_on_disk(&self) -> u64 {
        file_size(&self.file_path).await
    }

    async fn get_downloaded_bytes(&self) -> u64 {
        *self.downloaded_bytes.lock().await
    }

    async fn set_downloaded_bytes(&self, value: u64) -> () {
        let mut guard = self.downloaded_bytes.lock().await;
        *guard = value;
    }
}

#[cfg(test)]
mod test {
    use std::error::Error;
    use std::sync::Arc;

    use pretty_assertions::assert_eq;
    use tempfile::TempDir;
    use tokio::sync::Mutex;

    use super::*;

    async fn setup_test_download() -> Result<(Download, TempDir), Box<dyn Error>> {
        let tmp_dir = TempDir::new()?;
        let tmp_path = tmp_dir.path();
        let url_str = "https://github.com/yourkin/fileupload-fastapi/raw/a85a697cab2f887780b3278059a0dd52847d80f3/tests/data/test-10mb.bin";
        let url = Url::parse(url_str)?;
        let file_path = tmp_path.join(PathBuf::from(parse_filename(&url).unwrap()));
        let download = Download::new(url, file_path, Client::new(), None).await?;
        Ok((download, tmp_dir))
    }

    #[tokio::test]
    async fn concurrent_download_test() -> Result<(), Box<dyn Error>> {
        let mut download_arcs = Vec::new();
        let mut futures = Vec::new();
        let mut _tmp_dir_owner = Vec::new();
        for _ in 0..60 {
            let (download, _tmp_dir) = setup_test_download().await.unwrap();
            _tmp_dir_owner.push(_tmp_dir);
            let download_arc = Arc::new(download);
            download_arcs.push(download_arc.clone());
            let task = tokio::task::spawn(async move { download_arc.start().await.unwrap() });
            futures.push(task);
        }
        futures::future::join_all(futures).await;

        for download in download_arcs {
            assert_eq!(
                download.content_length,
                file_size(&download.file_path).await,
                "File size should be equal to content_length"
            );
            assert_eq!(
                download.get_downloaded_bytes().await,
                download.content_length,
                "The downloaded bytes need to be equal to the content_length when the download is finished"
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn server_data_is_requested_on_create_test() -> Result<(), Box<dyn Error>> {
        // given
        let url_str = "https://speed.hetzner.de/10GB.bin";
        let url = Url::parse(url_str)?;
        let file_path = PathBuf::from(parse_filename(&url).unwrap());
        // when creating a download, server data is present in the download struct
        let download = Download::new(url, file_path, Client::new(), None).await?;
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
    async fn default_download_test() -> Result<(), Box<dyn Error>> {
        // given
        let (download, _tmp_dir) = setup_test_download().await?;
        // when
        download.start().await?;
        // then
        assert_eq!(
            download.content_length,
            file_size(&download.file_path).await,
            "File size should be equal to content_length"
        );
        assert_eq!(
            download.get_downloaded_bytes().await,
            download.content_length,
            "The downloaded bytes need to be equal to the content_length when the download is finished"
        );
        Ok(())
    }

    #[tokio::test]
    async fn download_with_custom_chunksize_test() -> Result<(), Box<dyn Error>> {
        // given
        let mut config = DownloadConfig::default();
        config.chunk_size = 1024 * 1029;
        // and
        let (mut download, _tmp_dir) = setup_test_download().await?;
        download.config = config;
        // when
        download.start().await?;
        // then
        assert_eq!(
            download.content_length,
            file_size(&download.file_path).await,
            "File size should be equal to content_length"
        );
        assert_eq!(
            download.get_downloaded_bytes().await,
            download.content_length,
            "The downloaded bytes need to be equal to the content_length when the download is finished"
        );
        Ok(())
    }
}
