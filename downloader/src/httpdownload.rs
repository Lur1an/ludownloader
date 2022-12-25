use core::num;
use std::fs::{self, File};
use std::io::Write;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use directories::UserDirs;
use futures::Future;
use futures_util::StreamExt;
use log;
use reqwest::{Client, header, Url};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use crate::constants::DEFAULT_USER_AGENT;
use crate::util::{file_size, parse_filename};
use crate::util;

pub struct HttpDownload {
    /**
     * Download Link
     */
    url: Url,
    /**
     * Target file for the download
     */
    file_path: PathBuf,
    /**
     * Amount of current retries
     */
    tries: u32,
    /**
     * Download configuration
     */
    config: HttpDownloadConfig,
    /**
     * Currently used HttpClient
     */
    client: Client,
    paused: Arc<Mutex<bool>>,
    complete: bool,
    /**
     * Size of the download in bytes
     */
    content_length: u64,
    /**
     Amount of bytes downloaded, aka size of the file found at file_path.
    * This value is calculated in case the download is started with resume() and is then updated for every downloaded chunk.
     */
    downloaded_bytes: u64,
    /**
    If the server for the Download supports bytes
    * This value get's updated on start()
     */
    supports_byte_ranges: bool,
    /**
     * This function is called every time there is an update to the download progress
     * the float passed is a value between 0 and 1 representing the progress on the download
     *  - The idea is to use this functions return value to stop the download too, the main thread gets a chance to stop
     * the download on every update this download sends.
     */
    on_update: Option<Box<dyn FnMut(f64) -> Box<dyn Future<Output=bool>>>>,
}

impl HttpDownload {
    /** Initializes a new HttpDownload.
       *  file_path: Path to the file, doesn't matter if it exists already.
       *  config: optional HttpDownloadConfig (to configure timeout, headers, retries, etc...)
     */
    pub async fn new(url: Url, file_path: PathBuf, config: Option<HttpDownloadConfig>) -> Result<Self, String> {
        // If no configuration is passed the default one is copied
        let config = config.unwrap_or_else(|| HttpDownloadConfig::default());
        let downloaded_bytes = file_size(&file_path);
        let mut download = HttpDownload {
            url,
            file_path,
            config,
            downloaded_bytes,
            supports_byte_ranges: false,
            tries: 0,
            client: Client::new(),
            paused: Arc::new(Mutex::new(true)),
            complete: false,
            content_length: 0u64,
            on_update: None,
        };
        download.update_server_data().await?;
        return Ok(download);
    }

    pub async fn update_progress(&mut self) {}
    /**
     * Starts the Download from scratch
     */
    pub async fn start(&mut self) -> Result<(), String> {
        // Send the friggin request
        let resp = self
            .client
            .get(self.url.as_ref())
            .timeout(self.config.timeout)
            .headers(self.config.headers.clone())
            .send();
        // Open the file
        let mut file_handler = File::create(&self.file_path).or(Err(format!(
            "Failed creating/opening File for HttpDownload. path: {:?}",
            self.file_path
        )))?;
        // Await the response, raise error with String msg otherwise
        let resp = resp.await.or(Err(format!(
            "Failed to send GET to: '{}'",
            self.url.as_str()
        )))?;

        if let Some(chunk_size) = self.config.chunk_size {
            // Chunked-Buffer download
            let mut stream = resp.bytes_stream().chunks(chunk_size);
            while let Some(buffered_chunks) = stream.next().await {
                for result in buffered_chunks {
                    match result {
                        Ok(chunk) => self.downloaded_bytes += file_handler.write(&chunk)
                            .or(Err(format!("Error while writing to file at {:?}", self.file_path)))? as u64,
                        Err(e) => return Err(format!("Error while chunking download response. Error: {:?}", e)),
                    }
                }
            }
        } else {
            // Simple Chunked download, write em as they come
            let mut stream = resp.bytes_stream();
            while let Some(item) = stream.next().await {
                let chunk = item.map_err(|e| format!(
                    "Error while downloading file from url: {:#?}. Error: {:#?}",
                    self.url, e
                ))?;
                self.downloaded_bytes += file_handler
                    .write(&chunk)
                    .or(Err(format!("Error while writing to file at {:?}", self.file_path)))? as u64;
                self.update_progress().await;
            }
        }
        // chunked_stream.next()

        self.complete = true;
        Ok(())
    }
    /**
     * Pauses the download
     */
    pub fn pause(&mut self) {}

    /**
     * Tries to resume the download
     */
    pub fn resume(&mut self) {}

