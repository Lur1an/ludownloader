use std::fs::{self, File};
use std::path::{Path, PathBuf};

use futures::FutureExt;
use log;
use std::io::Write;
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::{self, HeaderMap, HeaderName, HeaderValue};
use reqwest::{Url, Client};

const DEFAULT_USER_AGENT: &str = "ludownloader";

#[derive(Debug)]
struct HttpDownload {
    /**
     * Download Link\
     */
    url: Url,
    /**
     * Target file for the download
     */
    file_path: PathBuf,
    file_handler: Option<File>,
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
    ongoing: bool,
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
    supports_bytes: bool,
}

impl HttpDownload {
    /**
       Initializes a new HttpDownload.
    * file_path: Path to the file, doesn't matter if it exists already.
    * config: optional HttpDownloadConfig (to configure timeout, headers, retries, etc...)
    */
    pub fn new(url: Url, file_path: PathBuf, config: Option<HttpDownloadConfig>) -> Self {
        // If no configuration is passed the default one is copied
        let config = config.unwrap_or_else(|| HttpDownloadConfig::default());
        let downloaded_bytes = file_size(&file_path);
        let download = HttpDownload {
            url,
            file_path,
            config,
            downloaded_bytes,
            file_handler: None,
            supports_bytes: false,
            tries: 0,
            client: Client::new(),
            ongoing: false,
            content_length: 0u64,
        };
        return download;
    }

    /**
     * Starts the Download from scratch
     * If file_handler is None : Opens file handler in write
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
        let file_handler: File;
        match File::create(&self.file_path) {
            Ok(f) => file_handler = f,
            Err(err) => {
                return Err(format!(
                    "Failed creating/opening File for HttpDownload. path: {:?}, error: {:?}",
                    self.file_path, err
                ))
            }
        };
        self.file_handler = Some(file_handler);
        // Await the response
        let resp = resp.await.or(Err(format!(
            "Failed to send GET from: '{}'",
            self.url.as_str()
        )))?;
        // Get content-length
        // self.content_length = resp.content_length().ok_or(Err(format!(
        //     "Failed to get content length from: '{}'",
        //     self.url.as_str()
        // )));
        // Download data
        let buffer_size = self.config.chunk_size as u64;
        Ok(())
    }
    /**
     * Pauses the download
     */
    pub fn pause(&mut self) {}

    /**
     * Tries to resume the download
     * If file_handler is None : Opens file handler in append
     */
    pub fn resume(&mut self) {}

    /**
     * Queries the server to check if the download supports Bytes, then updates the field of the struct with the result.
     * This function doesn't fail or return errors, on worst case nothing happens and self.supports_bytes remains unchanged
     */
    async fn update_supports_bytes(&mut self) {
        self.get_server_headers()
            .await
            .ok()
            .and_then(|headers| Some(supports_bytes(&headers)))
            .and_then(|support| Some(self.supports_bytes = support));
    }

    /**
     * Requests headers from server of this Download
     */
    async fn get_server_headers(&self) -> Result<HeaderMap, reqwest::Error> {
        let response = self
            .client
            .get(self.url.as_ref())
            .timeout(self.config.timeout)
            .header(header::ACCEPT, HeaderValue::from_str("*/*").unwrap())
            .headers(self.config.headers.clone())
            .send()
            .await?;
        Ok(response.headers().clone())
    }
}

/**
Holds the http configuration for the Download
*/
#[derive(Debug, Clone)]
struct HttpDownloadConfig {
    /**
     * Limits the amount of retries the Download can do before being terminated
     */
    max_retries: u32,
    /**
     * Timeout parameter for requests
     */
    timeout: Duration,
    num_workers: usize,
    /**
     * Request headers for the Download
     */
    headers: HeaderMap,
    chunk_size: u64,
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
            timeout: Duration::from_secs(30),
            num_workers: 8,
            headers: HeaderMap::new(),
            chunk_size: 512_000u64,
        };
        config.headers.insert(
            header::USER_AGENT,
            HeaderValue::from_str(DEFAULT_USER_AGENT).unwrap(),
        );
        return config;
    }
}

