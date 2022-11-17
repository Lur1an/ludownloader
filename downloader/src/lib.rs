use directories::UserDirs;
use futures::Future;
use futures_util::StreamExt;
use log;
use reqwest::{
    header::{self, HeaderMap, HeaderValue},
    Client, Url,
};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

const DEFAULT_USER_AGENT: &str = "ludownloader";

#[derive(Debug)]
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
    /** Initializes a new HttpDownload.
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
     */
    pub async fn start(&mut self) -> Result<(), String> {
        self.update_server_data().await?;
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
        let mut stream = resp.bytes_stream();
        while let Some(item) = stream.next().await {
            let chunk = item.map_err(|e| format!(
                "Error while downloading file from url: {:#?}. Error: {:#?}",
                self.url, e
            ))?;
            file_handler
                .write_all(&chunk)
                .or(Err(format!("Error while writing to file")))?;
        }
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
                ))
            }
        }
        self.supports_bytes = supports_bytes(response.headers());
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

pub async fn quick_download(url: &str) -> Result<(), String> {
    let url = Url::parse(url).map_err(|e| format!("Failed parsing the url: {:?}", e))?;
    let fname = PathBuf::from(parse_filename(&url).ok_or("Couldn't get a filename from the url")?);

    let fpath;
    if let Some(user_dirs) = UserDirs::new() {
        let download_dir = user_dirs
            .download_dir()
            .ok_or("Couldn't get download dir")?;
        fpath = download_dir.join(fname);
    } else {
        return Err(String::from("Couldn't get UserDirs from OS"));
    }

    let mut download = HttpDownload::new(url, fpath, None);
    return download.start().await;
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
 * Tries to extract file size in bytes from given Path
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
    use pretty_assertions::{assert_eq};

    use std::error::Error;
    use tempfile::TempDir;

    /**
     * Type definition for tests to enable unchecked '?' syntax
     */
    type Test = Result<(), Box<dyn Error>>;

    #[test]
    fn supports_bytes_test() {
        // Given
        let mut headermap = HeaderMap::new();
        // When
        headermap.insert(
            header::ACCEPT_RANGES,
            HeaderValue::from_str("bytes").unwrap(),
        );
        // Then
        assert!(
            supports_bytes(&headermap),
            "HeaderMap should support bytes!"
        );
        // When
        headermap.insert(
            header::ACCEPT_RANGES,
            HeaderValue::from_str("something else").unwrap(),
        );
        // Then
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
            file_size(&fpath),
            bytes,
            "File should have as many bytes as written in the buffer!"
        );
        Ok(())
    }

    #[tokio::test]
    async fn update_server_data_test() -> Test {
        // given
        let url_str = "https://speed.hetzner.de/10GB.bin";
        let url = Url::parse(url_str)?;
        let file_path = PathBuf::from(parse_filename(&url).unwrap());
        let mut download = HttpDownload::new(url, file_path, None);
        // when
        download.update_server_data().await?;
        // then
        assert!(download.supports_bytes, "Server should support bytes!");
        assert_eq!(
            download.content_length, 10485760000,
            "content-length should be exactly the same as always for the 10GB file"
        );
        Ok(())
    }

    #[tokio::test]
    async fn start_download_test() -> Test {
        // given
        let tmp_dir = TempDir::new()?;
        let tmp_path = tmp_dir.path();
        // and
        let url_str = "https://github.com/yourkin/fileupload-fastapi/raw/a85a697cab2f887780b3278059a0dd52847d80f3/tests/data/test-10mb.bin";
        let url = Url::parse(url_str)?;
        let file_path = tmp_path.join(PathBuf::from(parse_filename(&url).unwrap()));
        let mut download = HttpDownload::new(url, file_path, None);
        // when
        download.start().await?;
        // then
        assert_eq!(
            download.content_length,
            file_size(&download.file_path),
            "File size should be equal to content_length"
        );

        Ok(())
    }

    #[ignore]
    #[tokio::test]
    async fn quick_download_test() -> Test {
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
