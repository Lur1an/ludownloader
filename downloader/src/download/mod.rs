use std::fmt::Error;
use std::fs;
use std::path::{Path, PathBuf};

use std::time::Duration;
use log;
use std::{fs::File, io::Write};

use futures_util::StreamExt;
use reqwest::header::{self, HeaderMap, HeaderValue, HeaderName};
use reqwest::{Client, Url};

struct Package {
    downloads: Vec<HttpDownload>,
    root_folder: PathBuf,
    package_name: String,
}

impl Package {
    fn add_download(download: HttpDownload) {}
}

#[derive(Debug, Clone)]
struct HttpDownload {
    /**
     * Url of the download
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
    content_length: u64
}

impl HttpDownload {
    /**
     * Initializes a new HttpDownload.
     * file_path needs to be computed beforehand, it's not a responsibility of the Download.
     */
    fn new(url: Url, file_path: PathBuf, config: Option<HttpDownloadConfig>) -> Self {
        // If no configuration is passed the default one is copied
        let config = config.unwrap_or_else(|| HttpDownloadConfig::default());
        HttpDownload {
            url,
            file_path,
            config,
            tries: 0,
            client: Client::new(),
            ongoing: false,
            content_length: 0
        }
    }

    fn start(&self) {

    }

    fn pause(&self) {
    
    }

    async fn download(&self) -> Result<(), reqwest::Error> {
        let resp = self
            .client
            .get(self.url.as_ref())
            .timeout(self.config.timeout)
            .headers(self.config.headers.clone())
            .send()
            .await?;
        Ok(())
    }
    async fn prepare_headers(&self) {
        log::info!("preparing headers for download");
        let server_headers = self.get_server_headers();
    }
    /**
     * Requests headers from server of Download 
     */
    async fn get_server_headers(&self) -> Result<HeaderMap, reqwest::Error>{
        let response = self
            .client
            .get(self.url.as_ref())
            .timeout(self.config.timeout)
            .header(header::ACCEPT, HeaderValue::from_str("*/*").unwrap())
            .header(
                header::USER_AGENT,
                self.config.headers.get(header::USER_AGENT).unwrap(),
            )
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
}

const DEFAULT_USER_AGENT: &str = "ludownloader";

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
            num_workers: 8usize,
            headers: HeaderMap::new(),
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
fn parse_filename(url: &Url) -> Option<&str> {
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
fn supports_bytes(headers: &HeaderMap) -> bool {
    match headers.get(header::ACCEPT_RANGES) {
        Some(val) => val == "bytes",
        None => false,
    }
}

/**
 * Tries to extract file size from given Path
 * If the Path is wrong or the metadata read operation fails the function returns 0
 */
fn file_size(fpath: &Path) -> u64 {
    match fs::metadata(fpath) {
        Ok(metadata) => metadata.len(),
        _ => 0,
    }
}

async fn download_big_file() -> Result<(), reqwest::Error> {
    let client = Client::new();
    let url = Url::parse("https://example.net");

    let file_url = "https://speed.hetzner.de/1GB.bin";
    let response = client.get(file_url).send().await?;

    let mut stream = response.bytes_stream();
    let mut file: File = File::create("/tmp/testfile").expect("Failed creating file");

    while let Some(item) = stream.next().await {
        let chunk = item.or(Err("Error while downloading chunk")).unwrap();
        file.write(&chunk).expect("Writing ok");
    }
    file.flush().expect("Writing ok");
    return Ok(());
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
    fn download_execution_test() -> Test {
        let tmp_dir = TempDir::new()?;
        let tmp_path = tmp_dir.path();
        let url = Url::parse("https://speed.hetzner.de/1GB.bin")?;
        let fname = parse_filename(&url).unwrap();
        let fpath = tmp_path.join(Path::new(fname));
        let mut file_handler = File::create(&fpath).unwrap();
        assert_eq!(
            file_size(fpath.as_path()),
            0,
            "Newly created file should have 0 Bytes!"
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
        println!("{:?}", download_headers);
        return Ok(())
    }
}