/**
 * Parses the filename from the download URL
 * Returns None if there is no filename or if url.path_segments() fails
 */
pub fn parse_filename(url: &Url) -> Option<&str> {
    let segments = url.path_segments()?;
    let filename = segments.last()?;
    if filename.is_empty() {
        None
    } else {
        Some(filename)
    }
}

/**
 * Given a HeaderMap checks if the server that sent the headers supports byte ranges
 */
pub fn supports_bytes(headers: &HeaderMap) -> bool {
    if let Some(val) = headers.get(header::ACCEPT_RANGES) {
        return val == "bytes";
    }
    return false;
}

/**
 * Tries to extract file size from given Path
 * If the Path is wrong or the metadata read operation fails the function returns 0
 */
pub fn file_size(fpath: &Path) -> u64 {
    match fs::metadata(fpath) {
        Ok(metadata) => metadata.len(),
        _ => 0,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::{assert_eq, assert_ne};
    use std::error::Error;
    use tempfile::TempDir;

    /**
     * Type definition for tests to enable unchecked '?' syntax
     */
    type Test = Result<(), Box<dyn Error>>;

    fn supports_bytes_test() {
        let mut headermap = HeaderMap::new();

        headermap.insert(
            header::ACCEPT_RANGES,
            HeaderValue::from_str("bytes").unwrap(),
        );
        assert!(
            supports_bytes(&headermap),
            "HeaderMap should support bytes!"
        );

        headermap.insert(
            header::ACCEPT_RANGES,
            HeaderValue::from_str("something else").unwrap(),
        );
        assert!(
            !supports_bytes(&headermap),
            "HeaderMap shouldn't support bytes anymore!"
        );
    }

    #[test]
    fn parse_filename_success_test() -> Test {
        let url = Url::parse("https://somewebsite.biz/api/v1/big-ass-file.fantasy")?;
        let filename = parse_filename(&url).unwrap();
        assert_eq!(filename, "big-ass-file.fantasy", "File name doesn't match!");
        Ok(())
    }

    #[test]
    fn parse_filename_failure_test() -> Test {
        let url = Url::parse("https://somewebsite.biz/")?;
        assert!(parse_filename(&url).is_none());
        Ok(())
    }

    #[test]
    fn download_setup_test() -> Test {
        Ok(())
    }

    #[test]
    fn file_size_retrieval_test() -> Test {
        // Setup
        let tmp_dir = TempDir::new()?;
        let tmp_path = tmp_dir.path();
        let url = Url::parse("https://speed.hetzner.de/1GB.bin")?;
        let fname = parse_filename(&url).unwrap();
        let fpath = tmp_path.join(Path::new(fname));
        // Create file and check that it's empty (size == 0)
        let mut file_handler = File::create(&fpath).unwrap();
        assert_eq!(
            file_size(fpath.as_path()),
            0,
            "Newly created file should have 0 Bytes!"
        );
        // Write some bytes to the buffer
        let bytes: u64 = file_handler.write(b"b")? as u64;
        // Flush the buffer to the file
        file_handler.flush()?;
        // Assert that the file_size function retrieves the exact number of bytes written
        assert_eq!(
            file_size(fpath.as_path()),
            bytes,
            "File should have as many bytes as written in the buffer!"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_server_headers_test() -> Test {
        let url = Url::parse("https://speed.hetzner.de/1GB.bin")?;
        let file_path = PathBuf::from("tmp/ludownloader/1GB.bin");
        let download = HttpDownload::new(url, file_path, None);
        let download_headers = download.get_server_headers().await;
        assert!(download_headers.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn update_supports_bytes_test() -> Test {
        let url = Url::parse("https://speed.hetzner.de/1GB.bin")?;
        let file_path = PathBuf::from("tmp/ludownloader/1GB.bin");
        let mut download = HttpDownload::new(url, file_path, None);
        download.update_supports_bytes().await;
        assert!(download.supports_bytes);
        Ok(())
    }
}
