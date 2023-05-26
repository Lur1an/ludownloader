use reqwest::header::{self, HeaderMap, HeaderValue};
use std::time::Duration;

pub const DEFAULT_USER_AGENT: &str = "ludownloader";
pub const DEFAULT_CHUNK_SIZE: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct HttpDownloadConfig {
    pub timeout: Duration,
    pub headers: HeaderMap,
    pub chunk_size: usize,
}

impl Default for HttpDownloadConfig {
    fn default() -> Self {
        let mut config = HttpDownloadConfig {
            timeout: Duration::from_secs(60),
            headers: HeaderMap::new(),
            chunk_size: DEFAULT_CHUNK_SIZE,
        };
        config.headers.insert(
            header::USER_AGENT,
            HeaderValue::from_str(DEFAULT_USER_AGENT).unwrap(),
        );
        config
    }
}
