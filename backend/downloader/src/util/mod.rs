use reqwest::header::HeaderMap;
use reqwest::{header, Client, Url};
use std::error::Error;
use std::path::Path;
use tempfile::TempDir;

use crate::httpdownload::download::HttpDownload;

/// Extracts filesize from path, if file does not exist or read fails the function returns 0
pub async fn file_size(fpath: &Path) -> u64 {
    match tokio::fs::metadata(fpath).await {
        Ok(metadata) => metadata.len(),
        _ => 0,
    }
}
pub const HALF_SECOND: std::time::Duration = std::time::Duration::from_millis(500);
pub type TestResult<T> = std::result::Result<T, Box<dyn Error>>;
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

pub fn kb(bytes: u64) -> f64 {
    bytes as f64 / 1024.0
}

pub fn mb(bytes: u64) -> f64 {
    bytes as f64 / 1024.0 / 1024.0
}

pub fn gb(bytes: u64) -> f64 {
    bytes as f64 / 1024.0 / 1024.0 / 1024.0
}
/**
 * Given a HeaderMap checks if the server that sent the headers supports byte ranges
 */
pub fn supports_byte_ranges(headers: &HeaderMap) -> bool {
    if let Some(val) = headers.get(header::ACCEPT_RANGES) {
        val == "bytes"
    } else {
        false
    }
}

#[cfg(test)]
pub async fn setup_test_download(url_str: &str) -> anyhow::Result<(HttpDownload, TempDir)> {
    let tmp_dir = TempDir::new()?;
    let tmp_path = tmp_dir.path().to_owned();
    let url = Url::parse(url_str)?;
    let filename = parse_filename(&url).unwrap().to_string();
    let client = Client::new();
    let download = HttpDownload::create(url, tmp_path, filename, client, None).await?;
    Ok((download, tmp_dir))
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;
    use reqwest::header::HeaderValue;
    use std::{error::Error, fs::File, io::Write};
    use tempfile::TempDir;
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
            supports_byte_ranges(&headermap),
            "HeaderMap should support bytes!"
        );
        // When
        headermap.insert(
            header::ACCEPT_RANGES,
            HeaderValue::from_str("something else").unwrap(),
        );
        // Then
        assert!(
            !supports_byte_ranges(&headermap),
            "HeaderMap shouldn't support bytes anymore!"
        );
    }

    #[test]
    fn parse_filename_test() -> Result<(), Box<dyn Error>> {
        // Result<(), Box<dyn Error>> success
        let url = Url::parse("https://somewebsite.biz/api/v1/big-ass-file.fantasy")?;
        let filename = parse_filename(&url).unwrap();
        assert_eq!(filename, "big-ass-file.fantasy", "File name doesn't match!");
        // Result<(), Box<dyn Error>> failure
        let url = Url::parse("https://somewebsite.biz/")?;
        assert!(parse_filename(&url).is_none());
        Ok(())
    }

    #[tokio::test]
    async fn file_size_retrieval_test() -> Result<(), Box<dyn Error>> {
        // Setup
        let tmp_dir = TempDir::new()?;
        let tmp_path = tmp_dir.path();
        let url = Url::parse("https://speed.hetzner.de/1GB.bin")?;
        let fname = parse_filename(&url).unwrap();
        let fpath = tmp_path.join(Path::new(fname));
        // Create file and check that it's empty (size == 0)
        let mut file_handler = File::create(&fpath).unwrap();
        assert_eq!(
            file_size(fpath.as_path()).await,
            0,
            "Newly created file should have 0 Bytes!"
        );
        // Write some bytes to the buffer
        let bytes: u64 = file_handler.write(b"b")? as u64;
        // Flush the buffer to the file
        file_handler.flush()?;
        // Assert that the file_size function retrieves the exact number of bytes written
        assert_eq!(
            file_size(&fpath).await,
            bytes,
            "File should have as many bytes as written in the buffer!"
        );
        Ok(())
    }
}