    /**
    Queries the server to update some Download data.
    * updates content_length
    * updates accepts_bytes
    * This function doesn't fail or return errors, on worst case nothing happens
     */
    async fn update_server_data(&mut self) -> Result<(), String> {
        let response = self
            .client
            .get(self.url.as_ref())
            .timeout(self.config.timeout)
            .send()
            .await
            .map_err(|err| {
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
        self.supports_byte_ranges = util::supports_byte_ranges(response.headers());
        Ok(())
    }
}

/**
Holds the http configuration for the Download
 */
#[derive(Debug, Clone)]
pub struct HttpDownloadConfig {
    /**
     * Limits the amount of retries the Download can do before being terminated
     */
    max_retries: u32,
    /**
     * Timeout parameter for requests
     */
    timeout: Duration,
    /**
     * Number of threads that can concurrently handle this download, ignored
     * if the server doesn't support http ranges
     */
    num_workers: usize,
    /**
     * Request headers for the Download
     */
    headers: HeaderMap,
    chunk_size: Option<usize>,
}

impl HttpDownloadConfig {
    /**
    Creates a default set of settings.
    * max_retries: 100
    * headers: { user-agent: "ludownloader" }
    * num_workers: 8
    * timeout: 30s
     */
    fn default() -> Self {
        let mut config = HttpDownloadConfig {
            max_retries: 100,
            timeout: Duration::from_secs(60),
            num_workers: 8,
            headers: HeaderMap::new(),
            chunk_size: None,
        };
        config.headers.insert(
            header::USER_AGENT,
            HeaderValue::from_str(DEFAULT_USER_AGENT).unwrap(),
        );
        return config;
    }
}

pub async fn quick_download(url: &str) -> Result<(), String> {
    let url = Url::parse(url).map_err(|e| format!("Failed parsing the url: {:?}", e))?;
    let fname = PathBuf::from(util::parse_filename(&url).ok_or("Couldn't get a filename from the url")?);

    let fpath;
    if let Some(user_dirs) = UserDirs::new() {
        let download_dir = user_dirs
            .download_dir()
            .ok_or("Couldn't get download dir")?;
        fpath = download_dir.join(fname);
    } else {
        return Err(String::from("Couldn't get UserDirs from OS"));
    }

    let mut download = HttpDownload::new(url, fpath, None).await?;
    return download.start().await;
}

#[cfg(test)]
mod test {
    use std::error::Error;

    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::*;

    #[tokio::test]
    async fn server_data_is_requested_on_create_test() -> Result<(), Box<dyn Error>> {
        // given
        let url_str = "https://speed.hetzner.de/10GB.bin";
        let url = Url::parse(url_str)?;
        let file_path = PathBuf::from(parse_filename(&url).unwrap());
        // when creating a download, server data is present in the download struct
        let mut download = HttpDownload::new(url, file_path, None).await?;
        // then
        assert!(download.supports_byte_ranges, "Server should support bytes!");
        assert_eq!(
            download.content_length, 10485760000,
            "content-length should be exactly the same as always for the 10GB file"
        );
        Ok(())
    }

    #[tokio::test]
    async fn default_download_no_chunks_test() -> Result<(), Box<dyn Error>> {
        // given
        let tmp_dir = TempDir::new()?;
        let tmp_path = tmp_dir.path();
        // and
        let url_str = "https://github.com/yourkin/fileupload-fastapi/raw/a85a697cab2f887780b3278059a0dd52847d80f3/tests/data/test-10mb.bin";
        let url = Url::parse(url_str)?;
        let file_path = tmp_path.join(PathBuf::from(parse_filename(&url).unwrap()));
        let mut download = HttpDownload::new(url, file_path, None).await?;
        // when
        download.start().await?;
        // then
        assert_eq!(
            download.content_length,
            file_size(&download.file_path),
            "File size should be equal to content_length"
        );
        assert_eq!(
            download.downloaded_bytes,
            download.content_length,
            "The downloaded bytes need to be equal to the content_length when the download is finished"
        );
        assert!(
            download.complete, "Download should know that it's complete and expose this through the API"
        );

        Ok(())
    }

    #[tokio::test]
    async fn download_with_chunksize_test() -> Result<(), Box<dyn Error>> {
        // given
        let tmp_dir = TempDir::new()?;
        let tmp_path = tmp_dir.path();
        // and
        let url_str = "https://github.com/yourkin/fileupload-fastapi/raw/a85a697cab2f887780b3278059a0dd52847d80f3/tests/data/test-10mb.bin";
        let url = Url::parse(url_str)?;
        let file_path = tmp_path.join(PathBuf::from(parse_filename(&url).unwrap()));
        let mut config = HttpDownloadConfig::default();
        config.chunk_size = Some(536870912);
        let mut download = HttpDownload::new(url, file_path, None).await?;
        // when
        download.start().await?;
        // then
        assert_eq!(
            download.content_length,
            file_size(&download.file_path),
            "File size should be equal to content_length"
        );
        assert_eq!(
            download.downloaded_bytes,
            download.content_length,
            "The downloaded bytes need to be equal to the content_length when the download is finished"
        );
        assert!(
            download.complete, "Download should know that it's complete and expose this through the API"
        );

        Ok(())
    }

    #[ignore]
    #[tokio::test]
    async fn quick_download_test() -> Result<(), Box<dyn Error>> {
        // given
        let url_str = "https://github.com/yourkin/fileupload-fastapi/raw/a85a697cab2f887780b3278059a0dd52847d80f3/tests/data/test-10mb.bin";
        let url = Url::parse(url_str)?;
        let user_dirs = UserDirs::new().unwrap();
        let download_dir = user_dirs.download_dir().unwrap();
        let filename = parse_filename(&url).unwrap();
        let expected_download_path = download_dir.join(filename);
        // when
        quick_download(url_str).await?;
        // then
        assert!(file_size(&expected_download_path) != 0);
        fs::remove_file(expected_download_path)?;
        Ok(())
    }
}
